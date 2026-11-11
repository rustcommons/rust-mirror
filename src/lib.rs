//! Pinned toolchain snapshots. Upstream manifests are preserved, not rewritten.
pub mod publish;
mod resume;
pub mod selection;
pub use resume::Transfer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const ORIGIN: &str = "https://static.rust-lang.org/";
const METADATA_LIMIT: u64 = 4 * 1024 * 1024;
const ARTIFACT_LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub struct Options {
    pub release: String,
    pub host: String,
    pub rustup_version: String,
    /// Must not already exist. The completed tree is returned as `<output>/mirror`.
    pub output: PathBuf,
}

/// Transport boundary shared by the CLI and deterministic tests. Implementations must
/// refuse oversized responses. Sources are required to honor the supplied URL exactly.
pub trait Source {
    fn fetch(&self, url: &str, output: &mut dyn Write, limit: u64) -> Result<()>;
    /// Append bytes beginning at offset, within the remaining byte budget. Restart must
    /// write nothing. Errors may leave a prefix; only a final whole-file hash proves it.
    /// Sources without range support safely restart that file.
    fn fetch_from(
        &self,
        url: &str,
        output: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        if offset != 0 {
            return Ok(Transfer::Restart);
        }
        self.fetch(url, output, limit)?;
        Ok(Transfer::Complete)
    }
}

pub struct HttpsSource(ureq::Agent);
impl Default for HttpsSource {
    fn default() -> Self {
        Self(
            ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(180))
                .redirects(0)
                .build(),
        )
    }
}
impl Source for HttpsSource {
    fn fetch_from(
        &self,
        url: &str,
        output: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        safe_path(
            url.strip_prefix(ORIGIN)
                .ok_or("source must be the official HTTPS origin")?,
        )?;
        resume::http_range(&self.0, url, output, offset, limit)
    }
    fn fetch(&self, url: &str, output: &mut dyn Write, limit: u64) -> Result<()> {
        let path = url
            .strip_prefix(ORIGIN)
            .ok_or("source must be the official HTTPS origin")?;
        safe_path(path)?;
        let _ = writeln!(io::stderr().lock(), "Downloading {path}");
        let response = self.0.get(url).call()?;
        if response.status() != 200 {
            return Err("source did not return HTTP 200".into());
        }
        copy_bounded(response.into_reader(), output, limit)
    }
}
fn copy_bounded(input: impl Read, output: &mut dyn Write, limit: u64) -> Result<()> {
    let count = io::copy(&mut input.take(limit + 1), output)?;
    if count > limit {
        return Err("download exceeds size budget".into());
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: u32,
    pub release: String,
    pub host: String,
    pub rustup_version: String,
    pub profile: String,
    pub files: Vec<SnapshotFile>,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Manifest {
    manifest_version: String,
    date: String,
    pkg: BTreeMap<String, Package>,
    profiles: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    renames: BTreeMap<String, Renamed>,
}
#[derive(Deserialize)]
struct Renamed {
    to: String,
}
#[derive(Deserialize)]
struct Package {
    version: String,
    target: BTreeMap<String, Target>,
}
#[derive(Deserialize)]
struct Target {
    available: bool,
    url: Option<String>,
    hash: Option<String>,
    xz_url: Option<String>,
    xz_hash: Option<String>,
    zst_url: Option<String>,
    zst_hash: Option<String>,
    #[serde(default)]
    components: Vec<Component>,
    #[serde(default)]
    extensions: Vec<Component>,
}
#[derive(Deserialize)]
struct Component {
    pkg: String,
    target: String,
}

fn version(value: &str) -> Result<()> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || p.len() > 5 || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err("expected a pinned numeric version, such as 1.90.0".into());
    }
    Ok(())
}
fn safe_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 512
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || !path
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._/".contains(&c))
    {
        return Err("unsafe mirror-relative path".into());
    }
    Ok(())
}
fn valid_hash(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid SHA-256".into());
    }
    Ok(())
}
fn checksum(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)?;
    let hash = text
        .split_whitespace()
        .next()
        .ok_or("empty checksum file")?;
    valid_hash(hash)?;
    Ok(hash.to_ascii_lowercase())
}
fn bytes(source: &impl Source, path: &str, limit: u64) -> Result<Vec<u8>> {
    safe_path(path)?;
    let mut output = Vec::new();
    source.fetch(&format!("{ORIGIN}{path}"), &mut output, limit)?;
    if output.len() as u64 > limit {
        return Err("source exceeded byte budget".into());
    }
    Ok(output)
}
fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn plan(raw: &[u8], options: &Options) -> Result<Vec<(String, String)>> {
    let m: Manifest = toml::from_str(std::str::from_utf8(raw)?)?;
    if m.manifest_version != "2" {
        return Err("unsupported upstream manifest schema".into());
    }
    let date = m.date.as_bytes();
    if date.len() != 10
        || date[4] != b'-'
        || date[7] != b'-'
        || date
            .iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return Err("invalid manifest date".into());
    }
    let rust = m.pkg.get("rust").ok_or("missing rust metapackage")?;
    if rust.version.split_whitespace().next() != Some(options.release.as_str()) {
        return Err("manifest release does not match request".into());
    }
    let host = rust
        .target
        .get(&options.host)
        .filter(|h| h.available)
        .ok_or("host unavailable")?;
    let profile = m.profiles.get("minimal").ok_or("missing minimal profile")?;
    let mut artifacts = BTreeMap::new();
    let mut selected = Vec::new();
    for component in host
        .components
        .iter()
        .chain(&host.extensions)
        .filter(|c| profile.contains(&c.pkg) && (c.target == options.host || c.target == "*"))
    {
        let package = m
            .pkg
            .get(&component.pkg)
            .ok_or("dangling profile package")?;
        let target = package
            .target
            .get(&component.target)
            .or_else(|| package.target.get("*"))
            .ok_or("dangling component target")?;
        if !target.available {
            return Err("selected component unavailable".into());
        }
        let (url, hash) = if let Some(url) = &target.zst_url {
            (url, target.zst_hash.as_ref())
        } else if let Some(url) = &target.xz_url {
            (url, target.xz_hash.as_ref())
        } else {
            (
                target.url.as_ref().ok_or("component has no archive URL")?,
                target.hash.as_ref(),
            )
        };
        let hash = hash.ok_or("archive URL has no matching checksum")?;
        valid_hash(hash)?;
        let path = url.strip_prefix(ORIGIN).ok_or("foreign artifact origin")?;
        safe_path(path)?;
        if !path.starts_with(&format!("dist/{}/", m.date)) {
            return Err("artifact outside selected release directory".into());
        }
        if let Some(previous) = artifacts.insert(path.to_string(), hash.to_ascii_lowercase()) {
            if previous != hash.to_ascii_lowercase() {
                return Err("conflicting artifact checksums".into());
            }
        }
        selected.push(component.pkg.as_str());
    }
    for required in ["rustc", "cargo", "rust-std"] {
        if !selected.contains(&required) {
            return Err(format!("minimal profile is missing {required}").into());
        }
    }
    if artifacts.len() > 16 {
        return Err("minimal profile exceeds artifact budget".into());
    }
    Ok(artifacts.into_iter().collect())
}

