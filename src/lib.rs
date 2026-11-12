#![doc = include_str!("guide.md")]
#![warn(missing_docs)]
pub mod cli;
mod error;
pub mod events;
pub mod network;
pub mod releases;
pub use network::NetworkOptions;
pub mod publish;
pub use error::{error_kind, Error, ErrorKind};
mod resume;
pub mod selection;
pub use resume::Transfer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Error result used by synchronous library operations.
///
/// Errors include validation/refusal, local I/O and transport failures; callers
/// should preserve their context rather than treating every failure as retryable.
pub type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;
const ORIGIN: &str = "https://static.rust-lang.org/";
const METADATA_LIMIT: u64 = 4 * 1024 * 1024;
const ARTIFACT_LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
/// Options for the legacy minimal GNU Linux snapshot route.
///
/// Use [`selection::Config`] for multi-host selections, components and targets.
pub struct Options {
    /// Full numeric Rust release, for example `1.90.0`.
    pub release: String,
    /// Compiler host; this legacy route accepts only `x86_64-unknown-linux-gnu`.
    pub host: String,
    /// Full numeric Rustup bootstrap version, independent of the compiler release.
    pub rustup_version: String,
    /// Must not already exist. The completed tree is returned as `<output>/mirror`.
    pub output: PathBuf,
}

