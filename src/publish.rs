//! Verified, no-overwrite publication to a fresh Nexus Raw/static HTTP prefix.
//!
//! Pass a completed mirror root and explicit [Options] to [publish]. The library
//! takes credentials explicitly; only the CLI reads credential environment variables.
//! It verifies local closure and remote absence before writing and checks GET
//! hashes afterward. This never provisions Nexus, activates routing or installs tools.
use crate::{Result, SnapshotFile, ARTIFACT_LIMIT};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

/// Credentials are explicit, never read implicitly by the library or included in Debug output.
/// Explicit connection and authentication settings for [`publish`].
///
/// The library never reads CLI credential environment variables. This type has no
/// `Debug` implementation so credentials are not accidentally formatted.
pub struct Options {
    /// Fresh HTTPS prefix for complete mirror paths; no userinfo, query, fragment or redirects.
    pub base_url: String,
    /// Optional PEM CA bundle for this connection only; defaults to the publisher TLS root store.
    pub ca_file: Option<PathBuf>,
    /// Optional `(username, password)` for HTTPS Basic authentication; both must be nonempty.
    pub credentials: Option<(String, String)>,
    /// Test-only literal IPv4/IPv6 loopback. Credentials still require HTTPS.
    pub allow_loopback_http: bool,
}
#[derive(Serialize)]
/// Successful remote verification summary.
///
/// Returned only after all uploads/GET hashes succeed; it does not mean a Nexus
/// group was activated, a client installed tools or multiple requests switched atomically.
pub struct Report {
    /// Number of uploaded/verified files, including the inventory marker.
    pub files: usize,
    /// Total bytes read back and verified through GET requests.
    pub remote_bytes_verified: u64,
    /// SHA-256 of the exact uploaded `snapshot.toml` bytes.
    pub inventory_sha256: String,
    /// Source inventory schema, not a report-schema version.
    pub schema: u32,
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if !std::fs::metadata(path)?.is_file() {
        return Err("publication input must be a regular file".into());
    }
    let f = File::open(path)?;
    if !f.metadata()?.is_file() {
        return Err("publication input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    f.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("publication metadata exceeds byte budget".into());
    }
    Ok(bytes)
}
fn files(root: &Path) -> Result<(u32, Vec<SnapshotFile>, String)> {
    let directory = std::fs::symlink_metadata(root)?;
    let inventory = std::fs::symlink_metadata(root.join("snapshot.toml"))?;
    if !directory.is_dir()
        || directory.file_type().is_symlink()
        || !inventory.is_file()
        || inventory.file_type().is_symlink()
    {
        return Err("publication needs a real mirror directory and inventory".into());
    }
    let raw = read(&root.join("snapshot.toml"), 1024 * 1024)?;
    let doc: toml::Value = toml::from_str(std::str::from_utf8(&raw)?)?;
    let schema = doc
        .get("schema")
        .and_then(toml::Value::as_integer)
        .ok_or("missing snapshot schema")?;
    let mut files = if schema == 1 {
        let s: crate::Snapshot = toml::from_str(std::str::from_utf8(&raw)?)?;
        crate::version(&s.release)?;
        crate::version(&s.rustup_version)?;
        if s.profile != "minimal"
            || s.host != "x86_64-unknown-linux-gnu"
            || s.files.is_empty()
            || s.files.len() > 24
        {
            return Err("unsupported legacy inventory".into());
        }
        let mut names = BTreeSet::from([PathBuf::from("snapshot.toml")]);
        let mut map = BTreeMap::new();
        for f in &s.files {
            crate::safe_path(&f.path)?;
            crate::valid_hash(&f.sha256)?;
            if f.path == "snapshot.toml"
                || f.bytes > ARTIFACT_LIMIT
                || map.insert(f.path.clone(), f.sha256.clone()).is_some()
            {
                return Err("invalid legacy inventory entry".into());
            }
            names.insert(PathBuf::from(&f.path));
        }
        crate::resume::validate_tree(root, &names)?;
        for f in &s.files {
            if crate::resume::hash_file(&root.join(&f.path), f.bytes)?
                != (f.bytes, f.sha256.clone())
            {
                return Err("legacy inventory file changed".into());
            }
        }
        let manifest = format!("dist/channel-rust-{}.toml", s.release);
        let data = read(&root.join(&manifest), crate::METADATA_LIMIT)?;
        if crate::checksum(&read(&root.join(format!("{manifest}.sha256")), 4096)?)?
            != crate::sha256(&data)
        {
            return Err("legacy manifest checksum mismatch".into());
        }
        read(&root.join(format!("{manifest}.asc")), 64 * 1024)?;
        let options = crate::Options {
            release: s.release.clone(),
            host: s.host.clone(),
            rustup_version: s.rustup_version.clone(),
            output: PathBuf::new(),
        };
        let mut expected = crate::plan(&data, &options)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let bootstrap = format!("rustup/archive/{}/{}/rustup-init", s.rustup_version, s.host);
        let bootstrap_hash =
            crate::checksum(&read(&root.join(format!("{bootstrap}.sha256")), 4096)?)?;
        expected.insert(bootstrap.clone(), bootstrap_hash);
        let mut required = expected.keys().cloned().collect::<BTreeSet<_>>();
        required.extend([
            format!("{bootstrap}.sha256"),
            manifest.clone(),
            format!("{manifest}.sha256"),
            format!("{manifest}.asc"),
        ]);
        if required != map.keys().cloned().collect()
            || expected.iter().any(|(p, h)| map.get(p) != Some(h))
        {
            return Err("legacy inventory omits or changes required content".into());
        }
        s.files
    } else if schema == 2 || schema == 3 {
        crate::selection::collection::publication_files(root)?
    } else {
        return Err("unsupported publication inventory schema".into());
    };
    files.sort_by_key(|f| {
        (
            f.path.contains("/channel-rust-") || f.path == "rustup/release-stable.toml",
            f.path.ends_with(".sha256"),
            f.path.clone(),
        )
    });
    let hash = crate::sha256(&raw);
    files.push(SnapshotFile {
        path: "snapshot.toml".into(),
        sha256: hash.clone(),
        bytes: raw.len() as u64,
    });
    Ok((schema as u32, files, hash))
}
fn base_url(options: &Options) -> Result<String> {
    let base = options.base_url.trim_end_matches('/');
    let (scheme, rest) = base
        .split_once("://")
        .ok_or("publication needs an absolute URL")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty()
        || !authority
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-.[]:".contains(&c))
        || path.split('/').any(|p| p == "." || p == "..")
        || !path
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._/".contains(&c))
    {
        return Err(
            "unsafe publication URL (no credentials, query, fragment or encoded paths)".into(),
        );
    }
    let local = authority == "127.0.0.1"
        || authority == "[::1]"
        || authority.parse::<std::net::SocketAddr>().is_ok_and(|a| {
            a.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                || a.ip() == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        });
    if scheme != "https" && !(scheme == "http" && options.allow_loopback_http && local) {
        return Err("HTTPS required; literal loopback HTTP needs explicit test opt-in".into());
    }
    if scheme != "https" && (options.credentials.is_some() || options.ca_file.is_some()) {
        return Err("credentials and CA configuration require HTTPS".into());
    }
    Ok(base.into())
}
fn authorization(user: &str, password: &str) -> Result<String> {
    if user.is_empty() || password.is_empty() || user.contains(':') {
        return Err("invalid publication credentials".into());
    }
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = format!("{user}:{password}");
    let mut out = String::from("Basic ");
    for chunk in input.as_bytes().chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | (chunk.get(2).copied().unwrap_or(0) as u32);
        for i in 0..4 {
            out.push(if i > chunk.len() {
                '='
            } else {
                ALPHABET[((n >> (18 - i * 6)) & 63) as usize] as char
            });
        }
    }
    Ok(out)
}
struct Client {
    agent: ureq::Agent,
    base: String,
    authorization: Option<String>,
}
impl Client {
    fn request(
        &self,
        method: &str,
        name: &str,
        body: Option<&Path>,
        len: u64,
    ) -> Result<ureq::Response> {
        let mut request = self.agent.request(method, &format!("{}/{name}", self.base));
        if let Some(auth) = &self.authorization {
            request = request.set("Authorization", auth);
        }
        let result = if let Some(path) = body {
            request
                .set("Content-Length", &len.to_string())
                .set("Content-Type", "application/octet-stream")
                .set("If-None-Match", "*")
                .send(File::open(path)?)
        } else {
            request.call()
        };
        match result {
            Ok(r) | Err(ureq::Error::Status(_, r)) => Ok(r),
            Err(_) => {
                Err("publication transport failed (check endpoint, TLS trust and network)".into())
            }
        }
    }
}
/// Validate all local content and every destination's absence before the first PUT.
/// Send the inventory marker last, verify every remote byte, and never retry writes
/// or activate a server/group. Requires an exclusive publisher and no-redeploy server.
///
/// The root must be a completed schema-1/2/3 mirror, not private working state.
/// The returned [Report] counts the inventory marker too.
///
/// # Errors
/// Refuses invalid local content, URL/CA/credentials, any pre-existing remote path,
/// TLS/auth/HTTP failures or GET size/hash mismatches. Errors after writes may leave
/// partial remote data: investigate and use fresh storage. No remote rollback,
/// retry, resume or force mode is provided.
pub fn publish(root: &Path, options: &Options) -> Result<Report> {
    let base = base_url(options)?;
    let mut builder = ureq::AgentBuilder::new()
        .redirects(0)
        .try_proxy_from_env(false)
        .timeout(Duration::from_secs(180));
    if let Some(path) = &options.ca_file {
        use ureq::rustls::{
            pki_types::{pem::PemObject, CertificateDer},
            RootCertStore,
        };
        let raw = read(path, 4 * 1024 * 1024)?;
        let mut roots = RootCertStore::empty();
        let mut count = 0;
        for cert in CertificateDer::pem_slice_iter(&raw) {
            count += 1;
            if count > 256 {
                return Err("CA bundle exceeds certificate budget".into());
            }
            roots
                .add(cert.map_err(|_| "invalid CA PEM")?)
                .map_err(|_| "invalid CA certificate")?;
        }
        if count == 0 {
            return Err("CA bundle has no certificates".into());
        }
        let config = ureq::rustls::ClientConfig::builder_with_provider(Arc::new(
            ureq::rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        builder = builder.tls_config(Arc::new(config));
    }
    let authorization = options
        .credentials
        .as_ref()
        .map(|(u, p)| authorization(u, p))
        .transpose()?;
    let client = Client {
        agent: builder.build(),
        base,
        authorization,
    };
    let (schema, files, inventory_sha256) = files(root)?;
    for f in &files {
        match client.request("HEAD", &f.path, None, 0)?.status() {
            404 => {}
            200..=299 => return Err("destination is not absent; no files were uploaded".into()),
            status => {
                return Err(format!(
                    "destination preflight refused (HTTP {status}); no files were uploaded"
                )
                .into())
            }
        }
    }
    let mut total = 0u64;
    for f in &files {
        let path = root.join(&f.path);
        if crate::resume::hash_file(&path, f.bytes)? != (f.bytes, f.sha256.clone()) {
            return Err("snapshot changed before upload".into());
        }
        let response = client.request("PUT", &f.path, Some(&path), f.bytes)?;
        if ![200, 201, 204].contains(&response.status()) {
            return Err(format!("upload failed (HTTP {})", response.status()).into());
        }
        let response = client.request("GET", &f.path, None, 0)?;
        if response.status() != 200 {
            return Err("remote verification GET failed".into());
        }
        let mut reader = response.into_reader().take(f.bytes + 1);
        let mut hasher = Sha256::new();
        let mut n = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let bytes = reader.read(&mut buffer)?;
            if bytes == 0 {
                break;
            }
            n += bytes as u64;
            hasher.update(&buffer[..bytes]);
        }
        if n != f.bytes || hex::encode(hasher.finalize()) != f.sha256 {
            return Err("remote bytes differ; incomplete publication must not be activated".into());
        }
        total = total
            .checked_add(n)
            .ok_or("publication byte count overflow")?;
    }
    Ok(Report {
        files: files.len(),
        remote_bytes_verified: total,
        inventory_sha256,
        schema,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_auth_known_vectors() {
        assert_eq!(
            authorization("Aladdin", "open sesame").unwrap(),
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert_eq!(authorization("u", "p").unwrap(), "Basic dTpw");
        assert_eq!(authorization("u", "pa").unwrap(), "Basic dTpwYQ==");
    }
    #[test]
    fn endpoints_refuse_credentials_and_nonliteral_http() {
        for base in [
            "http://localhost/x",
            "http://127.0.0.1.example/x",
            "https://u:p@example.test/x",
            "https://example.test/../x",
            "https://example.test/x?token=secret",
        ] {
            assert!(base_url(&Options {
                base_url: base.into(),
                ca_file: None,
                credentials: None,
                allow_loopback_http: true
            })
            .is_err());
        }
    }
}
