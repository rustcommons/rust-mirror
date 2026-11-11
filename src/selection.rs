//! Saved selections and metadata-only, replayable download plans.
//!
//! Start with [Config::parse] or [Config::read]. [catalog] describes upstream
//! availability. [resolve] freezes metadata before [ResolvedPlan::save] and [fetch]
//! transfer its payloads. For all selections together use [sync::run], then
//! complete Rustup endpoints with [rustup::prepare].
pub mod collection;
pub mod retention;
pub mod rustup;
pub mod sync;
use crate::{
    bytes, checksum, safe_path, sha256, valid_hash, version, Manifest, Result, SnapshotFile,
    Source, ARTIFACT_LIMIT, METADATA_LIMIT, ORIGIN,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const SMALL_LIMIT: u64 = 1024 * 1024;
const MAX_FILES: usize = 1024;
fn default_profile() -> String {
    "default".into()
}
fn default_budget() -> u64 {
    10 * 1024 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Validated mirror configuration, normally parsed from TOML.
///
/// [`Config::parse`] and [`Config::read`] validate automatically. Directly constructed
/// values should pass [`Config::validate`] before use. Unknown TOML fields are refused.
pub struct Config {
    /// Configuration schema; currently `1`.
    pub schema: u32,
    /// Pinned full numeric Rustup manager version, not the Rust compiler version.
    pub rustup_version: String,
    #[serde(default = "default_budget")]
    /// Transfer/copy budget in bytes; defaults to 10 GiB, accepts 1 byte through 100 GiB. Not a disk quota.
    pub max_download_bytes: u64,
    /// One through 32 uniquely named selections.
    pub selections: Vec<Selection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional explicit generation retention; absent disables pruning. Sync never prunes.
    pub retention: Option<retention::Policy>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// One named compiler/platform/component choice within a [`Config`].
pub struct Selection {
    /// Unique nonblank selection label, at most 80 UTF-8 bytes with no control characters.
    pub name: String,
    /// Full numeric release, channel or dated channel; for example `1.90.0` or `stable-2025-09-18`.
    pub toolchain: String,
    /// One through 16 unique compiler host triples; no implicit native-host selection.
    pub hosts: Vec<String>,
    #[serde(default = "default_profile")]
    /// Upstream `minimal` or `default` profile; defaults to `default`.
    pub profile: String,
    #[serde(default)]
    /// Additional unique component names or accepted aliases, at most 64. Defaults to empty.
    pub components: Vec<String>,
    #[serde(default)]
    /// Additional unique compilation target triples, at most 64. Defaults to empty.
    pub targets: Vec<String>,
}
fn atom(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || s == "."
        || s == ".."
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(format!("invalid selection identifier: {s}").into());
    }
    Ok(())
}
fn unique(values: &[String], limit: usize) -> Result<()> {
    if values.len() > limit {
        return Err(
            crate::Error::new(crate::ErrorKind::Budget, "selection list exceeds budget").into(),
        );
    }
    let mut seen = BTreeSet::new();
    for value in values {
        atom(value)?;
        if !seen.insert(value) {
            return Err(format!("duplicate selection: {value}").into());
        }
    }
    Ok(())
}
fn valid_date(s: &str) -> bool {
    let p: Vec<_> = s.split('-').collect();
    if p.len() != 3
        || p[0].len() != 4
        || p[1].len() != 2
        || p[2].len() != 2
        || !p.iter().all(|s| s.bytes().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        p[0].parse::<u32>(),
        p[1].parse::<u32>(),
        p[2].parse::<u32>(),
    ) else {
        return false;
    };
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 400 == 0 || (y % 4 == 0 && y % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y >= 2010 && d > 0 && d <= days
}
#[derive(Debug)]
struct Toolchain {
    channel: String,
    date: Option<String>,
}
impl Toolchain {
    fn parse(value: &str) -> Result<Self> {
        for channel in ["stable", "beta", "nightly"] {
            if value == channel {
                return Ok(Self {
                    channel: channel.into(),
                    date: None,
                });
            }
            if let Some(date) = value.strip_prefix(&format!("{channel}-")) {
                if !valid_date(date) {
                    return Err("expected a calendar date YYYY-MM-DD".into());
                }
                return Ok(Self {
                    channel: channel.into(),
                    date: Some(date.into()),
                });
            }
        }
        version(value)?;
        Ok(Self {
            channel: value.into(),
            date: None,
        })
    }
    fn manifest_path(&self) -> String {
        match &self.date {
            Some(d) => format!("dist/{d}/channel-rust-{}.toml", self.channel),
            None => format!("dist/channel-rust-{}.toml", self.channel),
        }
    }
    fn pinned(&self, date: &str) -> String {
        if ["stable", "beta", "nightly"].contains(&self.channel.as_str()) {
            format!("{}-{date}", self.channel)
        } else {
            self.channel.clone()
        }
    }
}
impl Selection {
    /// Check selector syntax, list uniqueness and supported profile, without network access.
    ///
    /// # Errors
    /// Refuses invalid names/selectors, empty or duplicate hosts, unsupported profiles
    /// or component/target count/identifier limits. This does not prove upstream availability.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("invalid selection name".into());
        }
        Toolchain::parse(&self.toolchain)?;
        if self.hosts.is_empty() {
            return Err("select at least one explicit host platform".into());
        }
        unique(&self.hosts, 16)?;
        unique(&self.components, 64)?;
        unique(&self.targets, 64)?;
        if !["minimal", "default"].contains(&self.profile.as_str()) {
            return Err("use profile minimal or default plus explicit components; complete is unreliable and is not supported".into());
        }
        Ok(())
    }
}
impl Config {
    /// Parse strict TOML and validate all configuration/selection constraints.
    ///
    /// # Errors
    /// Refuses malformed/unknown fields, unsupported schema, invalid manager version/budget
    /// or missing/duplicate/invalid selections.
    pub fn parse(text: &str) -> Result<Self> {
        let c: Self = toml::from_str(text)?;
        c.validate()?;
        Ok(c)
    }
    /// Read up to 1 MiB of UTF-8 configuration TOML and call [`Self::parse`].
    ///
    /// # Errors
    /// Refuses missing/non-file/oversized input, invalid UTF-8 or invalid configuration.
    pub fn read(path: &Path) -> Result<Self> {
        Self::parse(std::str::from_utf8(&read_file(path, SMALL_LIMIT)?)?)
    }
    /// Check schema, Rustup version, budget and 1 to 32 uniquely named selections.
    ///
    /// Calls [`Selection::validate`] for each selection. This is local syntax/policy
    /// validation, not a download or native-platform check.
    ///
    /// # Errors
    /// Returns an error if any configuration or selection constraint is violated.
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            return Err("unsupported configuration schema".into());
        }
        version(&self.rustup_version)?;
        if self.max_download_bytes == 0 || self.max_download_bytes > 100 * 1024 * 1024 * 1024 {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "download budget must be 1 byte to 100 GiB",
            )
            .into());
        }
        if self.selections.is_empty() || self.selections.len() > 32 {
            return Err("select 1 to 32 named selections".into());
        }
        if let Some(policy) = &self.retention {
            policy.validate()?;
        }
        let mut names = BTreeSet::new();
        for s in &self.selections {
            s.validate()?;
            if !names.insert(&s.name) {
                return Err("selection names must be unique".into());
            }
        }
        Ok(())
    }
    /// Find one named selection without network access.
    ///
    /// # Errors
    /// Returns an error if no selection has exactly this name.
    pub fn get(&self, name: &str) -> Result<&Selection> {
        self.selections
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| format!("unknown selection: {name}").into())
    }
}
fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if !path.is_file() {
        return Err("local input must be a regular file".into());
    }
    let mut data = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(
            crate::Error::new(crate::ErrorKind::Budget, "local file exceeds budget").into(),
        );
    }
    Ok(data)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// One frozen archive/installer request in a [`Plan`].
