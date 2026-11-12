//! Explicit corporate-network routing, additive TLS trust and display-only diagnostics.
use crate::{events, Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::File,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use ureq::rustls::{
    self,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{pem::PemObject, CertificateDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};

/// Explicit proxy selection; the default never reads environment variables.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum ProxyMode {
    /// Connect directly.
    #[default]
    Direct,
    /// Explicitly resolve the documented environment precedence when building the source.
    Environment,
    /// Credential-free HTTP CONNECT proxy URL.
    Manual(String),
}
impl fmt::Debug for ProxyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Direct => "Direct",
            Self::Environment => "Environment",
            Self::Manual(_) => "Manual([configured endpoint])",
        })
    }
}
/// Memory-only proxy authentication. Never serialize or log credential values.
#[derive(Clone)]
pub struct ProxyCredentials {
    /// Proxy username; no colon, control characters or empty value.
    pub username: String,
    /// Proxy password; never written to config or event output.
    pub password: String,
}
impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyCredentials([redacted])")
    }
}
/// Runtime options for a single explicit upstream or publisher transport.
#[derive(Clone, Debug)]
pub struct NetworkOptions {
    /// Direct, explicitly environment-derived, or manual proxy.
    pub proxy: ProxyMode,
    /// Memory-only credentials, never persisted by this crate.
    pub proxy_credentials: Option<ProxyCredentials>,
    /// Host/domain exclusions, optional ports or `*`; no CIDR/wildcard patterns.
    pub no_proxy: Vec<String>,
    /// PEM certificates to add to Mozilla roots, never replace them.
    pub extra_ca_file: Option<PathBuf>,
    /// Socket connect timeout; OS name resolution may take longer.
    pub connect_timeout: Duration,
    /// Per-socket-read/write timeout; not an overall operation deadline.
    pub read_timeout: Duration,
}
impl Default for NetworkOptions {
    fn default() -> Self {
        Self {
            proxy: ProxyMode::Direct,
            proxy_credentials: None,
            no_proxy: vec![],
            extra_ca_file: None,
            connect_timeout: Duration::from_secs(180),
            read_timeout: Duration::from_secs(180),
        }
    }
}
fn seconds() -> u64 {
    180
}
fn direct() -> String {
    "direct".into()
}
/// Serializable configuration contains only credential environment variable names.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    /// `direct` (default), `manual` or `environment`.
    #[serde(default = "direct")]
    pub proxy_mode: String,
    /// Manual credential-free HTTP proxy URL.
    pub proxy_url: Option<String>,
    /// Explicit environment variable name for a proxy username, never its value.
    pub proxy_username_env: Option<String>,
    /// Explicit environment variable name for a proxy password, never its value.
    pub proxy_password_env: Option<String>,
    /// Explicit exclusions; empty by default.
    #[serde(default)]
    pub no_proxy: Vec<String>,
    /// Optional additional CA PEM file.
    pub extra_ca_file: Option<PathBuf>,
    /// Positive seconds; defaults to 180, maximum one day.
    #[serde(default = "seconds")]
    pub connect_timeout_seconds: u64,
    /// Positive seconds per read/write; defaults to 180, maximum one day.
    #[serde(default = "seconds")]
    pub read_timeout_seconds: u64,
}
impl fmt::Debug for NetworkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkConfig")
            .field("proxy_mode", &self.proxy_mode)
            .field(
                "proxy_url",
                &self.proxy_url.as_ref().map(|_| "[configured endpoint]"),
            )
            .finish_non_exhaustive()
    }
}
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy_mode: direct(),
            proxy_url: None,
            proxy_username_env: None,
            proxy_password_env: None,
            no_proxy: vec![],
            extra_ca_file: None,
            connect_timeout_seconds: 180,
            read_timeout_seconds: 180,
        }
    }
}
fn invalid(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::Invalid, message).into()
}
fn clean(s: &str) -> bool {
    !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control)
}
fn env_value(name: &str) -> Result<Option<String>> {
    std::env::var(name).map(Some).or_else(|e| {
        if e == std::env::VarError::NotPresent {
            Ok(None)
        } else {
            Err(invalid("network environment value must be UTF-8"))
        }
    })
}
impl NetworkConfig {
    /// Validate syntax without reading secrets, environment or CA files.
    pub fn validate(&self) -> Result<()> {
        if !["direct", "manual", "environment"].contains(&self.proxy_mode.as_str())
            || self.proxy_url.is_some() != (self.proxy_mode == "manual")
        {
            return Err(invalid(
                "manual proxy mode requires a URL; other modes forbid it",
            ));
        }
        if let Some(url) = &self.proxy_url {
            proxy_url(url, false)?;
        }
        for name in [&self.proxy_username_env, &self.proxy_password_env]
            .into_iter()
            .flatten()
        {
            if name.is_empty()
                || name.len() > 128
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || name.as_bytes()[0].is_ascii_digit()
            {
                return Err(invalid("invalid credential environment variable name"));
            }
        }
        if self.proxy_username_env.is_some() != self.proxy_password_env.is_some() {
            return Err(invalid(
                "proxy credential variable names must be supplied together",
            ));
        }
        validate_limits(
            Duration::from_secs(self.connect_timeout_seconds),
            Duration::from_secs(self.read_timeout_seconds),
            &self.no_proxy,
        )?;
        Ok(())
    }
    /// Explicitly resolve named secret variables. Config reads alone never read credentials.
    pub fn resolve(&self) -> Result<NetworkOptions> {
        self.validate()?;
        let credentials = match (&self.proxy_username_env, &self.proxy_password_env) {
            (Some(u), Some(p)) => Some(ProxyCredentials {
                username: env_value(u)?
                    .ok_or_else(|| invalid("proxy username variable is not set"))?,
                password: env_value(p)?
                    .ok_or_else(|| invalid("proxy password variable is not set"))?,
            }),
            _ => None,
        };
        Ok(NetworkOptions {
            proxy: match self.proxy_mode.as_str() {
                "manual" => ProxyMode::Manual(self.proxy_url.clone().unwrap()),
                "environment" => ProxyMode::Environment,
                _ => ProxyMode::Direct,
            },
            proxy_credentials: credentials,
            no_proxy: self.no_proxy.clone(),
            extra_ca_file: self.extra_ca_file.clone(),
            connect_timeout: Duration::from_secs(self.connect_timeout_seconds),
            read_timeout: Duration::from_secs(self.read_timeout_seconds),
        })
    }
}
/// Sanitized environment selection, suitable for display without secret values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentProxy {
    /// Selected proxy endpoint without userinfo.
    pub proxy_url: Option<String>,
    /// Parsed NO_PROXY entries.
    pub no_proxy: Vec<String>,
    /// Whether the selected environment URL contained credentials (never their values).
    pub credentials_present: bool,
}
#[derive(Clone)]
struct Proxy {
    host: String,
    port: u16,
    url: String,
    credentials: Option<ProxyCredentials>,
}
fn proxy_url(url: &str, allow_credentials: bool) -> Result<Proxy> {
    if !clean(url) {
        return Err(invalid("invalid proxy URL"));
    }
    let parsed = ureq::get(url)
        .request_url()
        .map_err(|_| invalid("invalid proxy URL"))?;
    let u = parsed.as_url();
    if u.scheme() != "http"
        || u.path() != "/"
        || u.query().is_some()
        || u.fragment().is_some()
        || u.host_str().is_none()
        || u.host_str().is_some_and(|h| h.contains(':'))
    {
        return Err(invalid(
            "proxy must be an HTTP CONNECT hostname/IPv4 URL without path, query or fragment",
        ));
    }
    let credentials = if !u.username().is_empty() || u.password().is_some() {
        if !allow_credentials {
            return Err(invalid(
                "proxy URL credentials are forbidden; use explicit secret inputs",
            ));
        }
        // URL userinfo is percent-encoded. Decode without interpreting '+' as space.
        fn decode(s: &str) -> Result<String> {
            let mut bytes = vec![];
            let b = s.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'%' {
                    if i + 2 >= b.len() {
                        return Err(invalid("invalid proxy credential encoding"));
                    }
                    let digit = |v: u8| match v {
                        b'0'..=b'9' => Some(v - b'0'),
                        b'a'..=b'f' => Some(v - b'a' + 10),
                        b'A'..=b'F' => Some(v - b'A' + 10),
                        _ => None,
                    };
                    let hi = digit(b[i + 1])
                        .ok_or_else(|| invalid("invalid proxy credential encoding"))?;
                    let lo = digit(b[i + 2])
                        .ok_or_else(|| invalid("invalid proxy credential encoding"))?;
                    bytes.push(hi * 16 + lo);
                    i += 3;
                } else {
                    bytes.push(b[i]);
                    i += 1;
                }
            }
            String::from_utf8(bytes).map_err(|_| invalid("proxy credentials must be UTF-8"))
        }
        Some(ProxyCredentials {
            username: decode(u.username())?,
            password: decode(u.password().unwrap_or(""))?,
        })
    } else {
        None
    };
    let host = u.host_str().unwrap().to_owned();
    let port = u.port_or_known_default().unwrap_or(80);
    if port == 0 {
        return Err(invalid("proxy port must be positive"));
    }
    if let Some(c) = &credentials {
        validate_credentials(c)?;
    }
    Ok(Proxy {
        url: format!("http://{host}:{port}"),
        host,
        port,
        credentials,
    })
}
fn environment() -> Result<(Option<Proxy>, Vec<String>)> {
    let mut selected = None;
    for name in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        if let Some(v) = env_value(name)?.filter(|v| !v.trim().is_empty()) {
            selected = Some(proxy_url(&v, true)?);
            break;
        }
    }
    let raw = env_value("NO_PROXY")?
        .filter(|v| !v.trim().is_empty())
        .or(env_value("no_proxy")?.filter(|v| !v.trim().is_empty()))
        .unwrap_or_default();
    let list = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for item in &list {
        exclusion(item)?;
    }
    Ok((selected, list))
}
/// Read environment only on explicit request, using the documented precedence.
pub fn detect_environment() -> Result<EnvironmentProxy> {
    let (p, no_proxy) = environment()?;
    Ok(EnvironmentProxy {
        proxy_url: p.as_ref().map(|p| p.url.clone()),
        credentials_present: p.as_ref().is_some_and(|p| p.credentials.is_some()),
        no_proxy,
    })
}
fn validate_credentials(c: &ProxyCredentials) -> Result<()> {
    if !clean(&c.username) || !clean(&c.password) || c.username.contains([':', '@']) {
        return Err(invalid("invalid proxy credentials"));
    }
    Ok(())
}
fn exclusion(value: &str) -> Result<(String, Option<u16>)> {
    if value == "*" {
        return Ok((value.into(), None));
    }
    if !clean(value) || value.len() > 255 || value.contains(['/', '@', '*', ' ']) {
        return Err(invalid("unsupported NO_PROXY entry"));
    }
    if value.parse::<std::net::Ipv6Addr>().is_ok() {
        return Ok((value.to_ascii_lowercase(), None));
    }
    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| invalid("invalid NO_PROXY IPv6 literal"))?;
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(invalid("invalid NO_PROXY IPv6 literal"));
        }
        (
            host,
            if tail.is_empty() {
                None
            } else {
                Some(
                    tail.strip_prefix(':')
                        .ok_or_else(|| invalid("invalid NO_PROXY port"))?
                        .parse::<u16>()
                        .map_err(|_| invalid("invalid NO_PROXY port"))?,
                )
            },
        )
    } else if let Some((h, p)) = value.rsplit_once(':') {
        (
            h,
            Some(
                p.parse::<u16>()
                    .map_err(|_| invalid("invalid NO_PROXY port"))?,
            ),
        )
    } else {
        (value, None)
    };
    let host = host
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty()
        || port == Some(0)
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-:".contains(&c))
    {
        return Err(invalid("invalid NO_PROXY host"));
    }
    Ok((host, port))
}
/// Test exclusions using host/domain boundaries; does not perform DNS resolution.
pub fn is_excluded(host: &str, port: u16, entries: &[String]) -> Result<bool> {
    let host = host
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let mut matched = false;
    for e in entries {
        let (h, p) = exclusion(e)?;
        matched |= (p.is_none() || p == Some(port))
            && (h == "*"
                || host == h
                || (h.parse::<std::net::IpAddr>().is_err() && host.ends_with(&format!(".{h}"))));
    }
    Ok(matched)
}
fn validate_limits(connect: Duration, read: Duration, entries: &[String]) -> Result<()> {
    if connect.is_zero()
        || read.is_zero()
        || connect > Duration::from_secs(86400)
        || read > Duration::from_secs(86400)
        || entries.len() > 256
    {
        return Err(invalid("network timeout/exclusion bounds exceeded"));
    }
    for e in entries {
        exclusion(e)?;
    }
    Ok(())
}
fn route(options: &NetworkOptions, host: &str, port: u16) -> Result<Option<Proxy>> {
    validate_limits(
        options.connect_timeout,
        options.read_timeout,
        &options.no_proxy,
    )?;
    let (mut proxy, mut exclusions) = match &options.proxy {
        ProxyMode::Direct => (None, vec![]),
        ProxyMode::Manual(url) => (Some(proxy_url(url, false)?), vec![]),
        ProxyMode::Environment => environment()?,
    };
    exclusions.extend(options.no_proxy.iter().cloned());
    if let Some(c) = &options.proxy_credentials {
        validate_credentials(c)?;
        let p = proxy
            .as_mut()
            .ok_or_else(|| invalid("proxy credentials require a selected proxy"))?;
        if p.credentials.is_some() {
            return Err(invalid("conflicting proxy credential sources"));
        }
        p.credentials = Some(c.clone());
    }
    if is_excluded(host, port, &exclusions)? {
        return Ok(None);
    }
    Ok(proxy)
}
/// Bounded PEM loader shared with publication. Certificates are not private keys.
pub(crate) fn ca_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(invalid("CA bundle must be a regular file"));
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("CA bundle must be a regular file"));
    }
    let mut raw = vec![];
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut raw)?;
    if raw.len() > 4 * 1024 * 1024 {
        return Err(Error::new(ErrorKind::Budget, "CA bundle exceeds byte budget").into());
    }
    let mut certs = vec![];
    for c in CertificateDer::pem_slice_iter(&raw) {
        if certs.len() == 256 {
            return Err(
                Error::new(ErrorKind::Budget, "CA certificate count exceeds budget").into(),
            );
        }
        certs.push(c.map_err(|_| Error::new(ErrorKind::Tls, "invalid CA PEM"))?);
    }
    if certs.is_empty() {
        return Err(Error::new(ErrorKind::Tls, "CA bundle has no certificates").into());
    }
    Ok(certs)
}
/// Display-only certificate metadata, not a trust or CA suitability assertion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// Bounded subject distinguished name.
    pub subject: String,
    /// Bounded issuer distinguished name; the issuer certificate may not have been sent.
    pub issuer: String,
    /// UTC expiry timestamp from the certificate.
    pub not_after: String,
    /// SHA-256 of the complete DER certificate, lowercase hexadecimal.
    pub sha256: String,
}
fn certificate_info(der: &[u8]) -> Result<CertificateInfo> {
    if der.len() > 256 * 1024 {
        return Err(Error::new(
            ErrorKind::Budget,
            "certificate exceeds diagnostic byte budget",
        )
        .into());
    }
    let (rest, c) = x509_parser::parse_x509_certificate(der)
        .map_err(|_| Error::new(ErrorKind::Tls, "certificate metadata could not be parsed"))?;
    if !rest.is_empty() {
        return Err(Error::new(ErrorKind::Tls, "certificate has trailing bytes").into());
    }
    let bounded = |v: String| v.chars().filter(|c| !c.is_control()).take(4096).collect();
    Ok(CertificateInfo {
        subject: bounded(c.subject().to_string()),
        issuer: bounded(c.issuer().to_string()),
        not_after: c
            .validity()
            .not_after
            .to_rfc2822()
            .map_err(|_| Error::new(ErrorKind::Tls, "invalid certificate expiry"))?,
        sha256: crate::sha256(der),
    })
}
/// Inspect every PEM certificate without trusting it or making network requests.
pub fn inspect_ca_file(path: &Path) -> Result<Vec<CertificateInfo>> {
    ca_certificates(path)?
        .iter()
        .map(|c| certificate_info(c.as_ref()))
        .collect()
}
#[derive(Debug)]
struct RecordingVerifier {
    inner: Arc<rustls::client::WebPkiServerVerifier>,
    seen: Arc<Mutex<Vec<CertificateInfo>>>,
}
impl ServerCertVerifier for RecordingVerifier {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let mut seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        seen.clear();
        for c in std::iter::once(end).chain(intermediates).take(16) {
            if let Ok(info) = certificate_info(c.as_ref()) {
                seen.push(info);
            }
        }
        drop(seen);
        self.inner
            .verify_server_cert(end, intermediates, name, ocsp, now)
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        sig: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, sig)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        sig: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, sig)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
fn tls_config(
    path: Option<&Path>,
    seen: Option<Arc<Mutex<Vec<CertificateInfo>>>>,
) -> Result<Arc<rustls::ClientConfig>> {
    tls_config_with_base(None, path, seen)
}
fn tls_config_with_base(
    base: Option<&Path>,
    extra: Option<&Path>,
    seen: Option<Arc<Mutex<Vec<CertificateInfo>>>>,
) -> Result<Arc<rustls::ClientConfig>> {
    let mut roots = if base.is_some() {
        rustls::RootCertStore::empty()
    } else {
        rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        }
    };
    for path in [base, extra].into_iter().flatten() {
        for c in ca_certificates(path)? {
            roots
                .add(c)
                .map_err(|_| Error::new(ErrorKind::Tls, "invalid CA certificate"))?;
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;
    let config = if let Some(seen) = seen {
        let inner =
            rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()?;
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(RecordingVerifier { inner, seen }))
            .with_no_client_auth()
    } else {
        builder.with_root_certificates(roots).with_no_client_auth()
    };
    Ok(Arc::new(config))
}
/// Construct an explicit target transport; caller still enforces target/origin policy.
pub(crate) fn agent(options: &NetworkOptions, host: &str, port: u16) -> Result<ureq::Agent> {
    agent_with_base_ca(options, host, port, None)
}
pub(crate) fn agent_with_base_ca(
    options: &NetworkOptions,
    host: &str,
    port: u16,
    base: Option<&Path>,
) -> Result<ureq::Agent> {
    let proxy = route(options, host, port)?;
    let mut builder = ureq::AgentBuilder::new()
        .redirects(0)
        .try_proxy_from_env(false)
        .timeout_connect(options.connect_timeout)
        .timeout_read(options.read_timeout)
        .timeout_write(options.read_timeout)
        .tls_config(tls_config_with_base(
            base,
            options.extra_ca_file.as_deref(),
            None,
        )?);
    if let Some(p) = proxy {
        let url = match p.credentials {
            Some(c) => format!("http://{}:{}@{}:{}", c.username, c.password, p.host, p.port),
            None => p.url,
        };
        builder =
            builder.proxy(ureq::Proxy::new(url).map_err(|_| invalid("invalid CONNECT proxy"))?);
    }
    Ok(builder.build())
}
/// Sanitize transport failures at the credential-bearing transport boundary.
pub(crate) fn transport_error(e: ureq::Error) -> Box<dyn std::error::Error + Send + Sync> {
    let kind = match &e {
        ureq::Error::Status(401 | 403 | 407, _) => ErrorKind::Auth,
        ureq::Error::Transport(t) if t.kind() == ureq::ErrorKind::ProxyUnauthorized => {
            ErrorKind::Auth
        }
        _ => crate::error_kind(&e),
    };
    Error::new(
        kind,
        match kind {
            ErrorKind::Auth => "network authentication was refused",
            ErrorKind::Tls => "TLS verification or handshake failed",
            _ => "network request failed; check routing, proxy authentication and timeouts",
        },
    )
    .into()
}
/// Result of an individual ordered connection check.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// This stage succeeded.
    Passed,
    /// This stage failed.
    Failed,
    /// Not needed or dependent on a failed stage.
    Skipped,
}
/// Bounded typed evidence from one connection stage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckStep {
    /// Stable stage name: dns/connect/proxy_auth/tls/download.
    pub name: String,
    /// Stage outcome.
    pub status: CheckStatus,
    /// Stable failure kind, absent for success/skips.
    pub error_kind: Option<ErrorKind>,
    /// Sanitized context, never raw authenticated transport errors.
    pub message: String,
    /// Certificates actually presented, if available.
    pub certificates: Vec<CertificateInfo>,
}
/// Ordered diagnostics; failure is represented per step, never implicit success.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkCheckReport {
    /// All required stages passed.
    pub ok: bool,
    /// Ordered stages, including skipped dependencies.
    pub steps: Vec<CheckStep>,
}
fn basic(credentials: &ProxyCredentials) -> String {
    // Same bounded encoding as native publication; not a URL.
    const B: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let data = format!("{}:{}", credentials.username, credentials.password);
    let mut out = String::new();
    for chunk in data.as_bytes().chunks(3) {
        let a = chunk[0] as u32;
        let b = *chunk.get(1).unwrap_or(&0) as u32;
        let c = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(B[((n >> 18) & 63) as usize] as char);
        out.push(B[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
/// Diagnose the official small-file path, preserving ordinary TLS verification and heartbeats.
pub fn check_network_with_control(
    options: &NetworkOptions,
    control: &events::Control,
) -> Result<NetworkCheckReport> {
    control.run("check-network", || check(options))
}
fn check(options: &NetworkOptions) -> Result<NetworkCheckReport> {
    let host = "static.rust-lang.org";
    let proxy = route(options, host, 443)?;
    let mut report = NetworkCheckReport {
        ok: false,
        steps: vec![],
    };
    let mut failure = false;
    let mut step =
        |name: &str, result: Result<()>, certificates: Vec<CertificateInfo>, skip: bool| {
            let (status, error_kind, message) = if skip {
                (
                    CheckStatus::Skipped,
                    None,
                    "not required or an earlier stage failed".into(),
                )
            } else {
                match result {
                    Ok(()) => (CheckStatus::Passed, None, "stage completed".into()),
                    Err(e) => {
                        failure = true;
                        (
                            CheckStatus::Failed,
                            Some(crate::error_kind(e.as_ref())),
                            e.to_string(),
                        )
                    }
                }
            };
            report.steps.push(CheckStep {
                name: name.into(),
                status,
                error_kind,
                message,
                certificates,
            });
        };
    let target = proxy
        .as_ref()
        .map(|p| (p.host.as_str(), p.port))
        .unwrap_or((host, 443));
    events::stage("dns")?;
    let addresses = target
        .to_socket_addrs()
        .map(|v| v.take(32).collect::<Vec<_>>())
        .map_err(|_| Error::new(ErrorKind::Network, "DNS resolution failed"));
    step(
        "dns",
        addresses
            .as_ref()
            .map(|_| ())
            .map_err(|_| Error::new(ErrorKind::Network, "DNS resolution failed").into()),
        vec![],
        false,
    );
    let mut socket = None;
    if let Ok(addresses) = addresses {
        events::stage("connect")?;
        for addr in addresses {
            events::check()?;
            if let Ok(s) = TcpStream::connect_timeout(&addr, options.connect_timeout) {
                s.set_read_timeout(Some(options.read_timeout))?;
                s.set_write_timeout(Some(options.read_timeout))?;
                socket = Some(s);
                break;
            }
        }
        step(
            "connect",
            if socket.is_some() {
                Ok(())
            } else {
                Err(Error::new(ErrorKind::Network, "socket connect failed or timed out").into())
            },
            vec![],
            false,
        );
    } else {
        step("connect", Ok(()), vec![], true);
    }
    let mut tunnel = true;
    if let (Some(p), Some(s)) = (&proxy, &mut socket) {
        events::stage("proxy_auth")?;
        let result = (|| -> Result<()> {
            let auth = p
                .credentials
                .as_ref()
                .map(|c| format!("Proxy-Authorization: Basic {}\r\n", basic(c)))
                .unwrap_or_default();
            write!(
                s,
                "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n{auth}\r\n"
            )
            .map_err(|_| Error::new(ErrorKind::Network, "proxy request failed"))?;
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                events::check()?;
                if header.len() >= 16384 {
                    return Err(Error::new(
                        ErrorKind::Budget,
                        "proxy response headers exceed budget",
                    )
                    .into());
                }
                let mut b = [0];
                if s.read(&mut b).map_err(|_| {
                    Error::new(ErrorKind::Network, "proxy response timed out or failed")
                })? == 0
                {
                    return Err(
                        Error::new(ErrorKind::Network, "proxy closed the connection").into(),
                    );
                }
                header.push(b[0]);
            }
            let end = header
                .windows(2)
                .position(|v| v == b"\r\n")
                .ok_or_else(|| Error::new(ErrorKind::Network, "invalid proxy status line"))?;
            let line = std::str::from_utf8(&header[..end])
                .map_err(|_| Error::new(ErrorKind::Network, "invalid proxy status line"))?;
            let mut words = line.split_ascii_whitespace();
            let protocol = words.next().unwrap_or("");
            let code = words.next().unwrap_or("");
            if !["HTTP/1.0", "HTTP/1.1"].contains(&protocol)
                || code.len() != 3
                || !code.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(Error::new(ErrorKind::Network, "invalid proxy status line").into());
            }
            let status = code
                .parse::<u16>()
                .map_err(|_| Error::new(ErrorKind::Network, "invalid proxy status code"))?;
            if [401, 403, 407].contains(&status) {
                return Err(Error::new(ErrorKind::Auth, "proxy authentication refused").into());
            }
            if status != 200 {
                return Err(Error::new(ErrorKind::Network, "proxy CONNECT refused").into());
            }
            Ok(())
        })();
        tunnel = result.is_ok();
        step("proxy_auth", result, vec![], false);
    } else {
        step("proxy_auth", Ok(()), vec![], true);
    }
    let mut tls_ok = false;
    if tunnel {
        if let Some(mut socket) = socket {
            events::stage("tls")?;
            let seen = Arc::new(Mutex::new(vec![]));
            let result = (|| -> Result<()> {
                let config = tls_config(options.extra_ca_file.as_deref(), Some(seen.clone()))?;
                let mut connection = rustls::ClientConnection::new(
                    config,
                    ServerName::try_from(host).unwrap().to_owned(),
                )?;
                connection
                    .complete_io(&mut socket)
                    .map(|_| ())
                    .map_err(|_| {
                        Error::new(ErrorKind::Tls, "TLS verification or handshake failed").into()
                    })
            })();
            tls_ok = result.is_ok();
            let certs = seen.lock().unwrap_or_else(|p| p.into_inner()).clone();
            step("tls", result, certs, false);
        } else {
            step("tls", Ok(()), vec![], true);
        }
    } else {
        step("tls", Ok(()), vec![], true);
    }
    if tls_ok {
        events::stage("download")?;
        let source = crate::HttpsSource::with_network(options)?;
        let mut data = vec![];
        let result = crate::Source::fetch(
            &source,
            "https://static.rust-lang.org/dist/channel-rust-stable.toml.sha256",
            &mut data,
            4096,
        )
        .and_then(|()| crate::checksum(&data).map(|_| ()));
        step("download", result, vec![], false);
    } else {
        step("download", Ok(()), vec![], true);
    }
    events::check()?;
    report.ok = !failure;
    Ok(report)
}

/// Keep remote read failures typed and credential-free; writer/cancellation errors remain distinct.
pub(crate) struct NetworkReader<R>(pub R);
impl<R: Read> Read for NetworkReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf).map_err(|_| {
            std::io::Error::other(Error::new(
                ErrorKind::Network,
                "network read failed or timed out",
            ))
        })
    }
}
