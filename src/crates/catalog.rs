//! Cargo-compatible registry catalog declarations and GUI-safe canonical editing.
use super::{budget, index_path, invalid};
use crate::{network::NetworkConfig, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Normalized registry dependency. The JSON form is always an object; TOML string shorthand is accepted by parsing.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Dependency {
    /// Cargo requirement; omitted means `*` during normalization.
    #[serde(default = "wildcard")]
    pub version: String,
    /// Canonical package name when the table key is an alias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Explicitly requested features.
    #[serde(default)]
    pub features: Vec<String>,
    /// Request the package default feature.
    #[serde(default = "yes")]
    pub default_features: bool,
    /// Preserve pasted optionality; a curated root is selected regardless.
    #[serde(default)]
    pub optional: bool,
    /// Only the built-in `crates-io` identity is accepted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}
impl Default for Dependency {
    fn default() -> Self {
        Self {
            version: wildcard(),
            package: None,
            features: vec![],
            default_features: true,
            optional: false,
            registry: None,
        }
    }
}
fn wildcard() -> String {
    "*".into()
}
fn yes() -> bool {
    true
}
fn one() -> usize {
    1
}
/// Multiple-version selection for one dependency alias.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Up to this many newest non-yanked matching versions; defaults to one.
    #[serde(default = "one")]
    pub latest: usize,
    /// Additional exact versions independent of the range.
    #[serde(default)]
    pub pins: Vec<String>,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            latest: 1,
            pins: vec![],
        }
    }
}
/// Explicit organization coverage and resource ceilings. Counts/bytes are software bounds, not approval.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Policy {
    /// Only `declared` is supported.
    pub features: String,
    /// Only resolver-2 lock-style coverage is currently qualified.
    pub resolver: String,
    /// Explicit compiler hosts for client qualification; closure covers all platforms.
    pub hosts: Vec<String>,
    /// Explicit compilation targets; no target-based archive pruning.
    pub targets: Vec<String>,
    /// Maximum distinct crate records in the final generation.
    pub max_versions: usize,
    /// Maximum final inventory file count.
    pub max_files: usize,
    /// Aggregate sparse metadata input limit.
    pub max_metadata_bytes: u64,
    /// Maximum individual archive length, including unknown-size streams.
    pub max_file_bytes: u64,
    /// Whole final generation bytes, including retained copies and generated metadata.
    pub max_final_bytes: u64,
    /// Bytes transferred across all workers and retry attempts.
    pub max_transfer_bytes: u64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            features: "declared".into(),
            resolver: "2".into(),
            hosts: vec![],
            targets: vec![],
            max_versions: 3000,
            max_files: 10000,
            max_metadata_bytes: 256 * 1024 * 1024,
            max_file_bytes: 512 * 1024 * 1024,
            max_final_bytes: 10 * 1024 * 1024 * 1024,
            max_transfer_bytes: 10 * 1024 * 1024 * 1024,
        }
    }
}
/// Immutable final destination used to generate sparse registry download URLs.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Output {
    /// Credential-free HTTPS generation prefix; may be empty in an unsaved paste proposal.
    pub base_url: String,
}
/// Nonsecret crate-specific network scheduling options.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Transport {
    /// Public operator contact in the User-Agent; never a credential.
    pub contact: String,
    /// Fixed worker count, default four, bounded to 1..16.
    pub concurrency: usize,
    /// Maximum attempts per artifact, including the first, 1..8.
    pub attempts: usize,
    /// Maximum Retry-After/backoff delay in seconds, 1..300.
    pub max_wait_seconds: u64,
}
impl Default for Transport {
    fn default() -> Self {
        Self {
            contact: String::new(),
            concurrency: 4,
            attempts: 4,
            max_wait_seconds: 60,
        }
    }
}
/// Typed JSON/Serde catalog; use [`Catalog::parse`] for TOML dependency string shorthand.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Catalog {
    /// Alias-keyed normalized registry declarations.
    pub crates: BTreeMap<String, Dependency>,
    /// Mirror-specific latest-N/pin policies, separate from Cargo dependency values.
    pub crate_rules: BTreeMap<String, Rule>,
    /// Feature/platform/resource policy.
    pub crate_policy: Policy,
    /// Final generation URL.
    pub crate_output: Output,
    /// Public contact and bounded worker/retry settings.
    pub crate_network: Transport,
    /// Optional explicit corporate routing. Contains env variable names, never secret values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkConfig>,
}
/// Machine-readable paste refusal; human messages are not parser APIs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// Malformed TOML.
    Syntax,
    /// Git dependency/source selector refused.
    Git,
    /// Local path dependency refused, even when a version is present.
    Path,
    /// Workspace inheritance needs unavailable project context.
    Workspace,
    /// Registry other than the default source.
    AlternateRegistry,
    /// Unsupported dependency or document key.
    UnknownField,
    /// Wrong type, invalid semver or value.
    InvalidValue,
    /// Whole project manifest supplied rather than a dependencies block.
    WholeManifest,
}
/// A bounded field-specific editor diagnostic, independent of Cargo prose.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Diagnostic {
    /// Dependency alias if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Offending field if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// One-based source line when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// One-based source column when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// Stable category.
    pub kind: DiagnosticKind,
    /// Bounded human context.
    pub message: String,
}
/// Paste preview. Accepted entries never imply consent to write or download anything.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PasteReport {
    /// Proposed partial catalog for editor integration; output/coverage may still need user input.
    pub catalog: Option<Catalog>,
    /// Accepted alias names.
    pub accepted: Vec<String>,
    /// All bounded dependency refusals, or one document-level refusal.
    pub rejected: Vec<Diagnostic>,
}
fn diagnostic(
    alias: Option<&str>,
    field: Option<&str>,
    kind: DiagnosticKind,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        alias: alias.map(str::to_owned),
        field: field.map(str::to_owned),
        line: None,
        column: None,
        kind,
        message: message.chars().take(512).collect(),
    }
}
fn declaration(
    alias: &str,
    value: &toml::Value,
) -> std::result::Result<Dependency, Vec<Diagnostic>> {
    if alias.is_empty()
        || alias.len() > 64
        || !alias
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(vec![diagnostic(
            Some(alias),
            None,
            DiagnosticKind::InvalidValue,
            "alias must be a canonical ASCII crate identifier",
        )]);
    }
    let result = match value {
        toml::Value::String(version) => Dependency {
            version: version.clone(),
            default_features: true,
            ..Dependency::default()
        },
        toml::Value::Table(fields) => {
            let mut errors = Vec::new();
            for key in fields.keys() {
                let kind = match key.as_str() {
                    "git" | "rev" | "branch" | "tag" => Some(DiagnosticKind::Git),
                    "path" => Some(DiagnosticKind::Path),
                    "workspace" => Some(DiagnosticKind::Workspace),
                    "version" | "package" | "features" | "default-features" | "optional"
                    | "registry" => None,
                    _ => Some(DiagnosticKind::UnknownField),
                };
                if let Some(kind) = kind {
                    errors.push(diagnostic(
                        Some(alias),
                        Some(key),
                        kind,
                        "unsupported registry dependency field",
                    ));
                }
            }
            if !errors.is_empty() {
                return Err(errors);
            }
            match value.clone().try_into::<Dependency>() {
                Ok(d) => d,
                Err(_) => {
                    return Err(vec![diagnostic(
                        Some(alias),
                        None,
                        DiagnosticKind::InvalidValue,
                        "dependency field has an invalid type",
                    )])
                }
            }
        }
        _ => {
            return Err(vec![diagnostic(
                Some(alias),
                None,
                DiagnosticKind::InvalidValue,
                "dependency must be a Cargo version string or table",
            )])
        }
    };
    let mut errors = Vec::new();
    if VersionReq::parse(&result.version).is_err() {
        errors.push(diagnostic(
            Some(alias),
            Some("version"),
            DiagnosticKind::InvalidValue,
            "invalid Cargo version requirement",
        ));
    }
    if result
        .package
        .as_ref()
        .is_some_and(|p| index_path(p).is_err())
    {
        errors.push(diagnostic(
            Some(alias),
            Some("package"),
            DiagnosticKind::InvalidValue,
            "invalid canonical package name",
        ));
    }
    if result.registry.as_deref().is_some_and(|r| r != "crates-io") {
        errors.push(diagnostic(
            Some(alias),
            Some("registry"),
            DiagnosticKind::AlternateRegistry,
            "only crates.io registry dependencies are supported",
        ));
    }
    if result.features.len() > 1024
        || result.features.iter().any(|f| {
            f.is_empty()
                || f.len() > 128
                || !f
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-+.".contains(&b))
        })
    {
        errors.push(diagnostic(
            Some(alias),
            Some("features"),
            DiagnosticKind::InvalidValue,
            "features must be bounded Cargo feature names, not forwarding expressions",
        ));
    }
    if errors.is_empty() {
        Ok(result)
    } else {
        Err(errors)
    }
}
fn error_report(errors: Vec<Diagnostic>) -> Box<dyn std::error::Error + Send + Sync> {
    let mut error = crate::Error::new(
        crate::ErrorKind::Invalid,
        "invalid crate catalog; inspect structured diagnostics",
    );
    error.report = toml::Value::try_from(errors).ok().map(|value| {
        toml::Value::Table(
            BTreeMap::from([("diagnostics".into(), value)])
                .into_iter()
                .collect(),
        )
    });
    error.into()
}
impl Catalog {
    /// Parse TOML, normalize Cargo shorthand and refuse unrelated/unsafe source forms.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 4 * 1024 * 1024 {
            return Err(budget("catalog input exceeds 4 MiB"));
        }
        let mut document: toml::Value = toml::from_str(text).map_err(|e: toml::de::Error| {
            invalid({
                let _ = e;
                "invalid catalog TOML"
            })
        })?;
        let entries = document
            .get("crates")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| invalid("catalog requires a [crates] table"))?
            .clone();
        let mut normalized = toml::map::Map::new();
        let mut errors = Vec::new();
        for (alias, value) in entries {
            match declaration(&alias, &value) {
                Ok(d) => {
                    normalized.insert(alias, toml::Value::try_from(d)?);
                }
                Err(e) => errors.extend(e),
            }
        }
        if !errors.is_empty() {
            return Err(error_report(errors));
        }
        document
            .as_table_mut()
            .unwrap()
            .insert("crates".into(), toml::Value::Table(normalized));
        let catalog: Self = document
            .try_into()
            .map_err(|_| invalid("unknown catalog field or invalid field type"))?;
        catalog.validate()?;
        Ok(catalog)
    }
    /// Check typed/JSON inputs before use; this does not inspect environment, network or files.
    pub fn validate(&self) -> Result<()> {
        if self.crates.is_empty() || self.crates.len() > 4096 {
            return Err(budget("catalog needs 1..4096 aliases"));
        }
        for (alias, d) in &self.crates {
            declaration(alias, &toml::Value::try_from(d)?).map_err(error_report)?;
        }
        for (alias, rule) in &self.crate_rules {
            if !self.crates.contains_key(alias) {
                return Err(invalid("crate rule references an unknown alias"));
            }
            if rule.latest > 32
                || rule.pins.len() > 128
                || (rule.latest == 0 && rule.pins.is_empty())
            {
                return Err(budget(
                    "crate version rule exceeds bounds or selects nothing",
                ));
            }
            let mut pins = BTreeSet::new();
            for pin in &rule.pins {
                Version::parse(pin).map_err(|_| invalid("pins require exact semver versions"))?;
                if !pins.insert(pin) {
                    return Err(invalid("duplicate crate pin"));
                }
            }
        }
        let p = &self.crate_policy;
        if p.features != "declared" || p.resolver != "2" {
            return Err(invalid(
                "only declared features and resolver-2 coverage are supported",
            ));
        }
        if p.hosts.len() > 128
            || p.targets.len() > 128
            || p.hosts.iter().chain(&p.targets).any(|h| {
                h.is_empty()
                    || h.len() > 128
                    || !h
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
        {
            return Err(invalid("invalid host/target declarations"));
        }
        if p.max_versions == 0
            || p.max_versions > 10000
            || p.max_files == 0
            || p.max_files > 40000
            || p.max_metadata_bytes == 0
            || p.max_metadata_bytes > 1024 * 1024 * 1024
            || p.max_file_bytes == 0
            || p.max_file_bytes > 512 * 1024 * 1024
            || p.max_final_bytes == 0
            || p.max_transfer_bytes == 0
            || p.max_final_bytes > i64::MAX as u64
            || p.max_transfer_bytes > i64::MAX as u64
        {
            return Err(budget(
                "crate catalog resource limits exceed supported bounds",
            ));
        }
        let n = &self.crate_network;
        if !(1..=16).contains(&n.concurrency)
            || !(1..=8).contains(&n.attempts)
            || !(1..=300).contains(&n.max_wait_seconds)
            || n.contact.len() > 512
            || !n.contact.is_ascii()
            || n.contact.chars().any(char::is_control)
        {
            return Err(invalid("invalid crate contact/concurrency/retry policy"));
        }
        if let Some(network) = &self.network {
            network.validate()?;
        }
        Ok(())
    }
    /// Validate and serialize canonical TOML. Comments, source formatting and key order are lost.
    pub fn to_toml(&self) -> Result<String> {
        self.validate()?;
        Ok(toml::to_string_pretty(self)?)
    }
    /// Preview a standalone dependencies block, collecting all field refusals. Whole manifests are refused.
    pub fn from_dependencies(text: &str) -> PasteReport {
        let mut report = PasteReport {
            catalog: None,
            accepted: vec![],
            rejected: vec![],
        };
        if text.len() > 4 * 1024 * 1024 {
            report.rejected.push(diagnostic(
                None,
                None,
                DiagnosticKind::InvalidValue,
                "paste exceeds 4 MiB",
            ));
            return report;
        }
        let document: toml::Value = match toml::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                let mut d = diagnostic(None, None, DiagnosticKind::Syntax, "invalid TOML paste");
                if let Some(span) = e.span() {
                    let prefix = &text[..span.start];
                    d.line = Some(prefix.bytes().filter(|b| *b == b'\n').count() + 1);
                    d.column = Some(prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1);
                }
                report.rejected.push(d);
                return report;
            }
        };
        let Some(table) = document.as_table() else {
            report.rejected.push(diagnostic(
                None,
                None,
                DiagnosticKind::InvalidValue,
                "paste must be a dependencies table",
            ));
            return report;
        };
        if table.len() != 1 || !table.contains_key("dependencies") {
            report.rejected.push(diagnostic(None,None,DiagnosticKind::WholeManifest,"paste only a standalone [dependencies] block; project/workspace context is not imported"));
            return report;
        }
        let Some(entries) = table["dependencies"].as_table() else {
            report.rejected.push(diagnostic(
                None,
                Some("dependencies"),
                DiagnosticKind::InvalidValue,
                "dependencies must be a table",
            ));
            return report;
        };
        if entries.len() > 4096 {
            report.rejected.push(diagnostic(
                None,
                None,
                DiagnosticKind::InvalidValue,
                "paste exceeds 4096 aliases",
            ));
            return report;
        }
        let positions: BTreeMap<String, BTreeMap<String, toml::Spanned<toml::Value>>> =
            toml::from_str(text).unwrap_or_default();
        let mut catalog = Self::default();
        for (alias, value) in entries {
            match declaration(alias, value) {
                Ok(d) => {
                    report.accepted.push(alias.clone());
                    catalog.crates.insert(alias.clone(), d);
                }
                Err(mut errors) => {
                    if let Some(span) = positions
                        .get("dependencies")
                        .and_then(|p| p.get(alias))
                        .map(|v| v.span())
                    {
                        let prefix = &text[..span.start];
                        for d in &mut errors {
                            d.line = Some(prefix.bytes().filter(|b| *b == b'\n').count() + 1);
                            d.column =
                                Some(prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1);
                        }
                    }
                    report.rejected.extend(errors)
                }
            }
        }
        if !catalog.crates.is_empty() {
            report.catalog = Some(catalog);
        }
        report
    }
}

