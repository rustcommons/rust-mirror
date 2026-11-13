//! Self-contained, provenance-bearing Cargo.lock pin sources. No network or Cargo subprocesses.
mod parse;
use super::{catalog::Catalog, index_path, invalid};
use crate::{events, Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Maximum original bytes per lock input.
pub const INPUT_LIMIT: usize = 16 * 1024 * 1024;
/// Maximum bytes across an import transaction or serialized catalog.
pub const CATALOG_LIMIT: usize = 64 * 1024 * 1024;
/// Explicit, memory-only lock input; original text/paths are never persisted.
#[derive(Clone)]
pub struct LockInput {
    /// Stable operator-assigned source name.
    pub source_name: String,
    /// Original basename, without a directory.
    pub file_name: String,
    /// Exact UTF-8 input bytes represented as text, preserving line endings.
    pub text: String,
}
impl std::fmt::Debug for LockInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockInput")
            .field("source_name", &safe(&self.source_name, 128))
            .field("file_name", &safe(&self.file_name, 255))
            .field("text", &"<redacted>")
            .finish()
    }
}
/// Registry identity or an explicitly unmirrored external package.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Official crates.io, including its sparse spelling.
    CratesIo,
    /// Git checkout must be provisioned separately.
    Git,
    /// Source-less path/workspace package; these cannot be distinguished in a lock.
    Local,
}
/// Fully disambiguated dependency identity; no credential-bearing URL or local path.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct LockedIdentity {
    /// Actual package name, not dependency alias.
    pub name: String,
    /// Exact semver version.
    pub version: String,
    /// Source classification.
    pub source_kind: SourceKind,
}
/// A copied registry package and its exact resolved edges.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct LockPin {
    /// Registry package name.
    pub name: String,
    /// Exact version.
    pub version: String,
    /// Lowercase archive SHA-256, checked against the index at plan time.
    pub checksum: String,
    /// Resolved dependencies; not inferred feature/default activation.
    pub dependencies: Vec<LockedIdentity>,
    /// One-based original package line, when recorded during import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// One-based original package column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}
