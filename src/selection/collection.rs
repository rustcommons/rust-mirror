//! Immutable multi-version generations assembled from verified local snapshots.
use super::{parsed, read_file, Selection, SelectionSnapshot, Toolchain};
use crate::{
    checksum, safe_path, sha256, valid_hash, Result, SnapshotFile, ARTIFACT_LIMIT, METADATA_LIMIT,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    path::{Path, PathBuf},
};
const INVENTORY_LIMIT: u64 = 1024 * 1024;
const FILE_LIMIT: usize = 4096;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolchainRecord {
    pub selection: Selection,
    pub release: String,
    pub date: String,
    pub pinned_toolchain: String,
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CollectionSnapshot {
    pub schema: u32,
    pub rustup_version: String,
    pub toolchains: Vec<ToolchainRecord>,
    /// SHA-256 identities of directly supplied inventories, including the previous generation.
    pub inputs: Vec<String>,
    pub files: Vec<SnapshotFile>,
}
#[derive(Clone)]
pub(super) struct Entry {
    pub(super) source: PathBuf,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}
pub(super) struct Input {
    pub(super) rustup: String,
    pub(super) records: Vec<ToolchainRecord>,
    pub(super) files: BTreeMap<String, Entry>,
    pub(super) identity: String,
}
pub(super) fn rolling(path: &str) -> bool {
    ["stable", "beta", "nightly"].iter().any(|c| {
        [
            format!("dist/channel-rust-{c}.toml"),
            format!("dist/channel-rust-{c}.toml.sha256"),
            format!("dist/channel-rust-{c}.toml.asc"),
        ]
        .iter()
        .any(|p| p == path)
    })
}
pub(super) fn read_input(root: &Path) -> Result<Input> {
    let raw = read_file(&root.join("snapshot.toml"), INVENTORY_LIMIT)?;
    let value: toml::Value = toml::from_str(std::str::from_utf8(&raw)?)?;
    let (rustup, records, files) =
        match value.get("schema").and_then(toml::Value::as_integer) {
            Some(2) => {
                let s: SelectionSnapshot = toml::from_str(std::str::from_utf8(&raw)?)?;
                (
                    s.rustup_version,
                    vec![ToolchainRecord {
                        selection: s.selection,
                        release: s.release,
                        date: s.date,
                        pinned_toolchain: s.pinned_toolchain,
                    }],
                    s.files,
                )
            }
            Some(3) => {
                let s: CollectionSnapshot = toml::from_str(std::str::from_utf8(&raw)?)?;
                if s.inputs.len() > 33 || s.inputs.is_empty() {
                    return Err("invalid collection input identities".into());
                }
                for identity in &s.inputs {
                    valid_hash(identity)?;
                }
                (s.rustup_version, s.toolchains, s.files)
            }
            _ => return Err(
                "assemble accepts selected schema 2 or collection schema 3, not legacy snapshots"
                    .into(),
            ),
        };
    crate::version(&rustup)?;
    if records.is_empty() || records.len() > 256 || files.is_empty() || files.len() > FILE_LIMIT {
        return Err("collection record/file limit exceeded".into());
    }
    let mut entries = BTreeMap::new();
    let mut expected = BTreeSet::from([PathBuf::from("snapshot.toml")]);
    for item in files {
        safe_path(&item.path)?;
        valid_hash(&item.sha256)?;
        if item.path == "snapshot.toml"
            || item.bytes > ARTIFACT_LIMIT
            || entries.contains_key(&item.path)
        {
            return Err("invalid duplicate/reserved source inventory entry".into());
        }
        expected.insert(PathBuf::from(&item.path));
        entries.insert(
            item.path.clone(),
            Entry {
                source: root.join(item.path),
                sha256: item.sha256,
                bytes: item.bytes,
            },
        );
    }
    crate::resume::validate_tree(root, &expected)?;
    for entry in entries.values() {
        if crate::resume::hash_file(&entry.source, entry.bytes)?
            != (entry.bytes, entry.sha256.clone())
        {
            return Err("input snapshot bytes differ from its inventory".into());
        }
    }
    for record in &records {
        record.selection.validate()?;
        let spec = Toolchain::parse(&record.selection.toolchain)?;
        // Historical moving-channel records bind to dated metadata, never today's root alias.
        let path = format!("dist/{}/channel-rust-{}.toml", record.date, spec.channel);
        safe_path(&path)?;
        let manifest = entries
            .get(&path)
            .ok_or("missing dated toolchain manifest")?;
        let hash = entries
            .get(&format!("{path}.sha256"))
            .ok_or("missing dated manifest checksum")?;
        if !entries.contains_key(&format!("{path}.asc")) {
            return Err("missing original signature file".into());
        }
        let bytes = read_file(&manifest.source, METADATA_LIMIT)?;
        if checksum(&read_file(&hash.source, 4096)?)? != sha256(&bytes) {
            return Err("source manifest checksum mismatch".into());
        }
        let metadata = super::Metadata {
            manifest: bytes.clone(),
            checksum: read_file(&hash.source, 4096)?,
            signature: read_file(&entries[&format!("{path}.asc")].source, 64 * 1024)?,
            bootstraps: record
                .selection
                .hosts
                .iter()
                .map(|host| {
                    let file = format!("{}.sha256", super::bootstrap_path(&rustup, host));
                    Ok((
                        host.clone(),
                        read_file(
                            &entries
                                .get(&file)
                                .ok_or("missing bootstrap sidecar")?
                                .source,
                            4096,
                        )?,
                    ))
                })
                .collect::<Result<_>>()?,
        };
        let expected = super::build_plan(
            &record.selection,
            &rustup,
            100 * 1024 * 1024 * 1024,
            &metadata,
        )?;
        for a in &expected.artifacts {
            if entries.get(&a.path).is_none_or(|e| e.sha256 != a.sha256) {
                return Err("source omits a selected artifact or has a different identity".into());
            }
        }
        let m = parsed(&bytes, &spec)?;
        let release = m
            .pkg
            .get("rust")
            .ok_or("missing rust package")?
            .version
            .split_whitespace()
            .next()
            .ok_or("missing Rust version")?;
        if m.date != record.date
            || release != record.release
            || record.pinned_toolchain != spec.pinned(&m.date)
        {
            return Err("toolchain record differs from preserved manifest".into());
        }
    }
    // A replacement root channel is a complete original triplet, never mixed metadata.
    for channel in ["stable", "beta", "nightly"] {
        let path = format!("dist/channel-rust-{channel}.toml");
        let names = [
            path.clone(),
            format!("{path}.sha256"),
            format!("{path}.asc"),
        ];
        let present = names.iter().filter(|p| entries.contains_key(*p)).count();
        if present != 0 {
            if present != 3 {
                return Err("incomplete moving-channel metadata triplet".into());
            }
            if checksum(&read_file(&entries[&names[1]].source, 4096)?)? != entries[&path].sha256 {
                return Err("moving-channel checksum mismatch".into());
            }
            let m = parsed(
                &read_file(&entries[&path].source, METADATA_LIMIT)?,
                &Toolchain::parse(channel)?,
            )?;
            let dated = format!("dist/{}/channel-rust-{channel}.toml", m.date);
            if entries
                .get(&dated)
                .is_none_or(|e| e.sha256 != entries[&path].sha256)
                || !records.iter().any(|r| {
                    r.date == m.date
                        && Toolchain::parse(&r.selection.toolchain)
                            .is_ok_and(|s| s.channel == channel)
                })
            {
                return Err("moving alias lacks matching dated toolchain record".into());
            }
        }
    }
    Ok(Input {
        rustup,
        records,
        files: entries,
        identity: sha256(&raw),
    })
}
/// Assemble a new generation. New inputs may replace only stable/beta/nightly root
/// metadata from `previous`; immutable path conflicts always refuse. No input is edited.
pub fn assemble(
    inputs: &[PathBuf],
    previous: Option<&Path>,
    output: &Path,
    max_bytes: u64,
) -> Result<PathBuf> {
    if inputs.is_empty()
        || inputs.len() > 32
        || max_bytes == 0
        || max_bytes > 100 * 1024 * 1024 * 1024
    {
        return Err("use 1-32 inputs and a 1 byte through 100 GiB generation budget".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let target = parent.canonicalize()?.join(
        output
            .file_name()
            .ok_or("output needs a new directory name")?,
    );
    for root in inputs.iter().map(PathBuf::as_path).chain(previous) {
        if target.starts_with(root.canonicalize()?) {
            return Err("output must not be inside an input snapshot".into());
        }
    }
    let (inventory, files) = prepare(inputs, previous, max_bytes)?;
    fs::create_dir(output)?;
    let staging = tempfile::Builder::new()
        .prefix(".assembling-")
        .tempdir_in(output)?;
    write_prepared(&inventory, &files, staging.path())?;
    let completed = output.join("mirror");
    if fs::symlink_metadata(&completed).is_ok() {
        return Err("completed generation unexpectedly exists".into());
    }
    fs::rename(staging.path(), &completed)?;
    Ok(completed)
}

pub(super) fn prepare(
    inputs: &[PathBuf],
    previous: Option<&Path>,
    max_bytes: u64,
) -> Result<(CollectionSnapshot, BTreeMap<String, Entry>)> {
    if inputs.is_empty()
        || inputs.len() > 32
        || max_bytes == 0
        || max_bytes > 100 * 1024 * 1024 * 1024
    {
        return Err("invalid collection input/budget".into());
    }
    let old = previous.map(read_input).transpose()?;
    let sources: Vec<_> = inputs
        .iter()
        .map(|p| read_input(p))
        .collect::<Result<_>>()?;
    let rustup = sources[0].rustup.clone();
    let mut files = BTreeMap::new();
    let mut records = Vec::new();
    let mut identities = BTreeSet::new();
    if let Some(old) = old {
        if old.rustup != rustup {
            return Err("all inputs must use the same Rustup bootstrap version".into());
        }
        files = old.files;
        records = old.records;
        identities.insert(old.identity);
    }
    let mut incoming: BTreeMap<String, Entry> = BTreeMap::new();
    for source in sources {
        if source.rustup != rustup {
            return Err("all inputs must use the same Rustup bootstrap version".into());
        }
        identities.insert(source.identity);
        for record in source.records {
            if !records.contains(&record) {
                records.push(record);
            }
        }
        for (path, entry) in source.files {
            if let Some(prior) = incoming.get(&path) {
                if prior.sha256 != entry.sha256 || prior.bytes != entry.bytes {
                    return Err(format!(
                        "conflicting new inputs at {path}; resolve a coherent set"
                    )
                    .into());
                }
            }
            incoming.insert(path, entry);
        }
    }
    for (path, entry) in incoming {
        if let Some(prior) = files.get(&path) {
            if (prior.sha256 != entry.sha256 || prior.bytes != entry.bytes) && !rolling(&path) {
                return Err(format!("immutable retained path conflict: {path}").into());
            }
        }
        files.insert(path, entry);
    }
    if records.len() > 256 || files.len() > FILE_LIMIT {
        return Err("collection record/file limit exceeded".into());
    }
    let total = files.values().try_fold(0u64, |n, e| {
        n.checked_add(e.bytes).ok_or("collection size overflow")
    })?;
    if total > max_bytes {
        return Err("generation byte budget exceeded before output creation".into());
    }
    records.sort_by_key(|r| toml::to_string(r).unwrap_or_default());
    let inventory = CollectionSnapshot {
        schema: 3,
        rustup_version: rustup,
        toolchains: records,
        inputs: identities.into_iter().collect(),
        files: files
            .iter()
            .map(|(p, e)| SnapshotFile {
                path: p.clone(),
                sha256: e.sha256.clone(),
                bytes: e.bytes,
            })
            .collect(),
    };
    let serialized = toml::to_string_pretty(&inventory)?;
    if serialized.len() as u64 > INVENTORY_LIMIT {
        return Err("collection inventory exceeds 1 MiB".into());
    }
    Ok((inventory, files))
}
pub(super) fn write_prepared(
    inventory: &CollectionSnapshot,
    files: &BTreeMap<String, Entry>,
    staging: &Path,
) -> Result<()> {
    let serialized = toml::to_string_pretty(inventory)?;
    for (path, entry) in files {
        let target = staging.join(path);
        fs::create_dir_all(target.parent().ok_or("missing parent")?)?;
        let mut file = File::create_new(target)?;
        let mut writer = crate::HashWriter {
            file: &mut file,
            hash: sha2::Sha256::default(),
            bytes: 0,
        };
        crate::copy_bounded(File::open(&entry.source)?, &mut writer, entry.bytes)?;
        let count = writer.bytes;
        let hash = hex::encode(sha2::Digest::finalize(writer.hash));
        if count != entry.bytes || hash != entry.sha256 {
            return Err("input changed while assembling generation".into());
        }
        file.sync_all()?;
    }
    super::write_new(&staging.join("snapshot.toml"), serialized.as_bytes())?;
    Ok(())
}