impl Catalog {
    /// Atomically write canonical TOML to an explicit path. Replacement requires opt-in and an existing valid catalog.
    ///
    /// Preserves existing permissions and refuses links, unrelated/invalid files and competing writers.
    /// A persistent zero-byte sidecar lock remains beside the catalog. Comments/order are not preserved.
    pub fn write(&self, path: &std::path::Path, replace: bool) -> Result<()> {
        use std::{
            fs::{self, File},
            io::Write,
        };
        let encoded = self.to_toml()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        if !fs::symlink_metadata(parent)?.is_dir()
            || fs::symlink_metadata(parent)?.file_type().is_symlink()
        {
            return Err(invalid("catalog parent must be a real existing directory"));
        }
        let filename = path
            .file_name()
            .ok_or_else(|| invalid("catalog path needs a file name"))?;
        let mut lockname = filename.to_os_string();
        lockname.push(".lock");
        let lockpath = parent.join(lockname);
        if let Ok(m) = fs::symlink_metadata(&lockpath) {
            if !m.is_file() || m.file_type().is_symlink() || m.len() != 0 {
                return Err(crate::Error::new(
                    crate::ErrorKind::Locked,
                    "invalid catalog writer lock",
                )
                .into());
            }
        }
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lockpath)?;
        lock.try_lock().map_err(|_| {
            crate::Error::new(crate::ErrorKind::Locked, "catalog has a competing writer")
        })?;
        let before = match fs::symlink_metadata(path) {
            Ok(m) => {
                if !replace {
                    return Err(crate::Error::new(
                        crate::ErrorKind::OutputExists,
                        "catalog already exists; replacement is explicit",
                    )
                    .into());
                }
                if !m.is_file() || m.file_type().is_symlink() || m.len() > 4 * 1024 * 1024 {
                    return Err(invalid("existing catalog must be a bounded regular file"));
                }
                let bytes = fs::read(path)?;
                Self::parse(std::str::from_utf8(&bytes)?)
                    .map_err(|_| invalid("refusing to replace an unrelated or invalid catalog"))?;
                Some((bytes, m.permissions()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(encoded.as_bytes())?;
        if let Some((_, permissions)) = &before {
            temp.as_file().set_permissions(permissions.clone())?;
        }
        temp.as_file().sync_all()?;
        match before {
            Some((bytes, _)) => {
                let m = fs::symlink_metadata(path)?;
                if m.file_type().is_symlink() || !m.is_file() || fs::read(path)? != bytes {
                    return Err(crate::Error::new(
                        crate::ErrorKind::Locked,
                        "catalog changed before replacement",
                    )
                    .into());
                }
                temp.persist(path).map_err(|e| e.error)?;
            }
            None => {
                temp.persist_noclobber(path).map_err(|e| e.error)?;
            }
        }
        drop(lock);
        Ok(())
    }
}
