//! Saved selections and metadata-only, replayable download plans.
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
pub struct Config {
    pub schema: u32,
    pub rustup_version: String,
    #[serde(default = "default_budget")]
    pub max_download_bytes: u64,
    pub selections: Vec<Selection>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub name: String,
    pub toolchain: String,
    pub hosts: Vec<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
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
        return Err("selection list exceeds budget".into());
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
    pub fn parse(text: &str) -> Result<Self> {
        let c: Self = toml::from_str(text)?;
        c.validate()?;
        Ok(c)
    }
    pub fn read(path: &Path) -> Result<Self> {
        Self::parse(std::str::from_utf8(&read_file(path, SMALL_LIMIT)?)?)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            return Err("unsupported configuration schema".into());
        }
        version(&self.rustup_version)?;
        if self.max_download_bytes == 0 || self.max_download_bytes > 100 * 1024 * 1024 * 1024 {
            return Err("download budget must be 1 byte to 100 GiB".into());
        }
        if self.selections.is_empty() || self.selections.len() > 32 {
            return Err("select 1 to 32 named selections".into());
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
        return Err("local file exceeds budget".into());
    }
    Ok(data)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
    pub package: String,
    pub target: String,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: u32,
    pub planner_revision: u32,
    pub rustup_version: String,
    pub max_download_bytes: u64,
    pub selection: Selection,
    pub resolved_release: String,
    pub date: String,
    pub pinned_toolchain: String,
    pub metadata_source: String,
    pub manifest_sha256: String,
    pub checksum_sha256: String,
    pub signature_sha256: String,
    pub bootstrap_checksums: BTreeMap<String, String>,
    pub artifacts: Vec<Artifact>,
}
pub struct ResolvedPlan {
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
        return Err("manifest date differs from requested date".into());
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
        return Err("plan exceeds artifact budget".into());
    }
    Ok(())
}
fn build_plan(selection: &Selection, rustup: &str, budget: u64, meta: &Metadata) -> Result<Plan> {
    selection.validate()?;
    version(rustup)?;
    let spec = Toolchain::parse(&selection.toolchain)?;
    let m = parsed(&meta.manifest, &spec)?;
    if sha256(&meta.manifest) != checksum(&meta.checksum)? {
        return Err("manifest checksum mismatch".into());
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
                    _ => return Err("archive URL/checksum pair is incomplete".into()),
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
        let sha = meta
            .bootstraps
            .get(host)
            .ok_or("missing bootstrap checksum metadata")?;
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
        return Err("upstream manifest checksum mismatch; retry resolution".into());
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
    let mut f = File::options().write(true).create_new(true).open(path)?;
    f.write_all(data)?;
    f.sync_all()?;
    Ok(())
}
impl ResolvedPlan {
    /// Persist only metadata and a preview/lock; no component archive is downloaded.
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
        return Err("frozen plan differs from cached metadata; resolve a new plan".into());
    }
    Ok(ResolvedPlan {
        plan,
        metadata: meta,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub schema: u32,
    pub release: String,
    pub date: String,
    pub pinned_toolchain: String,
    pub rustup_version: String,
    pub selection: Selection,
    pub files: Vec<SnapshotFile>,
}
/// Fetch a previously saved, verified plan. No live channel metadata is consulted.
pub fn fetch(plan_dir: &Path, output: &Path, source: &impl Source) -> Result<PathBuf> {
    let resolved = load_plan(plan_dir)?;
    let plan = &resolved.plan;
    let meta = &resolved.metadata;
    fs::create_dir(output)?;
    let staging = tempfile::Builder::new()
        .prefix(".building-")
        .tempdir_in(output)?;
    let mut files = Vec::new();
    let mut remaining = plan.max_download_bytes;
    for a in &plan.artifacts {
        let limit = remaining.min(ARTIFACT_LIMIT);
        if limit == 0 {
            return Err("snapshot download budget exhausted".into());
        }
        let destination = staging.path().join(&a.path);
        fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
        let mut f = File::options()
            .create_new(true)
            .write(true)
            .open(destination)?;
        let mut writer = crate::HashWriter {
            file: &mut f,
            hash: sha2::Sha256::default(),
            bytes: 0,
        };
        source.fetch(&format!("{ORIGIN}{}", a.path), &mut writer, limit)?;
        let count = writer.bytes;
        let actual = hex::encode(sha2::Digest::finalize(writer.hash));
        if count > limit || actual != a.sha256 {
            return Err(format!("size/hash verification failed: {}", a.path).into());
        }
        f.sync_all()?;
        remaining -= count;
        files.push(SnapshotFile {
            path: a.path.clone(),
            sha256: actual,
            bytes: count,
        });
    }
    let spec = Toolchain::parse(&plan.selection.toolchain)?;
    let mut metadata_files = BTreeMap::new();
    for path in [
        plan.metadata_source.clone(),
        format!("dist/{}/channel-rust-{}.toml", plan.date, spec.channel),
    ] {
        metadata_files.insert(path.clone(), &meta.manifest);
        metadata_files.insert(format!("{path}.sha256"), &meta.checksum);
        metadata_files.insert(format!("{path}.asc"), &meta.signature);
    }
    for host in &plan.selection.hosts {
        metadata_files.insert(
            format!("{}.sha256", bootstrap_path(&plan.rustup_version, host)),
            &meta.bootstraps[host],
        );
    }
    for (path, data) in metadata_files {
        if data.len() as u64 > remaining {
            return Err("snapshot budget exhausted by metadata".into());
        }
        remaining -= data.len() as u64;
        crate::save(staging.path(), &path, data, &mut files)?;
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let snapshot = SelectionSnapshot {
        schema: 2,
        release: plan.resolved_release.clone(),
        date: plan.date.clone(),
        pinned_toolchain: plan.pinned_toolchain.clone(),
        rustup_version: plan.rustup_version.clone(),
        selection: plan.selection.clone(),
        files,
    };
    write_new(
        &staging.path().join("snapshot.toml"),
        toml::to_string_pretty(&snapshot)?.as_bytes(),
    )?;
    let completed = output.join("mirror");
    if completed.try_exists()? {
        return Err("completed tree unexpectedly exists".into());
    }
    fs::rename(staging.path(), &completed)?;
    Ok(completed)
}

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub toolchain: String,
    pub release: String,
    pub date: String,
    pub hosts: Vec<Availability>,
    pub components: Vec<ComponentAvailability>,
    pub targets: Vec<Availability>,
    pub profiles: Vec<String>,
    pub supported_profiles: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Availability {
    pub name: String,
    pub available: bool,
}
#[derive(Debug, Serialize)]
pub struct ComponentAvailability {
    pub package: String,
    pub aliases: Vec<String>,
    pub target: String,
    pub available: bool,
    pub manifest_package_version: String,
}
/// Inspect upstream metadata. Availability is not native platform/runtime qualification.
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
    };
    config.validate()?;
    Ok(config)
}
pub fn write_config(config: &Config, path: &Path) -> Result<()> {
    config.validate()?;
    write_new(path, toml::to_string_pretty(config)?.as_bytes())
}