impl LockPin {
    pub(crate) fn identity(&self) -> LockedIdentity {
        LockedIdentity {
            name: self.name.clone(),
            version: self.version.clone(),
            source_kind: SourceKind::CratesIo,
        }
    }
}
/// Nonregistry graph placeholder; not an archive selection or Git/path import.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalPackage {
    /// Nonregistry package identity.
    pub identity: LockedIdentity,
    /// Copied resolved edges, including required registry packages.
    pub dependencies: Vec<LockedIdentity>,
}
/// Independent copied lock observation. The original input is not needed again.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LockSource {
    /// Original bounded basename.
    pub file_name: String,
    /// SHA-256 of exact original input bytes; provenance, not an authenticity signature.
    pub sha256: String,
    /// Import UTC Unix seconds.
    pub imported_at: u64,
    /// Detected standard Cargo.lock format, 1–4.
    pub lock_version: u32,
    /// Complete registry pin set, including development and inactive-target packages.
    pub pins: Vec<LockPin>,
    /// Explicit unmirrored graph placeholders.
    #[serde(default)]
    pub external_packages: Vec<ExternalPackage>,
}
/// Stable machine-readable parser/refusal categories.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// Malformed TOML or field type.
    Syntax,
    /// Unknown or nonstandard format version header.
    UnsupportedVersion,
    /// Invalid package name/version/source.
    InvalidPackage,
    /// Invalid SHA-256 encoding.
    InvalidChecksum,
    /// Registry entry has no usable recorded checksum.
    MissingChecksum,
    /// Exact package has contradictory recorded checksums.
    ConflictingChecksum,
    /// Duplicate or indistinguishable package identity.
    DuplicatePackage,
    /// A registry other than crates.io is not supported.
    AlternateRegistry,
    /// A shortened dependency matches more than one package.
    AmbiguousDependency,
    /// Resolved dependency does not exist in the lock.
    MissingDependency,
    /// Unsupported semantic field, never silently ignored.
    UnknownField,
    /// A resource limit prevents complete analysis.
    Budget,
}
/// Bounded source/field diagnostic. Messages never echo raw source URLs or input.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImportDiagnostic {
    /// Operator source name.
    pub source_name: String,
    /// Package name if known.
    pub package: Option<String>,
    /// Field name if known.
    pub field: Option<String>,
    /// One-based original line when available.
    pub line: Option<usize>,
    /// One-based original column when available.
    pub column: Option<usize>,
    /// Sanitized scheme/host origin of a refused registry, when safely parseable.
    pub registry_origin: Option<String>,
    /// Stable category.
    pub kind: DiagnosticKind,
    /// Sanitized explanation.
    pub message: String,
}
/// Why an entry is not a mirrored registry package.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipKind {
    /// Git sources are provisioned independently.
    Git,
    /// Path/workspace package, indistinguishable without project context.
    Local,
    /// Cargo recorded an unused patch rather than a resolved package.
    UnusedPatch,
}
/// Per-entry skip, preserving source location without copying a URL/path.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkippedEntry {
    /// Package name.
    pub name: String,
    /// Exact version.
    pub version: String,
    /// Explicit skip classification.
    pub kind: SkipKind,
    /// One-based source line.
    pub line: Option<usize>,
    /// One-based source column.
    pub column: Option<usize>,
}
/// Complete local parser analysis; rejected inputs have no proposed source.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParseReport {
    /// Whether every required registry entry/edge was accepted.
    pub accepted: bool,
    /// Copied source, with import time zero until a transaction stamps it.
    pub source: Option<LockSource>,
    /// Every nonregistry/unused entry within the input bound.
    pub skipped: Vec<SkippedEntry>,
    /// At most 1,024 sanitized diagnostics.
    pub diagnostics: Vec<ImportDiagnostic>,
    /// Total diagnostics before truncation.
    pub total_diagnostics: usize,
    /// Refusal report reached its diagnostic cap.
    pub truncated: bool,
    #[serde(skip)]
    line_starts: Vec<usize>,
}
impl ParseReport {
    fn new(input: &LockInput) -> Self {
        Self {
            accepted: false,
            source: None,
            skipped: vec![],
            diagnostics: vec![],
            total_diagnostics: 0,
            truncated: false,
            line_starts: if input.text.len() > INPUT_LIMIT {
                vec![0]
            } else {
                std::iter::once(0)
                    .chain(
                        input
                            .text
                            .bytes()
                            .enumerate()
                            .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
                    )
                    .collect()
            },
        }
    }
    fn location(&self, input: &LockInput, offset: usize) -> (usize, usize) {
        let mut end = offset.min(input.text.len());
        while !input.text.is_char_boundary(end) {
            end -= 1;
        }
        let line = self
            .line_starts
            .partition_point(|start| *start <= end)
            .saturating_sub(1);
        (
            line + 1,
            input.text[self.line_starts[line]..end].chars().count() + 1,
        )
    }
    fn issue(
        &mut self,
        input: &LockInput,
        package: Option<&str>,
        field: Option<&str>,
        offset: Option<usize>,
        kind: DiagnosticKind,
        message: &str,
    ) {
        self.total_diagnostics += 1;
        if self.diagnostics.len() == 1024 {
            self.truncated = true;
            return;
        }
        let (line, column) = offset
            .map(|n| self.location(input, n))
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or((None, None));
        self.diagnostics.push(ImportDiagnostic {
            source_name: safe(&input.source_name, 128),
            package: package.map(|x| safe(x, 128)),
            field: field.map(|x| safe(x, 128)),
            line,
            column,
            registry_origin: None,
            kind,
            message: message.into(),
        });
    }
}
fn safe(s: &str, limit: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(limit).collect()
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.chars().any(|c| c.is_control() || "/\\:".contains(c))
}
/// Validate a source name using the same portable rule as catalog parsing/import.
pub fn validate_source_name(name: &str) -> Result<()> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(invalid(
            "lock source name must be 1..128 portable ASCII letters/digits/_.-, excluding . and ..",
        ))
    }
}
/// Read an explicit regular UTF-8 lockfile, bounded to 16 MiB, without following symlinks.
/// No directory scan, network request or source path is persisted.
pub fn read_input(source_name: &str, path: &std::path::Path) -> Result<LockInput> {
    use std::io::Read;
    validate_source_name(source_name)?;
    events::stage("lock-read")?;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("lock input must be a regular non-symlink file"));
    }
    if metadata.len() > INPUT_LIMIT as u64 {
        return Err(super::budget("lock input exceeds 16 MiB"));
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| valid_file(n))
        .ok_or_else(|| invalid("lock input needs a safe UTF-8 basename"))?
        .to_owned();
    events::operation(&format!("lock-source:{source_name}"), &file_name);
    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        events::check()?;
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if bytes.len().saturating_add(n) > INPUT_LIMIT {
            return Err(super::budget("lock input grew beyond 16 MiB"));
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8(bytes).map_err(|_| invalid("lock input must be UTF-8"))?;
    Ok(LockInput {
        source_name: source_name.into(),
        file_name,
        text,
    })
}
/// Parse one bounded standard lock format locally, with cancellation/heartbeat support.
pub fn parse_with_control(input: &LockInput, control: &events::Control) -> Result<ParseReport> {
    control.run("lock-parse", || parse::parse(input))
}
/// Deterministic source replacement/removal classification.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportAction {
    /// New source name.
    Added,
    /// Existing name with changed pins/provenance.
    Replaced,
    /// Source explicitly removed.
    Removed,
    /// Source and provenance match exactly.
    Unchanged,
}
/// Exact per-source diff; no manual declaration is rewritten.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceImportReport {
    /// Operator source name.
    pub source_name: String,
    /// Change classification.
    pub action: ImportAction,
    /// Original basename.
    pub file_name: String,
    /// Original bytes' digest.
    pub sha256: String,
    /// UTC import timestamp.
    pub imported_at: u64,
    /// Lock format.
    pub lock_version: u32,
    /// Newly contributed exact pins.
    pub added: Vec<LockPin>,
    /// Pins no longer contributed by this source.
    pub removed: Vec<LockPin>,
    /// Identity/checksum pins still contributed (edge changes remain provenance changes).
    pub unchanged: Vec<LockPin>,
    /// Per-entry nonregistry/unused skips.
    pub skipped: Vec<SkippedEntry>,
    /// Source observation/graph metadata changed.
    pub provenance_changed: bool,
}
/// Atomic multi-source preview. Rejection never returns a partially changed catalog.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImportReport {
    /// Import-report schema, independent of event/inventory schemas.
    pub schema: u32,
    /// Whether the entire transaction was accepted.
    pub accepted: bool,
    /// Proposed self-contained catalog only on complete acceptance.
    pub catalog: Option<Catalog>,
    /// Deterministic per-source differences.
    pub sources: Vec<SourceImportReport>,
    /// Bounded typed diagnostics.
    pub diagnostics: Vec<ImportDiagnostic>,
    /// Total number before truncation.
    pub total_diagnostics: usize,
    /// Refusal list exceeded its bound.
    pub truncated: bool,
    /// Some packages require independent local/Git provisioning.
    pub external_sources_required: bool,
    /// Pure previews are false; the CLI sets true only after successful explicit write.
    pub written: bool,
}
fn diff(
    name: &str,
    old: Option<&LockSource>,
    new: Option<&LockSource>,
    skipped: Vec<SkippedEntry>,
) -> SourceImportReport {
    let current = new.or(old).unwrap();
    let old_pins: BTreeMap<_, _> = old
        .into_iter()
        .flat_map(|s| &s.pins)
        .map(|p| ((p.name.clone(), p.version.clone(), p.checksum.clone()), p))
        .collect();
    let new_pins: BTreeMap<_, _> = new
        .into_iter()
        .flat_map(|s| &s.pins)
        .map(|p| ((p.name.clone(), p.version.clone(), p.checksum.clone()), p))
        .collect();
    SourceImportReport {
        source_name: name.into(),
        action: match (old, new) {
            (None, _) => ImportAction::Added,
            (_, None) => ImportAction::Removed,
            (Some(a), Some(b)) if a == b => ImportAction::Unchanged,
            _ => ImportAction::Replaced,
        },
        file_name: current.file_name.clone(),
        sha256: current.sha256.clone(),
        imported_at: current.imported_at,
        lock_version: current.lock_version,
        added: new_pins
            .iter()
            .filter(|(k, _)| !old_pins.contains_key(*k))
            .map(|(_, p)| (*p).clone())
            .collect(),
        removed: old_pins
            .iter()
            .filter(|(k, _)| !new_pins.contains_key(*k))
            .map(|(_, p)| (*p).clone())
            .collect(),
        unchanged: new_pins
            .iter()
            .filter(|(k, _)| old_pins.contains_key(*k))
            .map(|(_, p)| (*p).clone())
            .collect(),
        skipped,
        provenance_changed: old != new,
    }
}
/// Preview all lock imports as one transaction. No network/filesystem mutation.
pub fn preview_import(
    catalog: &Catalog,
    inputs: &[LockInput],
    imported_at: u64,
    control: &events::Control,
) -> Result<ImportReport> {
    control.run("lock-import", || {
        catalog.validate()?;
        let mut result = ImportReport {
            schema: 1,
            accepted: false,
            catalog: None,
            sources: vec![],
            diagnostics: vec![],
            total_diagnostics: 0,
            truncated: false,
            external_sources_required: false,
            written: false,
        };
        if inputs.is_empty()
            || inputs.len() > 128
            || inputs.iter().map(|i| i.text.len()).sum::<usize>() > CATALOG_LIMIT
            || imported_at == 0
            || imported_at > i64::MAX as u64
        {
            return Err(super::budget(
                "lock import transaction exceeds supported bounds",
            ));
        }
        let mut proposed = catalog.clone();
        let mut seen = BTreeSet::new();
        for input in inputs {
            events::check()?;
            let mut parsed = parse::parse(input)?;
            if !seen.insert(&input.source_name) {
                parsed.issue(
                    input,
                    None,
                    None,
                    None,
                    DiagnosticKind::DuplicatePackage,
                    "source name occurs twice in this transaction",
                );
            }
            result.total_diagnostics += parsed.total_diagnostics;
            for d in parsed.diagnostics {
                if result.diagnostics.len() < 1024 {
                    result.diagnostics.push(d)
                } else {
                    result.truncated = true;
                }
            }
            result.truncated |= parsed.truncated;
            result.external_sources_required |= parsed
                .skipped
                .iter()
                .any(|s| s.kind != SkipKind::UnusedPatch);
            if let Some(mut source) = parsed.source {
                source.imported_at = imported_at;
                events::stage("lock-diff")?;
                events::operation(
                    &format!("lock-source:{}", input.source_name),
                    &input.file_name,
                );
                result.sources.push(diff(
                    &input.source_name,
                    proposed.crate_locks.get(&input.source_name),
                    Some(&source),
                    parsed.skipped,
                ));
                proposed
                    .crate_locks
                    .insert(input.source_name.clone(), source);
            }
        }
        if result.total_diagnostics == 0 {
            if let Err(e) = validate_sources(&proposed.crate_locks) {
                result.total_diagnostics = 1;
                let context = e.downcast_ref::<Error>().and_then(|e| e.report.as_ref());
                let source_name = context
                    .and_then(|r| r.get("source_name"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or("");
                let package = context
                    .and_then(|r| r.get("name"))
                    .and_then(toml::Value::as_str);
                let pin = proposed
                    .crate_locks
                    .get(source_name)
                    .and_then(|s| s.pins.iter().find(|p| Some(p.name.as_str()) == package));
                result.diagnostics.push(ImportDiagnostic {
                    source_name: source_name.into(),
                    package: package.map(str::to_owned),
                    field: Some("checksum".into()),
                    line: pin.and_then(|p| p.line),
                    column: pin.and_then(|p| p.column),
                    registry_origin: None,
                    kind: if crate::error_kind(e.as_ref()) == ErrorKind::Integrity {
                        DiagnosticKind::ConflictingChecksum
                    } else {
                        DiagnosticKind::Budget
                    },
                    message: "combined copied source pins conflict or exceed bounds".into(),
                });
            } else {
                proposed.validate()?;
                if proposed.to_toml()?.len() > CATALOG_LIMIT {
                    return Err(super::budget("copied catalog exceeds 64 MiB"));
                }
                result.accepted = true;
                result.external_sources_required |= proposed
                    .crate_locks
                    .values()
                    .any(|s| !s.external_packages.is_empty());
                result.catalog = Some(proposed);
            }
        }
        Ok(result)
    })
}
/// Preview removal of existing named sources only; manual declarations and other sources survive.
pub fn preview_remove(
    catalog: &Catalog,
    names: &[String],
    control: &events::Control,
) -> Result<ImportReport> {
    control.run("lock-diff", || {
        catalog.validate()?;
        if names.is_empty() || names.len() > 128 {
            return Err(invalid("bounded explicit source names required"));
        }
        let mut proposed = catalog.clone();
        let mut seen = BTreeSet::new();
        let mut sources = vec![];
        for name in names {
            events::check()?;
            if !seen.insert(name) {
                return Err(invalid("duplicate source removal"));
            }
            let old = proposed
                .crate_locks
                .remove(name)
                .ok_or_else(|| invalid("unknown lock source"))?;
            sources.push(diff(name, Some(&old), None, vec![]));
        }
        Ok(ImportReport {
            schema: 1,
            accepted: true,
            external_sources_required: proposed
                .crate_locks
                .values()
                .any(|s| !s.external_packages.is_empty()),
            catalog: Some(proposed),
            sources,
            diagnostics: vec![],
            total_diagnostics: 0,
            truncated: false,
            written: false,
        })
    })
}
pub(crate) fn validate_sources(sources: &BTreeMap<String, LockSource>) -> Result<()> {
    if sources.len() > 128 {
        return Err(super::budget("lock source count exceeds bound"));
    }
    let mut checksums = BTreeMap::new();
    for (name, s) in sources {
        events::check()?;
        if !valid_name(name)
            || !valid_file(&s.file_name)
            || !(1..=4).contains(&s.lock_version)
            || s.imported_at > i64::MAX as u64
        {
            return Err(invalid("invalid copied lock provenance"));
        }
        crate::valid_hash(&s.sha256)?;
        if s.pins.len() + s.external_packages.len() > 10000 {
            return Err(super::budget("copied lock package count exceeds bound"));
        }
        let mut identities = BTreeSet::new();
        let mut edges = 0usize;
        for p in &s.pins {
            index_path(&p.name)?;
            semver::Version::parse(&p.version)?;
            crate::valid_hash(&p.checksum)?;
            if !identities.insert(p.identity()) {
                return Err(invalid("duplicate copied lock package"));
            }
            if checksums
                .insert(
                    (p.name.clone(), p.version.clone()),
                    p.checksum.to_ascii_lowercase(),
                )
                .is_some_and(|c| !c.eq_ignore_ascii_case(&p.checksum))
            {
                let mut error = Error::new(
                    ErrorKind::Integrity,
                    "copied sources disagree on exact checksum",
                );
                error.report = Some(toml::Value::try_from(BTreeMap::from([
                    ("source_name", name.as_str()),
                    ("name", p.name.as_str()),
                    ("version", p.version.as_str()),
                ]))?);
                return Err(error.into());
            }
        }
        for p in &s.external_packages {
            if p.identity.source_kind == SourceKind::CratesIo || !valid_name(&p.identity.name) {
                return Err(invalid("invalid external lock placeholder"));
            }
            semver::Version::parse(&p.identity.version)?;
            if !identities.insert(p.identity.clone()) {
                return Err(invalid("duplicate external lock placeholder"));
            }
        }
        for deps in s
            .pins
            .iter()
            .map(|p| &p.dependencies)
            .chain(s.external_packages.iter().map(|p| &p.dependencies))
        {
            let mut seen = BTreeSet::new();
            for d in deps {
                events::check()?;
                edges += 1;
                if !identities.contains(d) || !seen.insert(d) {
                    return Err(invalid("missing or duplicate copied lock edge"));
                }
            }
        }
        if edges > 40000 || checksums.len() > 10000 {
            return Err(super::budget("copied lock pin/edge bound exceeded"));
        }
    }
    Ok(())
}
