//! Complete a verified generation with pinned Rustup distribution endpoints.
use super::{
    collection::{self, CollectionSnapshot},
    read_file, Toolchain,
};
use crate::{bytes, checksum, sha256, Result, SnapshotFile, Source, ARTIFACT_LIMIT};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    path::{Path, PathBuf},
};

/// Copy a selected/collection snapshot into a fresh generation, add Rustup's
/// pinned catalog/installers, and explicitly promote dated channel triplets.
/// The catalog is generated mirror policy; Rust manifests and shell script bytes
/// remain upstream originals. No input or remote server is modified.
pub fn prepare(
    snapshot: &Path,
    rustup_version: &str,
    channels: &[String],
    output: &Path,
    max_bytes: u64,
    source: &impl Source,
) -> Result<PathBuf> {
    crate::version(rustup_version)?;
    if max_bytes == 0 || max_bytes > 100 * 1024 * 1024 * 1024 || channels.len() > 3 {
        return Err("invalid Rustup generation budget/channel count".into());
    }
    if output.symlink_metadata().is_ok() {
        return Err("Rustup output already exists; select a new path".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let target = parent
        .canonicalize()?
        .join(output.file_name().ok_or("output needs a directory name")?);
    if target.starts_with(snapshot.canonicalize()?) {
        return Err("Rustup output cannot be inside its input snapshot".into());
    }
    let input = collection::read_input(snapshot)?;
    let initial_bytes = input.files.values().try_fold(0u64, |n, e| {
        n.checked_add(e.bytes)
            .ok_or("Rustup input byte count overflow")
    })?;
    if initial_bytes > max_bytes {
        return Err("Rustup input already exceeds generation budget".into());
    }
    let hosts: BTreeSet<_> = input
        .records
        .iter()
        .flat_map(|r| r.selection.hosts.iter().cloned())
        .collect();
    if hosts.is_empty() || hosts.len() > 64 {
        return Err("Rustup host limit exceeded".into());
    }
    let mut metadata = BTreeMap::new();
    let catalog = format!("schema-version = '1'\nversion = '{rustup_version}'\n").into_bytes();
    metadata.insert("rustup/release-stable.toml".to_string(), catalog);
    metadata.insert(
        "rustup/rustup-init.sh".to_string(),
        bytes(source, "rustup/rustup-init.sh", 256 * 1024)?,
    );
    let mut installers = Vec::new();
    for host in hosts {
        let archive = super::bootstrap_path(rustup_version, &host);
        let sidecar = bytes(source, &format!("{archive}.sha256"), 4096)?;
        let hash = checksum(&sidecar)?;
        let current = format!(
            "rustup/dist/{host}/rustup-init{}",
            if host.contains("-windows-") {
                ".exe"
            } else {
                ""
            }
        );
        metadata.insert(format!("{archive}.sha256"), sidecar.clone());
        metadata.insert(format!("{current}.sha256"), sidecar);
        installers.push((archive, current, hash));
    }
    // Rustup clients accept both full and major.minor numeric selectors. Publish
    // policy aliases using the highest selected stable patch, preserving every
    // original signed manifest byte. Tool selection itself remains explicitly pinned.
    let mut minor = BTreeMap::new();
    for item in &input.records {
        if crate::version(&item.release).is_err() {
            continue;
        }
        let numbers = item
            .release
            .split('.')
            .map(str::parse::<u64>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let key = format!("{}.{}", numbers[0], numbers[1]);
        let spec = Toolchain::parse(&item.selection.toolchain)?;
        let canonical = format!("dist/channel-rust-{}.toml", item.release);
        let dated = format!("dist/{}/channel-rust-{}.toml", item.date, spec.channel);
        if let Some(existing) = input.files.get(&canonical) {
            if input
                .files
                .get(&dated)
                .is_none_or(|e| e.sha256 != existing.sha256)
            {
                return Err("ambiguous canonical manifest for a numeric alias".into());
            }
        }
        let stem = if input.files.contains_key(&canonical) {
            canonical.clone()
        } else {
            format!("dist/{}/channel-rust-{}.toml", item.date, spec.channel)
        };
        if !input.files.contains_key(&canonical) {
            for suffix in ["", ".sha256", ".asc"] {
                let name = format!("{stem}{suffix}");
                let entry = input
                    .files
                    .get(&name)
                    .ok_or("numeric alias lacks original metadata")?;
                metadata.insert(
                    format!("{canonical}{suffix}"),
                    read_file(&entry.source, crate::METADATA_LIMIT)?,
                );
            }
        }
        if minor
            .get(&key)
            .is_none_or(|(patch, _, _): &(u64, String, String)| numbers[2] > *patch)
        {
            minor.insert(key, (numbers[2], item.release.clone(), stem));
        }
    }
    let mut alias_catalog = BTreeMap::new();
    for (alias, (_, release, stem)) in minor {
        for suffix in ["", ".sha256", ".asc"] {
            let entry = input
                .files
                .get(&format!("{stem}{suffix}"))
                .ok_or("minor alias lacks original metadata")?;
            metadata.insert(
                format!("dist/channel-rust-{alias}.toml{suffix}"),
                read_file(&entry.source, crate::METADATA_LIMIT)?,
            );
        }
        alias_catalog.insert(alias, release);
    }
    metadata.insert(
        "rustup/toolchain-aliases.toml".into(),
        format!(
            "schema = 1\n[aliases]\n{}",
            toml::to_string(&alias_catalog)?
        )
        .into_bytes(),
    );
    let mut seen = BTreeSet::new();
    for requested in channels {
        let spec = Toolchain::parse(requested)?;
        if !["stable", "beta", "nightly"].contains(&spec.channel.as_str())
            || spec.date.is_none()
            || !seen.insert(spec.channel.clone())
        {
            return Err(
                "promote one explicit dated selector per channel, e.g. stable-2025-09-18".into(),
            );
        }
        let date = spec.date.as_deref().unwrap();
        if !input.records.iter().any(|r| {
            r.date == date
                && Toolchain::parse(&r.selection.toolchain).is_ok_and(|s| s.channel == spec.channel)
        }) {
            return Err("promotion requires a matching verified dated channel selection".into());
        }
        for suffix in ["", ".sha256", ".asc"] {
            let from = format!("dist/{date}/channel-rust-{}.toml{suffix}", spec.channel);
            let to = format!("dist/channel-rust-{}.toml{suffix}", spec.channel);
            let item = input
                .files
                .get(&from)
                .ok_or("promotion lacks original metadata triplet")?;
            metadata.insert(to, read_file(&item.source, crate::METADATA_LIMIT)?);
        }
    }
    // Every metadata decision is frozen before installer payload transfers.
    fs::create_dir(output)?;
    let stage = tempfile::Builder::new()
        .prefix(".rustup-")
        .tempdir_in(output)?;
    let root = stage.path();
    let mut entries = BTreeMap::new();
    let mut total = 0u64;
    for (name, entry) in &input.files {
        let dest = root.join(name);
        fs::create_dir_all(dest.parent().unwrap())?;
        let mut out = File::create(&dest)?;
        crate::copy_bounded(File::open(&entry.source)?, &mut out, entry.bytes)?;
        out.sync_all()?;
        if crate::resume::hash_file(&dest, ARTIFACT_LIMIT)? != (entry.bytes, entry.sha256.clone()) {
            return Err("input changed during Rustup generation copy".into());
        }
        record(
            &mut entries,
            name,
            entry.bytes,
            entry.sha256.clone(),
            &mut total,
            max_bytes,
        )?;
    }
    for (archive, current, expected) in installers {
        if entries
            .get(&archive)
            .is_none_or(|f: &SnapshotFile| f.sha256 != expected)
        {
            let path = root.join(&archive);
            if entries.contains_key(&archive) {
                return Err("immutable Rustup archive checksum conflict".into());
            }
            fs::create_dir_all(path.parent().unwrap())?;
            let mut out = File::create(&path)?;
            source.fetch(
                &format!("{}{archive}", crate::ORIGIN),
                &mut out,
                (max_bytes - total).min(ARTIFACT_LIMIT),
            )?;
            out.sync_all()?;
            let (n, actual) = crate::resume::hash_file(&path, ARTIFACT_LIMIT)?;
            if actual != expected {
                return Err("Rustup installer checksum mismatch".into());
            }
            record(&mut entries, &archive, n, actual, &mut total, max_bytes)?;
        }
        let entry = &entries[&archive];
        let (n, hash) = (entry.bytes, entry.sha256.clone());
        record(&mut entries, &current, n, hash, &mut total, max_bytes)?;
        let path = root.join(&current);
        fs::create_dir_all(path.parent().unwrap())?;
        let mut out = File::create(&path)?;
        crate::copy_bounded(File::open(root.join(&archive))?, &mut out, n)?;
        out.sync_all()?;
    }
    for (name, data) in metadata {
        let dest = root.join(&name);
        fs::create_dir_all(dest.parent().unwrap())?;
        record(
            &mut entries,
            &name,
            data.len() as u64,
            sha256(&data),
            &mut total,
            max_bytes,
        )?;
        {
            use std::io::Write;
            let mut f = File::create(&dest)?;
            f.write_all(&data)?;
            f.sync_all()?;
        }
    }
    let inventory = CollectionSnapshot {
        schema: 3,
        rustup_version: rustup_version.into(),
        toolchains: input.records,
        inputs: vec![input.identity],
        files: entries.into_values().collect(),
    };
    let raw = toml::to_string_pretty(&inventory)?;
    if raw.len() > 1024 * 1024 {
        return Err("Rustup inventory exceeds byte limit".into());
    }
    super::write_new(&root.join("snapshot.toml"), raw.as_bytes())?;
    collection::read_input(root)?;
    let completed = output.join("mirror");
    fs::rename(root, &completed)?;
    Ok(completed)
}
fn record(
    files: &mut BTreeMap<String, SnapshotFile>,
    name: &str,
    bytes: u64,
    hash: String,
    total: &mut u64,
    budget: u64,
) -> Result<()> {
    let prior = files.get(name).map(|f| f.bytes).unwrap_or(0);
    *total = total
        .checked_sub(prior)
        .and_then(|v| v.checked_add(bytes))
        .ok_or("Rustup byte count overflow")?;
    if *total > budget || (!files.contains_key(name) && files.len() >= 4096) {
        return Err("Rustup generation exceeds byte/file budget".into());
    }
    files.insert(
        name.into(),
        SnapshotFile {
            path: name.into(),
            bytes,
            sha256: hash,
        },
    );
    Ok(())
}