/// Transport boundary shared by the CLI and deterministic tests. Implementations must
/// refuse oversized responses. Sources are required to honor the supplied URL exactly.
pub trait Source {
    /// Write the complete response for exactly `url`, refusing bytes beyond `limit`.
    ///
    /// Errors may leave partial output. The caller verifies the planned whole-file hash;
    /// implementations must not silently substitute another origin or artifact.
    fn fetch(&self, url: &str, output: &mut dyn Write, limit: u64) -> Result<()>;
    /// Optional HEAD Content-Length estimate; never an integrity proof. Default sources return unknown.
    fn content_length(&self, _url: &str) -> Result<Option<u64>> {
        Ok(None)
    }
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

/// Official upstream HTTPS transport with bounded reads and no redirects.
///
/// Create with [`Default::default`]. Requests must remain under
/// `https://static.rust-lang.org/`. The default request timeout is 180 seconds;
/// resume uses verified range handling. This does not configure publisher or client trust.
pub struct HttpsSource(ureq::Agent);
impl HttpsSource {
    /// Build explicit upstream transport; origin/path validation still applies to every request.
    pub fn with_network(options: &NetworkOptions) -> Result<Self> {
        Ok(Self(network::agent(options, "static.rust-lang.org", 443)?))
    }
}
impl Default for HttpsSource {
    fn default() -> Self {
        Self::with_network(&NetworkOptions::default()).expect("default network options are valid")
    }
}
impl Source for HttpsSource {
    fn content_length(&self, url: &str) -> Result<Option<u64>> {
        safe_path(
            url.strip_prefix(ORIGIN)
                .ok_or("source must be the official HTTPS origin")?,
        )?;
        events::artifact(url.strip_prefix(ORIGIN).unwrap(), "head", 0, None)?;
        let response = match self.0.head(url).set("Accept-Encoding", "identity").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(405 | 501, _)) => return Ok(None),
            Err(e) => return Err(network::transport_error(e)),
        };
        if response.status() != 200 {
            return Err(Error::new(ErrorKind::Network, "HEAD did not return HTTP 200").into());
        }
        if response
            .header("Content-Encoding")
            .is_some_and(|v| v != "identity")
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "HEAD response uses an unexpected encoding",
            )
            .into());
        }
        let values = response.all("Content-Length");
        if values.is_empty() {
            return Ok(None);
        }
        let mut size = None;
        for value in values {
            if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
                return Err(Error::new(ErrorKind::Integrity, "invalid HEAD Content-Length").into());
            }
            let n = value
                .parse::<u64>()
                .map_err(|_| Error::new(ErrorKind::Budget, "HEAD length exceeds integer bounds"))?;
            if size.is_some_and(|old| old != n) {
                return Err(Error::new(ErrorKind::Integrity, "conflicting HEAD lengths").into());
            }
            size = Some(n);
        }
        Ok(size)
    }

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
        events::artifact(path, "download", 0, None)?;
        let response = self.0.get(url).call().map_err(network::transport_error)?;
        if response.status() != 200 {
            return Err("source did not return HTTP 200".into());
        }
        events::artifact(
            path,
            "download",
            0,
            response
                .header("Content-Length")
                .and_then(|s| s.parse::<u64>().ok()),
        )?;
        copy_bounded(
            network::NetworkReader(response.into_reader()),
            &mut events::ProgressWriter {
                inner: output,
                path: path.into(),
                count: 0,
            },
            limit,
        )
    }
}
fn copy_bounded(mut input: impl Read, output: &mut dyn Write, limit: u64) -> Result<()> {
    let mut count = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        events::check()?;
        let remaining = limit.saturating_sub(count).saturating_add(1);
        let cap = buffer
            .len()
            .min(remaining.min(buffer.len() as u64) as usize);
        let n = input.read(&mut buffer[..cap])?;
        events::check()?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > limit {
            return Err(Error::new(ErrorKind::Budget, "download exceeds size budget").into());
        }
        output.write_all(&buffer[..n])?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// Legacy schema-1 inventory written to `snapshot.toml`.
///
/// For selected or combined generations see [`selection::SelectionSnapshot`] and
/// [`selection::collection::CollectionSnapshot`]. Parsing alone does not verify files.
pub struct Snapshot {
    /// Inventory schema number; the legacy route writes `1`.
    pub schema: u32,
    /// Pinned numeric Rust compiler release.
    pub release: String,
    /// GNU Linux x86-64 compiler host.
    pub host: String,
    /// Pinned bootstrap manager version.
    pub rustup_version: String,
    /// The legacy route records `minimal`.
    pub profile: String,
    /// Payload/metadata entries; the inventory itself is not listed.
    pub files: Vec<SnapshotFile>,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// One relative file path with its expected SHA-256 and byte length.
///
/// Consumers must verify actual bytes and path containment; deserialization is not verification.
pub struct SnapshotFile {
    /// Forward-slash path relative to the mirror root, not an arbitrary filesystem path.
    pub path: String,
    /// Expected lowercase hexadecimal SHA-256 of the complete file.
    pub sha256: String,
    /// Expected complete file size in bytes.
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
        .ok_or_else(|| crate::Error::new(crate::ErrorKind::Integrity, "empty checksum file"))?;
    valid_hash(hash)?;
    Ok(hash.to_ascii_lowercase())
}
fn bytes(source: &impl Source, path: &str, limit: u64) -> Result<Vec<u8>> {
    safe_path(path)?;
    let mut output = Vec::new();
    events::artifact(path, "download", 0, None)?;
    source.fetch(
        &format!("{ORIGIN}{path}"),
        &mut events::ProgressWriter {
            inner: &mut output,
            path: path.into(),
            count: 0,
        },
        limit,
    )?;
    events::check()?;
    if output.len() as u64 > limit {
        return Err(
            crate::Error::new(crate::ErrorKind::Budget, "source exceeded byte budget").into(),
        );
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
        let hash = hash.ok_or_else(|| {
            crate::Error::new(
                crate::ErrorKind::Integrity,
                "archive URL has no matching checksum",
            )
        })?;
        valid_hash(hash)?;
        let path = url.strip_prefix(ORIGIN).ok_or("foreign artifact origin")?;
        safe_path(path)?;
        if !path.starts_with(&format!("dist/{}/", m.date)) {
            return Err("artifact outside selected release directory".into());
        }
        if let Some(previous) = artifacts.insert(path.to_string(), hash.to_ascii_lowercase()) {
            if previous != hash.to_ascii_lowercase() {
                return Err(crate::Error::new(
                    crate::ErrorKind::Integrity,
                    "conflicting artifact checksums",
                )
                .into());
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
        return Err(crate::Error::new(
            crate::ErrorKind::Budget,
            "minimal profile exceeds artifact budget",
        )
        .into());
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
        events::check().map_err(io::Error::other)?;
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
    events::artifact(path, "copy", 0, Some(data.len() as u64))?;
    let destination = root.join(path);
    fs::create_dir_all(destination.parent().ok_or("missing parent")?)?;
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(&destination)?;
    events::ProgressWriter {
        inner: &mut file,
        path: path.into(),
        count: 0,
    }
    .write_all(data)?;
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
    events::artifact(path, "download", 0, None)?;
    source.fetch(
        &format!("{ORIGIN}{path}"),
        &mut events::ProgressWriter {
            inner: &mut writer,
            path: path.into(),
            count: 0,
        },
        ARTIFACT_LIMIT,
    )?;
    events::check()?;
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
///
/// Output must be new with an existing parent. Returns its completed mirror child.
/// For multiple hosts/components prefer [selection::sync::run].
///
/// # Errors
/// Refuses invalid versions/host, existing output, invalid metadata, unavailable
/// payloads, size/hash mismatches or transport/local I/O failures.
pub fn prepare(options: &Options, source: &impl Source) -> Result<PathBuf> {
    version(&options.release)?;
    version(&options.rustup_version)?;
    // The first milestone is deliberately narrow; other hosts need their own proof.
    if options.host != "x86_64-unknown-linux-gnu" {
        return Err("this milestone supports only x86_64-unknown-linux-gnu".into());
    }
    if options.output.try_exists()? {
        return Err(crate::Error::new(
            crate::ErrorKind::OutputExists,
            "output already exists; choose a new snapshot directory",
        )
        .into());
    }
    let manifest_path = format!("dist/channel-rust-{}.toml", options.release);
    let upstream_hash = bytes(source, &format!("{manifest_path}.sha256"), 4096)?;
    let raw = bytes(source, &manifest_path, METADATA_LIMIT)?;
    if sha256(&raw) != checksum(&upstream_hash)? {
        return Err(crate::Error::new(
            crate::ErrorKind::Integrity,
            "upstream manifest checksum mismatch",
        )
        .into());
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
    crate::events::check()?;
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