struct HashWriter<'a> {
    file: &'a mut File,
    hash: Sha256,
    bytes: u64,
}
impl Write for HashWriter<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let count = self.file.write(data)?;
        self.hash.update(&data[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
fn save(root: &Path, path: &str, data: &[u8], files: &mut Vec<SnapshotFile>) -> Result<()> {
    safe_path(path)?;
    let destination = root.join(path);
    fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(&destination)?;
    file.write_all(data)?;
    file.sync_all()?;
    files.push(SnapshotFile {
        path: path.into(),
        sha256: sha256(data),
        bytes: data.len() as u64,
    });
    Ok(())
}
fn download(
    root: &Path,
    source: &impl Source,
    path: &str,
    expected: &str,
    files: &mut Vec<SnapshotFile>,
) -> Result<()> {
    safe_path(path)?;
    valid_hash(expected)?;
    let destination = root.join(path);
    fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(&destination)?;
    let mut writer = HashWriter {
        file: &mut file,
        hash: Sha256::new(),
        bytes: 0,
    };
    source.fetch(&format!("{ORIGIN}{path}"), &mut writer, ARTIFACT_LIMIT)?;
    let count = writer.bytes;
    let actual = hex::encode(writer.hash.finalize());
    if count > ARTIFACT_LIMIT || actual != expected.to_ascii_lowercase() {
        return Err(format!("size/checksum verification failed: {path}").into());
    }
    file.sync_all()?;
    files.push(SnapshotFile {
        path: path.into(),
        sha256: actual,
        bytes: count,
    });
    Ok(())
}

/// Prepare an immutable, minimal-profile static tree. Does not install a toolchain,
/// upload to a server, modify upstream manifests, or claim signature authentication.
/// An existing output is always refused. Failed attempts may leave the reserved
/// output directory; they never produce a completed `mirror` tree.
pub fn prepare(options: &Options, source: &impl Source) -> Result<PathBuf> {
    version(&options.release)?;
    version(&options.rustup_version)?;
    // The first milestone is deliberately narrow; other hosts need their own proof.
    if options.host != "x86_64-unknown-linux-gnu" {
        return Err("this milestone supports only x86_64-unknown-linux-gnu".into());
    }
    if options.output.try_exists()? {
        return Err("output already exists; choose a new snapshot directory".into());
    }
    let manifest_path = format!("dist/channel-rust-{}.toml", options.release);
    let upstream_hash = bytes(source, &format!("{manifest_path}.sha256"), 4096)?;
    let raw = bytes(source, &manifest_path, METADATA_LIMIT)?;
    if sha256(&raw) != checksum(&upstream_hash)? {
        return Err("upstream manifest checksum mismatch".into());
    }
    let artifacts = plan(&raw, options)?;
    let signature = bytes(source, &format!("{manifest_path}.asc"), 64 * 1024)?;
    let bootstrap = format!(
        "rustup/archive/{}/{}/rustup-init",
        options.rustup_version, options.host
    );
    let bootstrap_hash = bytes(source, &format!("{bootstrap}.sha256"), 4096)?;
    let expected_bootstrap = checksum(&bootstrap_hash)?;
    fs::create_dir(&options.output)?;
    let staging = tempfile::Builder::new()
        .prefix(".building-")
        .tempdir_in(&options.output)?;
    let mut files = Vec::new();
    for (path, hash) in artifacts {
        download(staging.path(), source, &path, &hash, &mut files)?;
    }
    download(
        staging.path(),
        source,
        &bootstrap,
        &expected_bootstrap,
        &mut files,
    )?;
    save(
        staging.path(),
        &format!("{bootstrap}.sha256"),
        &bootstrap_hash,
        &mut files,
    )?;
    save(staging.path(), &manifest_path, &raw, &mut files)?;
    save(
        staging.path(),
        &format!("{manifest_path}.sha256"),
        &upstream_hash,
        &mut files,
    )?;
    save(
        staging.path(),
        &format!("{manifest_path}.asc"),
        &signature,
        &mut files,
    )?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let snapshot = Snapshot {
        schema: 1,
        release: options.release.clone(),
        host: options.host.clone(),
        rustup_version: options.rustup_version.clone(),
        profile: "minimal".into(),
        files,
    };
    let inventory = toml::to_string_pretty(&snapshot)?;
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(staging.path().join("snapshot.toml"))?;
    file.write_all(inventory.as_bytes())?;
    file.sync_all()?;
    drop(file); // Windows requires closing the inventory before renaming its directory.
    let completed = options.output.join("mirror");
    // `output` is an exclusively-created reservation, not a shared cache or live tree.
    if completed.try_exists()? {
        return Err("completed tree unexpectedly exists".into());
    }
    fs::rename(staging.path(), &completed)?;
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_reject_traversal_origins_and_encoding() {
        for value in [
            "../file",
            "/file",
            "a/../b",
            "a\\b",
            "a//b",
            "a/%2e%2e/b",
            "a?query",
            "a:stream",
        ] {
            assert!(safe_path(value).is_err(), "{value}");
        }
        assert!(safe_path("dist/2025-09-18/rustc-1.90.0.tar.xz").is_ok());
    }
    #[test]
    fn versions_are_pinned_and_bounded() {
        for value in [
            "stable",
            "nightly",
            "../1.90.0",
            "1.90",
            "1.90.0-beta",
            "123456.0.0",
        ] {
            assert!(version(value).is_err());
        }
        assert!(version("1.90.0").is_ok());
    }
    #[test]
    fn bounded_download_refuses_excess() {
        let mut output = Vec::new();
        assert!(copy_bounded(&b"12345"[..], &mut output, 4).is_err());
        assert!(copy_bounded(&b"1234"[..], &mut Vec::new(), 4).is_ok());
    }
    #[test]
    fn checksum_requires_full_hex() {
        assert!(checksum(b"deadbeef  file").is_err());
        assert_eq!(
            checksum(format!("{}  file\n", "A".repeat(64)).as_bytes()).unwrap(),
            "a".repeat(64)
        );
    }
}