///
/// The plan binds a relative upstream path to a whole-file hash; it does not contain payload bytes.
pub struct Artifact {
    /// Relative official-distribution path requested for this artifact.
    pub path: String,
    /// Expected SHA-256 of the complete payload.
    pub sha256: String,
    /// Manifest package name, or `rustup-init` for a bootstrap installer.
    pub package: String,
    /// Manifest target/host identifier for this payload.
    pub target: String,
    /// Artifact category: `component` for tool archives or `bootstrap` for Rustup.
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Serializable metadata-only download plan, stored as `plan.toml`.
///
/// Use [`resolve`] to construct it and [`ResolvedPlan::save`] to persist its original
/// metadata. [`load_plan`] revalidates saved metadata and reconstructs the plan.
pub struct Plan {
    /// Saved-plan schema number; currently `1`.
    pub schema: u32,
    /// Planner recipe revision checked when replaying a saved plan.
    pub planner_revision: u32,
    /// Pinned bootstrap manager release.
    pub rustup_version: String,
    /// Frozen download/copy budget in bytes.
    pub max_download_bytes: u64,
    /// Exact named selection used during resolution.
    pub selection: Selection,
    /// Full numeric compiler release reported by the manifest.
    pub resolved_release: String,
    /// Resolved upstream release date.
    pub date: String,
    /// Stable replay selector, such as a numeric release or a dated channel.
    pub pinned_toolchain: String,
    /// Relative upstream manifest path used for resolution.
    pub metadata_source: String,
    /// SHA-256 of the original manifest bytes.
    pub manifest_sha256: String,
    /// SHA-256 of the checksum-sidecar bytes, not another copy of the manifest hash.
    pub checksum_sha256: String,
    /// SHA-256 of retained signature bytes; not a signature-verification result.
    pub signature_sha256: String,
    /// Host-to-hash map for bootstrap checksum-sidecar bytes, not installer payload hashes.
    pub bootstrap_checksums: BTreeMap<String, String>,
    /// Frozen payload requests, including every advertised format for selected components.
    pub artifacts: Vec<Artifact>,
}
/// A [`Plan`] together with its original manifest, sidecars and bootstrap checksums.
///
/// The metadata bytes are private so saved/replayed plans retain their validation boundary.
pub struct ResolvedPlan {
    /// Serializable preview; edits are not a substitute for fresh validated resolution.
    pub plan: Plan,
    metadata: Metadata,
}
struct Metadata {
    manifest: Vec<u8>,
    checksum: Vec<u8>,
    signature: Vec<u8>,
    bootstraps: BTreeMap<String, Vec<u8>>,
}
fn parsed(raw: &[u8], spec: &Toolchain) -> Result<Manifest> {
    let m: Manifest = toml::from_str(std::str::from_utf8(raw)?)?;
    if m.manifest_version != "2" || !valid_date(&m.date) {
        return Err("unsupported manifest schema or date".into());
    }
    if spec.date.as_ref().is_some_and(|d| d != &m.date) {
        return Err(crate::Error::new(
            crate::ErrorKind::Integrity,
            "manifest date differs from requested date",
        )
        .into());
    }
    let release = m
        .pkg
        .get("rust")
        .ok_or("missing rust metapackage")?
        .version
        .split_whitespace()
        .next()
        .ok_or("missing Rust version")?;
    match spec.channel.as_str() {
        "stable" => version(release)?,
        "nightly" => version(
            release
                .strip_suffix("-nightly")
                .ok_or("nightly endpoint returned a different channel")?,
        )?,
        "beta" => {
            let (number, suffix) = release
                .split_once("-beta")
                .ok_or("beta endpoint returned a different channel")?;
            version(number)?;
            if !suffix.is_empty()
                && (!suffix.starts_with('.')
                    || suffix.len() < 2
                    || !suffix[1..].bytes().all(|b| b.is_ascii_digit()))
            {
                return Err("invalid beta release identity".into());
            }
        }
        _ => {
            if release != spec.channel {
                return Err("manifest does not match requested Rust version".into());
            }
        }
    }
    Ok(m)
}
fn canonical(m: &Manifest, name: &str) -> Result<String> {
    let mut name = name.to_string();
    let mut seen = BTreeSet::new();
    while let Some(rename) = m.renames.get(&name) {
        if !seen.insert(name.clone()) || seen.len() > 8 {
            return Err("cyclic or excessive component aliases".into());
        }
        name = rename.to.clone();
    }
    atom(&name)?;
    Ok(name)
}
fn add_artifact(map: &mut BTreeMap<String, Artifact>, a: Artifact) -> Result<()> {
    safe_path(&a.path)?;
    valid_hash(&a.sha256)?;
    if let Some(old) = map.get(&a.path) {
        if old.sha256 != a.sha256 {
            return Err("conflicting artifact identities".into());
        }
    } else {
        map.insert(a.path.clone(), a);
    }
    if map.len() > MAX_FILES {
        return Err(
            crate::Error::new(crate::ErrorKind::Budget, "plan exceeds artifact budget").into(),
        );
    }
    Ok(())
}
fn build_plan(selection: &Selection, rustup: &str, budget: u64, meta: &Metadata) -> Result<Plan> {
    selection.validate()?;
    version(rustup)?;
    let spec = Toolchain::parse(&selection.toolchain)?;
    let m = parsed(&meta.manifest, &spec)?;
    if sha256(&meta.manifest) != checksum(&meta.checksum)? {
        return Err(
            crate::Error::new(crate::ErrorKind::Integrity, "manifest checksum mismatch").into(),
        );
    }
    let mut artifacts = BTreeMap::new();
    let root = m.pkg.get("rust").ok_or("missing rust package")?;
    let profile = m
        .profiles
        .get(&selection.profile)
        .ok_or("requested profile absent in this release")?;
    for host in &selection.hosts {
        let packages = root
            .target
            .get(host)
            .filter(|t| t.available)
            .ok_or_else(|| format!("host unavailable for {}: {host}", selection.toolchain))?;
        let mut wanted = BTreeSet::new();
        for input in profile.iter().chain(&selection.components) {
            let package = canonical(&m, input)?;
            let component = packages
                .components
                .iter()
                .chain(&packages.extensions)
                .find(|c| c.pkg == package && (c.target == *host || c.target == "*"));
            if let Some(c) = component {
                wanted.insert((c.pkg.clone(), c.target.clone()));
            } else if selection.components.contains(input) {
                return Err(
                    format!("requested component {input} is not provided for {host}").into(),
                );
            }
        }
        for required in ["rustc", "cargo", "rust-std"] {
            if !wanted.iter().any(|(p, _)| p == required) {
                return Err(format!("profile lacks required {required} for {host}").into());
            }
        }
        for target in &selection.targets {
            let component = packages
                .components
                .iter()
                .chain(&packages.extensions)
                .find(|c| c.pkg == "rust-std" && c.target == *target)
                .ok_or_else(|| format!("target {target} not advertised for host {host}"))?;
            wanted.insert((component.pkg.clone(), component.target.clone()));
        }
        for (package, target) in wanted {
            let p = m.pkg.get(&package).ok_or("dangling component package")?;
            let t = p
                .target
                .get(&target)
                .or_else(|| p.target.get("*"))
                .filter(|t| t.available)
                .ok_or_else(|| {
                    format!(
                        "component {package} for {target} unavailable in {}",
                        selection.toolchain
                    )
                })?;
            let variants = [
                (&t.zst_url, &t.zst_hash),
                (&t.xz_url, &t.xz_hash),
                (&t.url, &t.hash),
            ];
            let mut found = false;
            for (url, hash) in variants {
                let (url, hash) = match (url, hash) {
                    (Some(url), Some(hash)) => (url, hash),
                    (None, None) => continue,
                    _ => {
                        return Err(crate::Error::new(
                            crate::ErrorKind::Integrity,
                            "archive URL/checksum pair is incomplete",
                        )
                        .into())
                    }
                };
                found = true;
                let path = url.strip_prefix(ORIGIN).ok_or("foreign artifact origin")?;
                safe_path(path)?;
                if !path.starts_with(&format!("dist/{}/", m.date)) {
                    return Err("artifact outside manifest release directory".into());
                }
                valid_hash(hash)?;
                add_artifact(
                    &mut artifacts,
                    Artifact {
                        path: path.into(),
                        sha256: hash.to_ascii_lowercase(),
                        package: package.clone(),
                        target: target.clone(),
                        kind: "component".into(),
                    },
                )?;
            }
            if !found {
                return Err("available component has no downloadable archives".into());
            }
        }
        let sha = meta.bootstraps.get(host).ok_or_else(|| {
            crate::Error::new(
                crate::ErrorKind::Integrity,
                "missing bootstrap checksum metadata",
            )
        })?;
        let path = bootstrap_path(rustup, host);
        add_artifact(
            &mut artifacts,
            Artifact {
                path,
                sha256: checksum(sha)?,
                package: "rustup-init".into(),
                target: host.clone(),
                kind: "bootstrap".into(),
            },
        )?;
    }
    Ok(Plan {
        schema: 1,
        planner_revision: 1,
        rustup_version: rustup.into(),
        max_download_bytes: budget,
        selection: selection.clone(),
        resolved_release: root
            .version
            .split_whitespace()
            .next()
            .ok_or("missing version")?
            .into(),
        date: m.date.clone(),
        pinned_toolchain: spec.pinned(&m.date),
        metadata_source: spec.manifest_path(),
        manifest_sha256: sha256(&meta.manifest),
        checksum_sha256: sha256(&meta.checksum),
        signature_sha256: sha256(&meta.signature),
        bootstrap_checksums: meta
            .bootstraps
            .iter()
            .map(|(h, b)| (h.clone(), sha256(b)))
            .collect(),
        artifacts: artifacts.into_values().collect(),
    })
}
fn bootstrap_path(version: &str, host: &str) -> String {
    format!(
        "rustup/archive/{version}/{host}/rustup-init{}",
        if host.contains("-windows-") {
            ".exe"
        } else {
            ""
        }
    )
}
fn metadata(toolchain: &str, source: &impl Source) -> Result<Metadata> {
    let spec = Toolchain::parse(toolchain)?;
    let path = spec.manifest_path();
    let hash = bytes(source, &format!("{path}.sha256"), 4096)?;
    let raw = bytes(source, &path, METADATA_LIMIT)?;
    if sha256(&raw) != checksum(&hash)? {
        return Err(crate::Error::new(
            crate::ErrorKind::Integrity,
            "upstream manifest checksum mismatch; retry resolution",
        )
        .into());
    }
    parsed(&raw, &spec)?;
    let sig = bytes(source, &format!("{path}.asc"), 64 * 1024)?;
    Ok(Metadata {
        manifest: raw,
        checksum: hash,
        signature: sig,
        bootstraps: BTreeMap::new(),
    })
}
/// Resolve `name` from `config`, preserving original metadata and freezing payload hashes.
///
/// Reads manifests, checksum/signature sidecars and bootstrap checksums through
/// `source`. No compiler archive is downloaded and no files are written. Follow with
/// [`ResolvedPlan::save`] and [`fetch`] to transfer payloads.
///
/// # Errors
/// Refuses invalid config, unknown selection, unavailable components/hosts/targets,
/// malformed/incoherent metadata, or transport/size failures.
pub fn resolve(config: &Config, name: &str, source: &impl Source) -> Result<ResolvedPlan> {
    config.validate()?;
    let selected = config.get(name)?;
    let mut meta = metadata(&selected.toolchain, source)?;
    // Validate the component/host closure before asking for bootstrap metadata.
    for host in &selected.hosts {
        meta.bootstraps
            .insert(host.clone(), format!("{}\n", "0".repeat(64)).into_bytes());
    }
    build_plan(
        selected,
        &config.rustup_version,
        config.max_download_bytes,
        &meta,
    )?;
    for host in &selected.hosts {
        meta.bootstraps.insert(
            host.clone(),
            bytes(
                source,
                &format!("{}.sha256", bootstrap_path(&config.rustup_version, host)),
                4096,
            )?,
        );
    }
    let plan = build_plan(
        selected,
        &config.rustup_version,
        config.max_download_bytes,
        &meta,
    )?;
    Ok(ResolvedPlan {
        plan,
        metadata: meta,
    })
}
fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    crate::events::artifact(&path.to_string_lossy(), "copy", 0, Some(data.len() as u64))?;
    let mut f = File::options().write(true).create_new(true).open(path)?;
    crate::events::ProgressWriter {
        inner: &mut f,
        path: path.to_string_lossy().into(),
        count: 0,
    }
    .write_all(data)?;
    f.sync_all()?;
    Ok(())
}
impl ResolvedPlan {
    /// Persist only metadata and a preview/lock; no component archive is downloaded.
    /// Write `plan.toml` and original metadata into a new `output` directory.
    ///
    /// The parent must exist. No overwrite/resume mode or component-archive download
    /// is performed. A failed write can leave the reserved output directory.
    ///
    /// # Errors
    /// Returns an error for existing output, unsuitable parent, serialization or I/O failure.
    pub fn save(&self, output: &Path) -> Result<()> {
        fs::create_dir(output)?;
        write_new(&output.join("manifest.toml"), &self.metadata.manifest)?;
        write_new(&output.join("manifest.sha256"), &self.metadata.checksum)?;
        write_new(&output.join("manifest.asc"), &self.metadata.signature)?;
        for (i, host) in self.plan.selection.hosts.iter().enumerate() {
            write_new(
                &output.join(format!("bootstrap-{i}.sha256")),
                &self.metadata.bootstraps[host],
            )?;
        }
        write_new(
            &output.join("plan.toml"),
            toml::to_string_pretty(&self.plan)?.as_bytes(),
        )?;
        Ok(())
    }
}
fn cache_read(root: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let p = root.join(name);
    if fs::symlink_metadata(&p)?.file_type().is_symlink() || !p.is_file() {
        return Err("plan metadata must be regular files".into());
    }
    read_file(&p, limit)
}
/// Load and revalidate a saved plan directory without network access.
///
/// Checks metadata hashes and replay identity, then rebuilds the expected closure.
/// Use this rather than trusting a deserialized [`Plan`] alone.
///
/// # Errors
/// Refuses linked/missing/oversized files, unsupported schema/revision, invalid hashes
/// or a saved plan that disagrees with its reconstructed metadata.
pub fn load_plan(root: &Path) -> Result<ResolvedPlan> {
    if fs::symlink_metadata(root)?.file_type().is_symlink() || !root.is_dir() {
        return Err("plan must be a real directory".into());
    }
    let plan: Plan = toml::from_str(std::str::from_utf8(&cache_read(
        root,
        "plan.toml",
        SMALL_LIMIT,
    )?)?)?;
    if plan.schema != 1 || plan.planner_revision != 1 {
        return Err("unsupported frozen plan schema/revision".into());
    }
    Config {
        schema: 1,
        rustup_version: plan.rustup_version.clone(),
        max_download_bytes: plan.max_download_bytes,
        selections: vec![plan.selection.clone()],
        retention: None,
    }
    .validate()?;
    let mut meta = Metadata {
        manifest: cache_read(root, "manifest.toml", METADATA_LIMIT)?,
        checksum: cache_read(root, "manifest.sha256", 4096)?,
        signature: cache_read(root, "manifest.asc", 64 * 1024)?,
        bootstraps: BTreeMap::new(),
    };
    let mut expected: BTreeSet<String> = [
        "plan.toml",
        "manifest.toml",
        "manifest.sha256",
        "manifest.asc",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for (i, host) in plan.selection.hosts.iter().enumerate() {
        let name = format!("bootstrap-{i}.sha256");
        meta.bootstraps
            .insert(host.clone(), cache_read(root, &name, 4096)?);
        expected.insert(name);
    }
    for item in fs::read_dir(root)? {
        let item = item?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| "invalid plan file name")?;
        if !expected.remove(&name) {
            return Err("unexpected plan files".into());
        }
    }
    if !expected.is_empty() {
        return Err("incomplete plan directory".into());
    }
    if build_plan(
        &plan.selection,
        &plan.rustup_version,
        plan.max_download_bytes,
        &meta,
    )? != plan
    {
        return Err(crate::Error::new(
            crate::ErrorKind::Integrity,
            "frozen plan differs from cached metadata; resolve a new plan",
        )
        .into());
    }
    Ok(ResolvedPlan {
        plan,
        metadata: meta,
    })
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Schema-2 inventory describing one completed selected toolchain snapshot.
///
/// Written as `snapshot.toml`; input validation additionally checks file hashes and
/// the selected manifest closure. Deserialization alone does not establish readiness.
pub struct SelectionSnapshot {
    /// Inventory schema number; selected snapshots use `2`.
    pub schema: u32,
    /// Resolved full numeric compiler release.
    pub release: String,
    /// Original upstream manifest date.
    pub date: String,
    /// Frozen numeric or dated-channel selector.
    pub pinned_toolchain: String,
    /// Bootstrap manager version used by this snapshot.
    pub rustup_version: String,
    /// Exact requested host/profile/component/target choice.
    pub selection: Selection,
    /// Verified payload/metadata entries, excluding the inventory itself.
    pub files: Vec<SnapshotFile>,
}
fn snapshot_metadata(resolved: &ResolvedPlan) -> Result<BTreeMap<String, &Vec<u8>>> {
    let plan = &resolved.plan;
    let meta = &resolved.metadata;
    let spec = Toolchain::parse(&plan.selection.toolchain)?;
    let mut files = BTreeMap::new();
    for path in [
        plan.metadata_source.clone(),
        format!("dist/{}/channel-rust-{}.toml", plan.date, spec.channel),
    ] {
        files.insert(path.clone(), &meta.manifest);
        files.insert(format!("{path}.sha256"), &meta.checksum);
        files.insert(format!("{path}.asc"), &meta.signature);
    }
    for host in &plan.selection.hosts {
        files.insert(
            format!("{}.sha256", bootstrap_path(&plan.rustup_version, host)),
            &meta.bootstraps[host],
        );
    }
    Ok(files)
}
fn snapshot(plan: &Plan, files: Vec<SnapshotFile>) -> SelectionSnapshot {
    SelectionSnapshot {
        schema: 2,
        release: plan.resolved_release.clone(),
        date: plan.date.clone(),
        pinned_toolchain: plan.pinned_toolchain.clone(),
        rustup_version: plan.rustup_version.clone(),
        selection: plan.selection.clone(),
        files,
    }
}
/// Fetch a frozen plan into a new output; retain private progress if interrupted.
/// Return `<output>/mirror` only after frozen payloads and final-tree verification.
///
/// `plan_dir` comes from [`ResolvedPlan::save`]. Output must be new with an existing
/// parent. Interrupted private progress may continue through [`fetch_resuming`];
/// never serve that working state.
///
/// # Errors
/// Refuses existing output, invalid plans, competing writers, transport failures,
/// size/hash mismatches or local I/O/assembly failures.
pub fn fetch(plan_dir: &Path, output: &Path, source: &impl Source) -> Result<PathBuf> {
    fetch_with_resume(plan_dir, output, source, false, None)
}
/// Explicitly reopen an existing output for this exact frozen plan. Reuse requires
/// whole-file verification; a valid completed snapshot is checked, never overwritten.
/// Return the verified completed mirror from initialized matching progress.
///
/// Rehashes reusable files and verifies ranges against whole-file hashes. It does
/// not refresh metadata or overwrite completed content.
///
/// # Errors
/// Refuses uninitialized/foreign state, plan mismatch, unknown paths/links, locks,
/// invalid ranges, size/hash mismatches or I/O failures.
pub fn fetch_resuming(plan_dir: &Path, output: &Path, source: &impl Source) -> Result<PathBuf> {
    fetch_with_resume(plan_dir, output, source, true, None)
}
fn fetch_limited(
    plan: &Path,
    output: &Path,
    source: &impl Source,
    resume: bool,
    limit: u64,
) -> Result<PathBuf> {
    fetch_with_resume(plan, output, source, resume, Some(limit))
}
fn fetch_with_resume(
    plan_dir: &Path,
    output: &Path,
    source: &impl Source,
    resume: bool,
    limit: Option<u64>,
) -> Result<PathBuf> {
    let resolved = load_plan(plan_dir)?;
    let plan = &resolved.plan;
    let metadata = snapshot_metadata(&resolved)?;
    let expected: BTreeSet<PathBuf> = plan
        .artifacts
        .iter()
        .map(|a| PathBuf::from(&a.path))
        .chain(metadata.keys().map(PathBuf::from))
        .chain([PathBuf::from("snapshot.toml")])
        .collect();
    let session = crate::resume::Session::open(output, plan, resume)?;
    let completed = output.join("mirror");
    if fs::symlink_metadata(&completed).is_ok() {
        crate::resume::validate_tree(&completed, &expected)?;
        let actual: SelectionSnapshot = toml::from_str(std::str::from_utf8(&read_file(
            &completed.join("snapshot.toml"),
            SMALL_LIMIT,
        )?)?)?;
        let mut files = Vec::new();
        let mut remaining = plan.max_download_bytes.min(limit.unwrap_or(u64::MAX));
        for a in &plan.artifacts {
            let (n, h) =
                crate::resume::hash_file(&completed.join(&a.path), remaining.min(ARTIFACT_LIMIT))?;
            if h != a.sha256 {
                return Err(crate::Error::new(
                    crate::ErrorKind::Integrity,
                    "completed snapshot changed; refusing overwrite",
                )
                .into());
            }
            remaining -= n;
            files.push(SnapshotFile {
                path: a.path.clone(),
                bytes: n,
                sha256: h,
            });
        }
        for (path, data) in &metadata {
            let (n, h) =
                crate::resume::hash_file(&completed.join(path), remaining.min(ARTIFACT_LIMIT))?;
            if n != data.len() as u64 || h != sha256(data) {
                return Err(crate::Error::new(
                    crate::ErrorKind::Integrity,
                    "completed metadata changed; refusing overwrite",
                )
                .into());
            }
            remaining -= n;
            files.push(SnapshotFile {
                path: path.clone(),
                bytes: n,
                sha256: h,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if actual != snapshot(plan, files) {
            return Err(crate::Error::new(
                crate::ErrorKind::Integrity,
                "completed inventory differs from frozen plan",
            )
            .into());
        }
        session.cleanup()?;
        return Ok(completed);
    }
    let staging = session.stage(&expected)?;
    let mut files = Vec::new();
    let mut remaining = plan.max_download_bytes.min(limit.unwrap_or(u64::MAX));
    for a in &plan.artifacts {
        let limit = remaining.min(ARTIFACT_LIMIT);
        if limit == 0 {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "snapshot download budget exhausted",
            )
            .into());
        }
        let (path, n) = session.retrieve(a, source, limit)?;
        let destination = staging.join(&a.path);
        fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
        let mut file = File::create_new(destination)?;
        let mut writer = crate::HashWriter {
            file: &mut file,
            hash: sha2::Sha256::default(),
            bytes: 0,
        };
        crate::events::artifact(&a.path.clone(), "copy", 0, Some(n))?;
        crate::copy_bounded(
            File::open(path)?,
            &mut crate::events::ProgressWriter {
                inner: &mut writer,
                path: a.path.clone(),
                count: 0,
            },
            limit,
        )?;
        let count = writer.bytes;
        let h = hex::encode(sha2::Digest::finalize(writer.hash));
        if count != n || h != a.sha256 {
            return Err(crate::Error::new(
                crate::ErrorKind::Integrity,
                "cached artifact changed while assembling",
            )
            .into());
        }
        file.sync_all()?;
        remaining -= count;
        files.push(SnapshotFile {
            path: a.path.clone(),
            bytes: count,
            sha256: h,
        });
    }
    for (path, data) in metadata {
        if data.len() as u64 > remaining {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "snapshot budget exhausted by metadata",
            )
            .into());
        }
        remaining -= data.len() as u64;
        crate::save(&staging, &path, data, &mut files)?;
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    write_new(
        &staging.join("snapshot.toml"),
        toml::to_string_pretty(&snapshot(plan, files))?.as_bytes(),
    )?;
    if fs::symlink_metadata(&completed).is_ok() {
        return Err("completed tree unexpectedly exists".into());
    }
    crate::events::check()?;
    fs::rename(&staging, &completed)?;
    session.cleanup()?;
    Ok(completed)
}

#[derive(Debug, Serialize)]
/// Metadata availability report returned by [`catalog`].
///
/// Availability describes upstream metadata, not native execution or SDK qualification.
pub struct Catalog {
    /// Requested selector.
    pub toolchain: String,
    /// Resolved compiler release from the selected manifest.
    pub release: String,
    /// Upstream manifest date.
    pub date: String,
    /// Compiler hosts advertised by the Rust metapackage.
    pub hosts: Vec<Availability>,
    /// Component details for the requested host; empty without a host argument.
    pub components: Vec<ComponentAvailability>,
    /// Additional standard-library targets for the requested host; empty without a host argument.
    pub targets: Vec<Availability>,
    /// Profile names recorded upstream, which may exceed supported mirror profiles.
    pub profiles: Vec<String>,
    /// Upstream profiles also supported here: the available subset of `minimal` and `default`.
    pub supported_profiles: Vec<String>,
}
#[derive(Debug, Serialize)]
/// One advertised compiler host or standard-library target.
pub struct Availability {
    /// Host or target triple from upstream metadata.
    pub name: String,
    /// Whether upstream marks this entry available for the resolved release.
    pub available: bool,
}
#[derive(Debug, Serialize)]
/// A component package and its aliases for one requested compiler host.
pub struct ComponentAvailability {
    /// Canonical manifest package name.
    pub package: String,
    /// Alternative component names recorded by manifest renames.
    pub aliases: Vec<String>,
    /// Manifest target identifier, including `*` for host-independent packages.
    pub target: String,
    /// Whether this manifest package/target is available.
    pub available: bool,
    /// Upstream package-version text; not necessarily a bare semantic version.
    pub manifest_package_version: String,
}
/// Inspect upstream metadata. Availability is not native platform/runtime qualification.
/// Resolve release metadata; `host` adds its components and compilation targets.
///
/// Reads metadata/sidecars only and writes no files. Availability does not establish
/// native execution or SDK availability.
///
/// # Errors
/// Refuses unsupported selectors, malformed/hash-invalid metadata, unknown hosts
/// or upstream transport/size failures.
pub fn catalog(toolchain: &str, host: Option<&str>, source: &impl Source) -> Result<Catalog> {
    let meta = metadata(toolchain, source)?;
    let m = parsed(&meta.manifest, &Toolchain::parse(toolchain)?)?;
    let root = m.pkg.get("rust").ok_or("missing rust metapackage")?;
    let hosts = root
        .target
        .iter()
        .filter(|(h, _)| h.as_str() != "*")
        .map(|(h, t)| Availability {
            name: h.clone(),
            available: t.available,
        })
        .collect();
    let mut components = BTreeMap::new();
    let mut targets = BTreeMap::new();
    if let Some(host) = host {
        atom(host)?;
        let t = root
            .target
            .get(host)
            .filter(|t| t.available)
            .ok_or("host unavailable in this manifest")?;
        for c in t.components.iter().chain(&t.extensions) {
            let p = m.pkg.get(&c.pkg).ok_or("dangling component package")?;
            let target = p
                .target
                .get(&c.target)
                .or_else(|| p.target.get("*"))
                .ok_or("dangling target metadata")?;
            if c.pkg == "rust-std" {
                targets.insert(
                    c.target.clone(),
                    Availability {
                        name: c.target.clone(),
                        available: target.available,
                    },
                );
            }
            if c.target == host || c.target == "*" {
                components.insert(
                    (c.pkg.clone(), c.target.clone()),
                    ComponentAvailability {
                        package: c.pkg.clone(),
                        aliases: m
                            .renames
                            .iter()
                            .filter(|(_, r)| r.to == c.pkg)
                            .map(|(a, _)| a.clone())
                            .collect(),
                        target: c.target.clone(),
                        available: target.available,
                        manifest_package_version: p.version.clone(),
                    },
                );
            }
        }
    }
    Ok(Catalog {
        toolchain: toolchain.into(),
        release: root.version.clone(),
        date: m.date,
        hosts,
        components: components.into_values().collect(),
        targets: targets.into_values().collect(),
        supported_profiles: m
            .profiles
            .keys()
            .filter(|p| ["minimal", "default"].contains(&p.as_str()))
            .cloned()
            .collect(),
        profiles: m.profiles.into_keys().collect(),
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolchainFile {
    toolchain: ImportedToolchain,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedToolchain {
    channel: Option<String>,
    path: Option<String>,
    profile: Option<String>,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    targets: Vec<String>,
}
/// Import one project toolchain file. Hosts and bootstrap version remain explicit.
/// Read a project toolchain file into one named, validated configuration.
///
/// Supports TOML and legacy `rust-toolchain` selectors. Caller-supplied `name`,
/// `hosts` and manager version `rustup` remain explicit. The result is not written;
/// use [`write_config`]. No upstream availability lookup occurs.
///
/// # Errors
/// Refuses missing/oversized/malformed files, local custom paths, unsupported
/// selectors or invalid profile/component/target/configuration constraints.
pub fn import_toolchain(
    path: &Path,
    name: &str,
    hosts: Vec<String>,
    rustup: &str,
) -> Result<Config> {
    let raw = read_file(path, SMALL_LIMIT)?;
    let text = std::str::from_utf8(&raw)?;
    let imported = if path.file_name().is_some_and(|n| n == "rust-toolchain")
        && !text.trim_start().starts_with('[')
    {
        let channel = text.trim();
        if !channel.is_ascii() {
            return Err("legacy toolchain file must be ASCII".into());
        }
        ImportedToolchain {
            channel: Some(channel.into()),
            path: None,
            profile: None,
            components: Vec::new(),
            targets: Vec::new(),
        }
    } else {
        toml::from_str::<ToolchainFile>(text)?.toolchain
    };
    if imported.path.is_some() {
        return Err("local custom toolchain paths cannot be mirrored".into());
    }
    let selection = Selection {
        name: name.into(),
        toolchain: imported
            .channel
            .ok_or("toolchain file needs an explicit channel")?,
        hosts,
        profile: imported.profile.unwrap_or_else(default_profile),
        components: imported.components,
        targets: imported.targets,
    };
    let config = Config {
        schema: 1,
        rustup_version: rustup.into(),
        max_download_bytes: default_budget(),
        selections: vec![selection],
        retention: None,
    };
    config.validate()?;
    Ok(config)
}
/// Validate and write pretty TOML to a new file without network access.
///
/// # Errors
/// Refuses invalid config, existing destination, missing parent or local I/O failures.
pub fn write_config(config: &Config, path: &Path) -> Result<()> {
    config.validate()?;
    write_new(path, toml::to_string_pretty(config)?.as_bytes())
}

/// Fetch with structured progress, independent heartbeats and cooperative cancellation.
/// # Errors
/// Returns typed fetch/refusal/cancellation errors; initialized private progress remains resumable.
pub fn fetch_with_control(
    plan: &Path,
    output: &Path,
    source: &impl Source,
    control: &crate::events::Control,
) -> Result<PathBuf> {
    control.run("fetch", || fetch(plan, output, source))
}
/// Resume a frozen fetch with progress and cancellation; never changes the saved identity.
/// # Errors
/// Returns typed resume/refusal/cancellation errors, preserving private progress.
pub fn fetch_resuming_with_control(
    plan: &Path,
    output: &Path,
    source: &impl Source,
    control: &crate::events::Control,
) -> Result<PathBuf> {
    control.run("fetch resume", || fetch_resuming(plan, output, source))
}
