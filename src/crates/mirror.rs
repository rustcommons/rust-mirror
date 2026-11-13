//! Frozen catalog previews and immutable crate-generation operations.
mod fetch;
mod retained;
mod sync;
use super::{
    catalog::Catalog,
    source::{archive_path, CrateSource},
    *,
};
use crate::{events, Error, ErrorKind, Result};
pub(crate) use fetch::inspect_run_metadata;
pub use fetch::{fetch, fetch_with_control, FetchOptions, FetchReport, Inventory};
pub use retained::{Cohort, RetentionObservation};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
pub use sync::{sync, sync_with_control, SyncOptions, SyncReport};

const PLAN_LIMIT: u64 = 64 * 1024 * 1024;
fn json_bytes<T: Serialize>(value: &T, pretty: bool) -> Result<Vec<u8>> {
    struct Bounded {
        bytes: Vec<u8>,
        exceeded: bool,
    }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().saturating_add(bytes.len()) as u64 > PLAN_LIMIT {
                self.exceeded = true;
                return Err(std::io::Error::other(
                    "frozen plan serialization bound exceeded",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Bounded {
        bytes: Vec::new(),
        exceeded: false,
    };
    let result = if pretty {
        serde_json::to_writer_pretty(&mut buffer, value)
    } else {
        serde_json::to_writer(&mut buffer, value)
    };
    if buffer.exceeded {
        return Err(budget("frozen plan exceeds serialized metadata bound"));
    }
    result?;
    Ok(buffer.bytes)
}
/// One exact selected root with its source alias and range/pin declaration.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedRoot {
    /// Catalog alias responsible for selection.
    pub alias: String,
    /// Original Cargo requirement (exact for an additional pin).
    pub requirement: String,
    /// Exact version and declared profile resolved independently.
    pub profile: Root,
}
/// Version identity used in change lists and size diagnostics.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// Case-preserved registry name.
    pub name: String,
    /// Exact semver version.
    pub version: String,
}
/// GUI row with original index bytes and complete native provenance.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRow {
    /// Case-preserved registry name.
    pub name: String,
    /// Exact version.
    pub version: String,
    /// Original archive checksum from crates.io.
    pub cksum: String,
    /// Optional HEAD estimate, never an integrity proof.
    pub expected_bytes: Option<u64>,
    /// Captured upstream yank state.
    pub yanked: bool,
    /// All known inclusion reasons are lock-only rather than build activation.
    pub lock_only: bool,
    /// Aliases selecting this version directly; empty means transitive only.
    pub root_aliases: Vec<String>,
    /// Short native inclusion reason for a preview row.
    pub primary_reason: Option<Reason>,
    /// All root/parent/requirement/feature reasons.
    pub reasons: Vec<Reason>,
    /// Original record belongs to the validated previous generation.
    pub retained_from_previous: bool,
    /// Original UTF-8 JSON line, including its original delimiter.
    pub index_line: String,
    /// Original line position within its upstream index file.
    pub source_ordinal: usize,
}
impl VersionRow {
    fn identity(&self) -> Identity {
        Identity {
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }
}
/// Summary against the explicitly chosen previous generation.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Changes {
    /// Versions added to this generation.
    pub added: Vec<Identity>,
    /// Existing versions preserved.
    pub retained: Vec<Identity>,
    /// Previously unyanked versions observed yanked but still retained.
    pub newly_yanked_retained: Vec<Identity>,
    /// Versions no longer selected by current rules but preserved for old clients.
    pub no_longer_selected_retained: Vec<Identity>,
}
/// Exact metadata observation used during native resolution; not an independent signature.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Requested canonical package name.
    pub name: String,
    /// SHA-256 of the complete observed sparse file before filtering.
    pub sha256: String,
    /// Original metadata bytes.
    pub bytes: u64,
}
/// Exact destination counts (sizes remain estimates until verification).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// Unique crate names.
    pub crates: usize,
    /// Exact mirrored version count.
    pub versions: usize,
    /// Served regular files, including the final marker.
    pub files: usize,
    /// Independently selected root/profile count.
    pub roots: usize,
}
/// Optional archive-length projection; streaming budgets remain authoritative.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SizeEstimate {
    /// Always true for a planning estimate.
    pub estimated: bool,
    /// Sum of known HEAD lengths.
    pub known_archive_bytes: u64,
    /// Number of archives without a known length.
    pub unknown_archives: usize,
    /// Whole archive total only when every length is known.
    pub expected_archive_bytes: Option<u64>,
    /// Exact generated index/config bytes; final marker size is checked at fetch completion.
    pub generated_index_bytes: u64,
}
/// Known oversized file (unknown lengths are reported separately).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OversizedFile {
    /// Exact generation-relative archive path.
    pub path: String,
    /// Observed length estimate.
    pub expected_bytes: u64,
}
/// Library-computed preview policy flags; absence of a known violation is not stream approval.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BudgetFlags {
    /// Known archive/index bytes already exceed the final budget.
    pub exceeds_final_budget: bool,
    /// Known new archive lengths exceed the transfer budget.
    pub exceeds_transfer_budget: bool,
    /// Known archives exceeding their individual cap.
    pub oversized_files: Vec<OversizedFile>,
}
/// Serializable approved-input boundary. Schema 1 is unrelated to inventory schema 4/protocol 1.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPlan {
    /// Frozen-plan schema (currently 1).
    pub schema: u32,
    /// SHA-256 of canonical typed JSON with this field empty.
    pub plan_id: String,
    /// Nonsecret selection/output/resource settings; network routing is excluded.
    pub catalog: Catalog,
    /// UTC Unix seconds at observation; never a freshness guarantee.
    pub observed_at: u64,
    /// Independently resolved exact root profiles.
    pub roots: Vec<SelectedRoot>,
    /// Stable preview rows plus original index lines.
    pub versions: Vec<VersionRow>,
    /// Native per-root coherence witnesses.
    pub witnesses: Vec<Witness>,
    /// Full input-file observation hashes.
    pub observations: Vec<Observation>,
    /// Exact counts.
    pub counts: Counts,
    /// Known versus unknown archive-length estimates.
    pub size_estimate: SizeEstimate,
    /// Library-computed known policy violations.
    pub budgets: BudgetFlags,
    /// Version preservation/change classification.
    pub changes: Changes,
    /// Previous marker hash, absent for a first generation.
    pub previous_inventory_sha256: Option<String>,
    /// Bounded coverage and size warnings for user review.
    pub warnings: Vec<String>,
    /// Flat, independently validated historical selection proofs; empty for a first generation.
    #[serde(default)]
    pub cohorts: Vec<Cohort>,
    /// Current observations and verified prior sizes for retained records.
    #[serde(default)]
    pub retention_observations: Vec<RetentionObservation>,
}
/// Explicit whole-preview approval. A caller must display the corresponding frozen preview first.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    /// Exact identity approved by the operator; never recomputed silently by fetch.
    pub plan_id: String,
}
struct Cache<'a> {
    source: &'a dyn CrateSource,
    files: BTreeMap<String, Vec<u8>>,
    bytes: u64,
    limit: u64,
}
impl IndexSource for Cache<'_> {
    fn index(&mut self, name: &str, limit: usize) -> Result<Vec<u8>> {
        if let Some(bytes) = self.files.get(name) {
            return Ok(bytes.clone());
        }
        events::check()?;
        let path = format!("cargo/index/{}", index_path(name)?);
        events::artifact(&path, "download", 0, None)?;
        let bytes = self.source.index(name, limit)?;
        events::complete(&path, "download", Some(bytes.len() as u64), false)?;
        if bytes.len() > limit {
            return Err(budget("index source exceeded its file bound"));
        }
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| budget("metadata byte overflow"))?;
        if self.bytes > self.limit {
            return Err(budget("catalog metadata budget exceeded"));
        }
        self.files.insert(name.into(), bytes.clone());
        Ok(bytes)
    }
}
fn base_url(catalog: &Catalog) -> Result<String> {
    let value = catalog.crate_output.base_url.trim_end_matches('/');
    let rest = value
        .strip_prefix("https://")
        .ok_or_else(|| invalid("crate output base-url must use HTTPS"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty()
        || !authority
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.[]:".contains(&b))
        || path.split('/').any(|p| p == "." || p == "..")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b))
    {
        return Err(invalid(
            "crate output URL must be credential-free with a safe absolute path",
        ));
    }
    Ok(value.into())
}
fn records(bytes: &[u8]) -> Result<Vec<(usize, Record, Vec<u8>)>> {
    bytes
        .split_inclusive(|b| *b == b'\n')
        .enumerate()
        .map(|(n, line)| {
            Ok((
                n,
                serde_json::from_slice(line).map_err(|_| invalid("invalid frozen index JSON"))?,
                line.to_vec(),
            ))
        })
        .collect()
}
fn selected_roots(catalog: &Catalog, cache: &mut dyn IndexSource) -> Result<Vec<SelectedRoot>> {
    let mut roots = Vec::new();
    for (alias, dependency) in &catalog.crates {
        events::check()?;
        let name = dependency.package.as_deref().unwrap_or(alias);
        let bytes = cache.index(name, 32 * 1024 * 1024)?;
        let rows = records(&bytes)?;
        let requirement = semver::VersionReq::parse(&dependency.version)
            .map_err(|_| invalid("invalid catalog requirement"))?;
        let mut candidates: Vec<_> = rows
            .iter()
            .filter_map(|(_, r, _)| semver::Version::parse(&r.vers).ok().map(|v| (v, r)))
            .filter(|(v, r)| !r.yanked && requirement.matches(v))
            .collect();
        candidates.sort_by(|(a, _), (b, _)| b.cmp(a));
        let rule = catalog.crate_rules.get(alias).cloned().unwrap_or_default();
        let mut selections: BTreeMap<String, String> = candidates
            .iter()
            .take(rule.latest)
            .map(|(_, r)| (r.vers.clone(), dependency.version.clone()))
            .collect();
        for version in &rule.pins {
            selections.insert(version.clone(), format!("={version}"));
        }
        if selections.is_empty() {
            return Err(selection_error(
                alias,
                name,
                &dependency.version,
                "no_matching_version",
                &rows,
                None,
            ));
        }
        for (version, requirement) in selections {
            let record = rows
                .iter()
                .find(|(_, r, _)| r.vers == version)
                .map(|(_, r, _)| r)
                .ok_or_else(|| {
                    selection_error(
                        alias,
                        name,
                        &requirement,
                        "no_matching_version",
                        &rows,
                        None,
                    )
                })?;
            if record.name != name {
                return Err(invalid(
                    "catalog package case differs from its upstream registry identity",
                ));
            }
            if record.yanked {
                return Err(selection_error(
                    alias,
                    name,
                    &requirement,
                    "yanked",
                    &rows,
                    None,
                ));
            }
            if let Some(feature) = dependency.features.iter().find(|f| !has_feature(record, f)) {
                return Err(selection_error(
                    alias,
                    name,
                    &requirement,
                    "missing_feature",
                    &rows,
                    Some(feature),
                ));
            }
            roots.push(SelectedRoot {
                alias: alias.clone(),
                requirement,
                profile: Root {
                    name: name.into(),
                    version,
                    features: dependency.features.clone(),
                    default_features: dependency.default_features,
                },
            });
            if roots.len() > 4096 {
                return Err(budget("selected root/profile limit exceeded"));
            }
        }
    }
    Ok(roots)
}
fn selection_error(
    alias: &str,
    name: &str,
    requirement: &str,
    kind: &str,
    rows: &[(usize, Record, Vec<u8>)],
    feature: Option<&str>,
) -> Box<dyn std::error::Error + Send + Sync> {
    let mut error = Error::new(
        ErrorKind::Invalid,
        "crate selection refused; inspect structured rule context",
    );
    let available: Vec<_> = rows
        .iter()
        .rev()
        .take(10)
        .map(|(_, r, _)| r.vers.clone())
        .collect();
    let mut report = toml::map::Map::new();
    for (key, value) in [
        ("alias", alias),
        ("package", name),
        ("requirement", requirement),
        ("kind", kind),
    ] {
        report.insert(key.into(), toml::Value::String(value.into()));
    }
    if let Some(feature) = feature {
        report.insert("feature".into(), toml::Value::String(feature.into()));
    }
    report.insert(
        "available_versions".into(),
        toml::Value::try_from(available).unwrap(),
    );
    error.report = Some(toml::Value::Table(report));
    error.into()
}
fn native_bounds(catalog: &Catalog) -> Bounds {
    Bounds {
        crates: catalog.crate_policy.max_versions.min(10000),
        versions: catalog.crate_policy.max_versions,
        index_bytes: 32 * 1024 * 1024,
        total_metadata_bytes: catalog.crate_policy.max_metadata_bytes as usize,
        work: 20_000_000,
        attempts: 1_000_000,
    }
}
impl FrozenPlan {
    fn computed_id(&self) -> Result<String> {
        let mut value = self.clone();
        value.plan_id.clear();
        Ok(crate::sha256(&json_bytes(&value, false)?))
    }
    /// Bounded local loading with complete typed validation. Never accesses the network or environment.
    pub fn load(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > PLAN_LIMIT {
            return Err(invalid("frozen plan must be a bounded regular file"));
        }
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(PLAN_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > PLAN_LIMIT {
            return Err(budget("frozen plan input grew beyond bound"));
        }
        let plan: Self =
            serde_json::from_slice(&bytes).map_err(|_| invalid("invalid frozen crate plan"))?;
        plan.validate()?;
        Ok(plan)
    }
    /// Save a new frozen JSON file without overwriting an existing path.
    pub fn save_new(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&json_bytes(self, true)?)?;
        file.as_file().sync_all()?;
        file.persist_noclobber(path).map_err(|e| e.error)?;
        Ok(())
    }
    /// Reconstruct IDs, names, records, counts and native coherence before trusting persisted input.
    pub fn validate(&self) -> Result<()> {
        self.validate_inner(true)
    }
    pub(crate) fn inspect_metadata(&self) -> Result<()> {
        self.validate_inner(false)
    }
    fn validate_inner(&self, replay: bool) -> Result<()> {
        self.catalog.validate()?;
        if !self.cohorts.is_empty() {
            return retained::validate(self, replay);
        }
        if !self.retention_observations.is_empty() {
            return Err(invalid("retention observations require historical proofs"));
        }
        if self.catalog.crate_policy.hosts.is_empty()
            || self.catalog.crate_policy.targets.is_empty()
        {
            return Err(invalid(
                "frozen coverage requires explicit hosts and targets",
            ));
        }
        if self.schema != 1
            || self.catalog.network.is_some()
            || self.observed_at == 0
            || self.roots.is_empty()
            || self.roots.len() > 4096
            || self.versions.len() > self.catalog.crate_policy.max_versions
            || self.observations.len() > 10000
            || self.warnings.len() > 128
        {
            return Err(invalid("unsupported or oversized frozen plan"));
        }
        base_url(&self.catalog)?;
        crate::valid_hash(&self.plan_id)?;
        let mut observations = BTreeSet::new();
        let mut metadata_bytes = 0u64;
        for observation in &self.observations {
            index_path(&observation.name)?;
            crate::valid_hash(&observation.sha256)?;
            if !observations.insert(&observation.name)
                || observation.bytes == 0
                || observation.bytes > 32 * 1024 * 1024
            {
                return Err(invalid("invalid frozen metadata observation"));
            }
            metadata_bytes = metadata_bytes
                .checked_add(observation.bytes)
                .ok_or_else(|| budget("metadata count overflow"))?;
        }
        if metadata_bytes > self.catalog.crate_policy.max_metadata_bytes {
            return Err(budget("frozen metadata exceeds catalog limit"));
        }
        let mut package_names = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut ordinals = BTreeSet::new();
        for row in &self.versions {
            events::check()?;
            let normalized = row.name.to_ascii_lowercase().replace('_', "-");
            if package_names
                .insert(normalized, row.name.clone())
                .is_some_and(|old| old != row.name)
            {
                return Err(invalid(
                    "registry package names collide after normalization",
                ));
            }
            index_path(&row.name)?;
            archive_path(&row.name, &row.version)?;
            crate::valid_hash(&row.cksum)?;
            if !names.insert((row.name.clone(), row.version.clone()))
                || !ordinals.insert((row.name.clone(), row.source_ordinal))
                || row.reasons.len() > 100000
                || row.index_line.len() > 32 * 1024 * 1024
            {
                return Err(invalid("duplicate/oversized frozen version"));
            }
            if !row.index_line.ends_with('\n') {
                return Err(invalid("index records require original newline delimiters"));
            }
            let parsed = records(row.index_line.as_bytes())?;
            if parsed.len() != 1 {
                return Err(invalid("one original index record required per version"));
            }
            let r = &parsed[0].1;
            if r.name != row.name
                || r.vers != row.version
                || r.cksum != row.cksum
                || r.yanked != row.yanked
                || r.v > 2
                || r.v == 0
            {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "frozen index record disagrees with preview",
                )
                .into());
            }
            if row.retained_from_previous && self.previous_inventory_sha256.is_none() {
                return Err(invalid(
                    "retained record requires a previous inventory identity",
                ));
            }
            if !row.retained_from_previous && row.yanked {
                return Err(invalid("new yanked versions cannot be selected"));
            }
            if row.reasons.iter().any(|r| r.root >= self.roots.len()) {
                return Err(invalid("reason references an unknown root"));
            }
        }
        for root in &self.roots {
            let dependency = self
                .catalog
                .crates
                .get(&root.alias)
                .ok_or_else(|| invalid("unknown root alias"))?;
            let expected_name = dependency.package.as_deref().unwrap_or(&root.alias);
            if root.profile.name != expected_name
                || root.profile.features != dependency.features
                || root.profile.default_features != dependency.default_features
                || !names.contains(&(root.profile.name.clone(), root.profile.version.clone()))
            {
                return Err(invalid("root profile differs from catalog"));
            }
            let requirement = semver::VersionReq::parse(&dependency.version)
                .map_err(|_| invalid("invalid root requirement"))?;
            let version = semver::Version::parse(&root.profile.version)
                .map_err(|_| invalid("invalid exact root version"))?;
            if !requirement.matches(&version)
                && !self
                    .catalog
                    .crate_rules
                    .get(&root.alias)
                    .is_some_and(|r| r.pins.contains(&root.profile.version))
            {
                return Err(invalid("root is neither matching nor explicitly pinned"));
            }
        }
        let (counts, sizes, budgets) = summaries(self)?;
        if counts != self.counts || sizes != self.size_estimate || budgets != self.budgets {
            return Err(
                Error::new(ErrorKind::Integrity, "frozen preview counters were altered").into(),
            );
        }
        if self.computed_id()? != self.plan_id {
            return Err(Error::new(ErrorKind::Integrity, "frozen plan identity mismatch").into());
        }
        if self.previous_inventory_sha256.is_some() {
            return Err(invalid(
                "retained-generation planning integration is not yet available",
            ));
        }
        if !replay {
            return Ok(());
        }
        let mut index = FilteredIndex::new(&self.versions)?;
        let expected_roots = selected_roots(&self.catalog, &mut index)?;
        if serde_json::to_value(&expected_roots)? != serde_json::to_value(&self.roots)? {
            return Err(invalid("frozen root set differs from catalog selection"));
        }
        let profiles: Vec<_> = self.roots.iter().map(|r| r.profile.clone()).collect();
        let rebuilt = super::plan(&mut index, &profiles, native_bounds(&self.catalog))?;
        if rebuilt.versions.len() != self.versions.len()
            || serde_json::to_value(&rebuilt.witnesses)? != serde_json::to_value(&self.witnesses)?
        {
            return Err(invalid(
                "frozen witness/version set differs from native reconstruction",
            ));
        }
        for row in &self.versions {
            let native = rebuilt
                .versions
                .iter()
                .find(|v| v.name == row.name && v.version == row.version)
                .ok_or_else(|| invalid("frozen version is not explained by the catalog"))?;
            let aliases: Vec<_> = self
                .roots
                .iter()
                .filter(|r| r.profile.name == row.name && r.profile.version == row.version)
                .map(|r| r.alias.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if native.index_line != row.index_line.as_bytes()
                || native.reasons != row.reasons
                || row.primary_reason != row.reasons.first().cloned()
                || row.lock_only != row.reasons.iter().all(|r| r.lock_only)
                || row.root_aliases != aliases
            {
                return Err(invalid(
                    "frozen provenance differs from native reconstruction",
                ));
            }
        }
        let added: Vec<_> = self.versions.iter().map(VersionRow::identity).collect();
        if self.changes.added != added
            || !self.changes.retained.is_empty()
            || !self.changes.newly_yanked_retained.is_empty()
            || !self.changes.no_longer_selected_retained.is_empty()
        {
            return Err(invalid("frozen changes do not match generation contents"));
        }

        Ok(())
    }
}
struct FilteredIndex {
    files: BTreeMap<String, Vec<u8>>,
}
impl FilteredIndex {
    fn new(rows: &[VersionRow]) -> Result<Self> {
        let mut map: BTreeMap<String, Vec<&VersionRow>> = BTreeMap::new();
        for row in rows {
            map.entry(row.name.clone()).or_default().push(row);
        }
        let mut files = BTreeMap::new();
        for (name, mut rows) in map {
            rows.sort_by_key(|r| r.source_ordinal);
            let mut bytes = Vec::new();
            for row in rows {
                bytes.extend_from_slice(row.index_line.as_bytes());
            }
            files.insert(name, bytes);
        }
        Ok(Self { files })
    }
}
impl IndexSource for FilteredIndex {
    fn index(&mut self, name: &str, limit: usize) -> Result<Vec<u8>> {
        let data = self
            .files
            .get(name)
            .ok_or_else(|| invalid("filtered closure requires an unmirrored crate"))?;
        if data.len() > limit {
            return Err(budget("filtered index file exceeds bound"));
        }
        Ok(data.clone())
    }
}
fn generated_files(plan: &FrozenPlan) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let filtered = FilteredIndex::new(&plan.versions)?;
    for (name, bytes) in filtered.files {
        if files
            .insert(format!("cargo/index/{}", index_path(&name)?), bytes)
            .is_some()
        {
            return Err(invalid("sparse index output path collision"));
        }
    }
    let config = serde_json::json!({"dl":format!("{}/cargo/crates/{{crate}}/{{crate}}-{{version}}.crate",base_url(&plan.catalog)?)});
    files.insert(
        "cargo/index/config.json".into(),
        serde_json::to_vec(&config)?,
    );
    Ok(files)
}
fn summaries(plan: &FrozenPlan) -> Result<(Counts, SizeEstimate, BudgetFlags)> {
    let generated = generated_files(plan)?;
    let mut sizes = SizeEstimate {
        estimated: true,
        ..Default::default()
    };
    let mut budgets = BudgetFlags::default();
    let mut transfer = 0u64;
    for row in &plan.versions {
        if let Some(bytes) = row.expected_bytes {
            sizes.known_archive_bytes = sizes
                .known_archive_bytes
                .checked_add(bytes)
                .ok_or_else(|| budget("archive size overflow"))?;
            if !row.retained_from_previous {
                transfer = transfer
                    .checked_add(bytes)
                    .ok_or_else(|| budget("transfer size overflow"))?;
            }
            if bytes > plan.catalog.crate_policy.max_file_bytes {
                budgets.oversized_files.push(OversizedFile {
                    path: archive_path(&row.name, &row.version)?,
                    expected_bytes: bytes,
                });
            }
        } else {
            sizes.unknown_archives += 1;
        }
    }
    if sizes.unknown_archives == 0 {
        sizes.expected_archive_bytes = Some(sizes.known_archive_bytes);
    }
    sizes.generated_index_bytes = generated.values().map(|b| b.len() as u64).sum();
    for (path, bytes) in &generated {
        if bytes.len() as u64 > plan.catalog.crate_policy.max_file_bytes {
            budgets.oversized_files.push(OversizedFile {
                path: path.clone(),
                expected_bytes: bytes.len() as u64,
            });
        }
    }

    budgets.exceeds_final_budget = sizes
        .known_archive_bytes
        .checked_add(sizes.generated_index_bytes)
        .ok_or_else(|| budget("final estimate overflow"))?
        > plan.catalog.crate_policy.max_final_bytes;
    budgets.exceeds_transfer_budget = transfer > plan.catalog.crate_policy.max_transfer_bytes;
    let counts = Counts {
        crates: plan
            .versions
            .iter()
            .map(|r| r.name.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        versions: plan.versions.len(),
        files: plan.versions.len() + generated.len() + 1,
        roots: plan.roots.len(),
    };
    if counts.files > plan.catalog.crate_policy.max_files {
        return Err(budget("final file count exceeds budget"));
    }
    Ok((counts, sizes, budgets))
}
/// Resolve a catalog natively and return a frozen preview, with optional HEAD estimates and no archive calls.
/// Previous-generation support is integrated with the schema-4 reader; unavailable states refuse rather than drop retention.
pub fn plan(
    catalog: &Catalog,
    source: &dyn CrateSource,
    previous: Option<&Path>,
    estimate_sizes: bool,
) -> Result<FrozenPlan> {
    catalog.validate()?;
    base_url(catalog)?;
    if catalog.crate_policy.hosts.is_empty() || catalog.crate_policy.targets.is_empty() {
        return Err(invalid(
            "crate planning requires explicit host and target declarations",
        ));
    }
    if let Some(previous) = previous {
        return retained::plan(catalog, source, previous, estimate_sizes);
    }
    events::stage("catalog")?;
    let mut cache = Cache {
        source,
        files: BTreeMap::new(),
        bytes: 0,
        limit: catalog.crate_policy.max_metadata_bytes,
    };
    let roots = selected_roots(catalog, &mut cache)?;
    let profiles: Vec<_> = roots.iter().map(|r| r.profile.clone()).collect();
    let native = super::plan(&mut cache, &profiles, native_bounds(catalog)).map_err(|error| {
        if let Some(typed) = error.downcast_ref::<Error>() {
            let mut typed = typed.clone();
            if let Some(table) = typed.report.as_mut().and_then(toml::Value::as_table_mut) {
                if let Some(root) = table
                    .get("root_index")
                    .and_then(toml::Value::as_integer)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| roots.get(i))
                {
                    table.insert("alias".into(), toml::Value::String(root.alias.clone()));
                }
            }
            return Box::new(typed) as Box<dyn std::error::Error + Send + Sync>;
        }
        error
    })?;
    let mut versions = Vec::new();
    for v in native.versions {
        let source_ordinal = records(&cache.files[&v.name])?
            .into_iter()
            .find(|(_, r, _)| r.vers == v.version)
            .ok_or_else(|| invalid("native record missing from frozen input"))?
            .0;
        let record: Record = serde_json::from_slice(&v.index_line)?;
        let root_aliases = roots
            .iter()
            .filter(|r| r.profile.name == v.name && r.profile.version == v.version)
            .map(|r| r.alias.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let expected_bytes = if estimate_sizes {
            events::stage("measure")?;
            let path = archive_path(&v.name, &v.version)?;
            events::artifact(&path, "head", 0, None)?;
            let length = source.head(&v.name, &v.version)?;
            events::complete(&path, "head", length, false)?;
            length
        } else {
            None
        };
        versions.push(VersionRow {
            name: v.name,
            version: v.version,
            cksum: v.checksum,
            expected_bytes,
            yanked: record.yanked,
            lock_only: v.reasons.iter().all(|r| r.lock_only),
            root_aliases,
            primary_reason: v.reasons.first().cloned(),
            reasons: v.reasons,
            retained_from_previous: false,
            index_line: String::from_utf8(v.index_line)?,
            source_ordinal,
        });
    }
    let observations = cache
        .files
        .iter()
        .map(|(name, data)| Observation {
            name: name.clone(),
            sha256: crate::sha256(data),
            bytes: data.len() as u64,
        })
        .collect();
    let mut semantic = catalog.clone();
    semantic.network = None;
    let mut value = FrozenPlan {
        schema: 1,
        plan_id: String::new(),
        catalog: semantic,
        observed_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        roots,
        versions,
        witnesses: native.witnesses,
        observations,
        counts: Counts::default(),
        size_estimate: SizeEstimate::default(),
        budgets: BudgetFlags::default(),
        changes: Changes::default(),
        previous_inventory_sha256: None,
        cohorts: Vec::new(),
        retention_observations: Vec::new(),
        warnings: vec![
            "Declared features with all-platform normal/build lock coverage; build success is not implied.".into(),
            "HEAD sizes are estimates; actual stream/final-generation budgets remain authoritative.".into(),
        ],
    };
    value.changes.added = value.versions.iter().map(VersionRow::identity).collect();
    let (counts, sizes, budgets) = summaries(&value)?;
    value.counts = counts;
    value.size_estimate = sizes;
    value.budgets = budgets;
    value.plan_id = value.computed_id()?;
    value.validate()?;
    Ok(value)
}
/// Controlled planning with independent liveness and cooperative native/index cancellation checks.
pub fn plan_with_control(
    catalog: &Catalog,
    source: &dyn CrateSource,
    previous: Option<&Path>,
    estimate_sizes: bool,
    control: &events::Control,
) -> Result<FrozenPlan> {
    control.run("crates-plan", || {
        plan(catalog, source, previous, estimate_sizes)
    })
}

/// One version step in a root-to-dependency explanation, using the preview's exact reason object.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplainStep {
    /// Exact version at this step.
    pub identity: Identity,
    /// Original native reason; root indexes refer to the same frozen preview.
    pub reason: Reason,
}
/// One simple inclusion path with no repeated version identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplainChain {
    /// Catalog root alias.
    pub root_alias: String,
    /// Root-first dependency steps.
    pub steps: Vec<ExplainStep>,
}
/// Bounded local explanation. A null total means the traversal budget prevented an exact count.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExplainReport {
    /// At most the requested number of chains.
    pub chains: Vec<ExplainChain>,
    /// Exact simple-chain count when traversal finishes; unknown rather than a fabricated count on exhaustion.
    pub total_chains: Option<u64>,
    /// More chains exist or exact counting reached its safety bound.
    pub truncated: bool,
}
/// Explain locally without network. Counts all simple paths within a fixed work/depth bound; returns at most 256.
pub fn explain(
    plan: &FrozenPlan,
    name: &str,
    version: Option<&str>,
    limit: usize,
) -> Result<ExplainReport> {
    if limit == 0
        || limit > 256
        || plan.versions.len() > 10000
        || plan.roots.len() > 4096
        || plan.computed_id()? != plan.plan_id
    {
        return Err(invalid("invalid explanation input or limit"));
    }
    let rows: BTreeMap<_, _> = plan
        .versions
        .iter()
        .map(|r| ((r.name.as_str(), r.version.as_str()), r))
        .collect();
    struct Frame<'a> {
        row: &'a VersionRow,
        next: usize,
        root: usize,
        seen: BTreeSet<String>,
    }
    let mut chains = Vec::new();
    let mut total = 0u64;
    let mut work = 0usize;
    let mut exhausted = false;
    'targets: for target in plan
        .versions
        .iter()
        .filter(|r| r.name == name && version.is_none_or(|v| v == r.version))
    {
        let mut target_seen = BTreeSet::new();
        for target_reason in &target.reasons {
            let mut key = target_reason.clone();
            key.conservative = false;
            if !target_seen.insert(serde_json::to_string(&key)?) {
                continue;
            }
            events::check()?;
            let root = target_reason.root;
            let alias = &plan
                .roots
                .get(root)
                .ok_or_else(|| invalid("unknown explanation root"))?
                .alias;
            let first = ExplainStep {
                identity: target.identity(),
                reason: target_reason.clone(),
            };
            let mut path = vec![first];
            let mut stack = Vec::new();
            if let Some(parent) = &target_reason.parent {
                let (name, version) = parent
                    .split_once('@')
                    .ok_or_else(|| invalid("invalid explanation parent"))?;
                let row = rows
                    .get(&(name, version))
                    .ok_or_else(|| invalid("missing explanation parent"))?;
                stack.push(Frame {
                    row,
                    next: 0,
                    root,
                    seen: BTreeSet::new(),
                });
            } else {
                total = total
                    .checked_add(1)
                    .ok_or_else(|| budget("explanation count overflow"))?;
                if chains.len() < limit {
                    chains.push(ExplainChain {
                        root_alias: alias.clone(),
                        steps: path.clone(),
                    });
                }
            }
            while !stack.is_empty() {
                events::check()?;
                work += 1;
                if work > 100000 || stack.len() > 256 {
                    exhausted = true;
                    break 'targets;
                }
                let frame = stack.last_mut().unwrap();
                if frame.next >= frame.row.reasons.len() {
                    stack.pop();
                    if path.len() > 1 {
                        path.pop();
                    }
                    continue;
                }
                let reason = &frame.row.reasons[frame.next];
                frame.next += 1;
                if reason.root != frame.root {
                    continue;
                }
                let mut key = reason.clone();
                key.conservative = false;
                if !frame.seen.insert(serde_json::to_string(&key)?) {
                    continue;
                }
                let identity = frame.row.identity();
                if path.iter().any(|s| s.identity == identity) {
                    continue;
                }
                path.push(ExplainStep {
                    identity,
                    reason: reason.clone(),
                });
                if let Some(parent) = &reason.parent {
                    let (name, version) = parent
                        .split_once('@')
                        .ok_or_else(|| invalid("invalid explanation parent"))?;
                    let row = rows
                        .get(&(name, version))
                        .ok_or_else(|| invalid("missing explanation parent"))?;
                    stack.push(Frame {
                        row,
                        next: 0,
                        root,
                        seen: BTreeSet::new(),
                    });
                } else {
                    total = total
                        .checked_add(1)
                        .ok_or_else(|| budget("explanation count overflow"))?;
                    if chains.len() < limit {
                        let mut steps = path.clone();
                        steps.reverse();
                        chains.push(ExplainChain {
                            root_alias: alias.clone(),
                            steps,
                        });
                    }
                    path.pop();
                }
            }
        }
    }
    Ok(ExplainReport {
        truncated: exhausted || total > chains.len() as u64,
        total_chains: if exhausted { None } else { Some(total) },
        chains,
    })
}
/// Exact project-local Cargo source replacement text; never edits a Cargo home or project file.
pub fn client_config(plan: &FrozenPlan) -> Result<String> {
    let base = base_url(&plan.catalog)?;
    Ok(format!("[source.crates-io]\nreplace-with = \"org-mirror\"\n\n[source.org-mirror]\nregistry = \"sparse+{base}/cargo/index/\"\n"))
}

/// Read bounded generation metadata and return its Cargo source-replacement snippet.
/// This performs no archive reads or network requests and does not verify payloads.
/// Publication and pruning must still use the full inventory verification paths.
pub fn client_config_for_generation(root: &Path) -> Result<String> {
    client_config(&Inventory::inspect(root)?.plan)
}
