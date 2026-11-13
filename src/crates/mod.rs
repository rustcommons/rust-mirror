//! Native crate dependency closure, curated catalogs and experimental immutable registry workflows.
//!
//! No Cargo executable is invoked at runtime. The schema-0 report remains a research
//! format; approved frozen MVP operations live in [`mirror`].
pub mod catalog;
pub mod lockfile;
pub mod mirror;
pub mod retention;
pub mod source;
use crate::{events, network, Error, ErrorKind, NetworkOptions, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Read;

fn invalid(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::Invalid, message).into()
}
fn budget(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::Budget, message).into()
}
/// Metadata transport with no archive operation.
pub trait IndexSource {
    /// Return the original bounded newline-delimited index bytes for a canonical name.
    fn index(&mut self, name: &str, limit: usize) -> Result<Vec<u8>>;
}
/// Official sparse metadata transport. Never requests an archive or a redirect.
pub struct SparseIndex {
    agent: ureq::Agent,
}
impl SparseIndex {
    /// Construct with explicit proxy/CA options and no implicit environment discovery.
    pub fn new(network: NetworkOptions) -> Result<Self> {
        Ok(Self {
            agent: network::agent(&network, "index.crates.io", 443)?,
        })
    }
}
impl IndexSource for SparseIndex {
    fn index(&mut self, name: &str, limit: usize) -> Result<Vec<u8>> {
        let path = index_path(name)?;
        let response = self
            .agent
            .get(&format!("https://index.crates.io/{path}"))
            .set(
                "User-Agent",
                concat!(
                    "rust-mirror/",
                    env!("CARGO_PKG_VERSION"),
                    "-phase0 (+https://github.com/rustcommons/rust-mirror)"
                ),
            )
            .set("Accept-Encoding", "identity")
            .call()
            .map_err(network::transport_error)?;
        if response.status() != 200 {
            return Err(invalid("sparse index did not return HTTP 200"));
        }
        let mut reader = response.into_reader();
        let mut bytes = Vec::new();
        let mut buffer = [0; 16384];
        loop {
            events::check()?;
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if bytes.len().saturating_add(count) > limit {
                return Err(budget("index file limit exceeded"));
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        Ok(bytes)
    }
}
/// Validate a portable registry name and return its lowercase Cargo sparse shard path.
pub fn index_path(name: &str) -> Result<String> {
    let lower = name.to_ascii_lowercase();
    let reserved = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        || reserved.contains(&lower.as_str())
    {
        return Err(invalid(
            "crate name must be a valid portable crates.io identifier",
        ));
    }
    Ok(match lower.len() {
        1 => format!("1/{lower}"),
        2 => format!("2/{lower}"),
        3 => format!("3/{}/{lower}", &lower[..1]),
        _ => format!("{}/{}/{lower}", &lower[..2], &lower[2..4]),
    })
}
/// One exact registry root and declared feature profile. Catalog range syntax is not implemented here.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    /// Canonical crates.io package name.
    pub name: String,
    /// Exact version selected before resolution.
    pub version: String,
    /// Explicitly enabled root features.
    #[serde(default)]
    pub features: Vec<String>,
    /// Enable the default feature if defined.
    #[serde(default = "yes")]
    pub default_features: bool,
}
fn yes() -> bool {
    true
}
/// Hard resource bounds for the research planner.
#[derive(Clone, Debug)]
pub struct Bounds {
    /// Maximum distinct index files requested.
    pub crates: usize,
    /// Maximum bytes per index file.
    pub index_bytes: usize,
    /// Maximum aggregate metadata bytes.
    pub total_metadata_bytes: usize,
    /// Maximum distinct included versions per plan.
    pub versions: usize,
    /// Maximum feature/edge evaluation operations per plan.
    pub work: usize,
    /// Maximum backtracking attempts per plan.
    pub attempts: usize,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            crates: 1000,
            index_bytes: 32 * 1024 * 1024,
            total_metadata_bytes: 256 * 1024 * 1024,
            versions: 3000,
            work: 2_000_000,
            attempts: 100_000,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
struct Dependency {
    name: String,
    #[serde(default)]
    package: Option<String>,
    req: String,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default = "yes")]
    default_features: bool,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    registry: Option<String>,
    #[serde(default)]
    target: Option<String>,
}
impl Dependency {
    fn package(&self) -> &str {
        self.package.as_deref().unwrap_or(&self.name)
    }
}
#[derive(Clone, Debug, Deserialize)]
struct Record {
    name: String,
    vers: String,
    deps: Vec<Dependency>,
    cksum: String,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    features2: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    yanked: bool,
    #[serde(default)]
    links: Option<String>,
    #[serde(default = "one")]
    v: u32,
}
fn one() -> u32 {
    1
}
#[derive(Clone)]
struct Entry {
    record: Record,
    version: Version,
    raw: Vec<u8>,
}
type Identity = (String, String);
/// One explained inclusion edge, including conservative alternatives outside the witness.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reason {
    /// Root profile index in the input.
    pub root: usize,
    /// Requiring package, or null for the exact root.
    pub parent: Option<String>,
    /// Dependency alias (which can differ from its package name).
    pub alias: String,
    /// Original semver requirement or exact-root requirement.
    pub requirement: String,
    /// Original normal/build target predicate; null means unconditional.
    pub target: Option<String>,
    /// True when the inclusion is from conservative newest-per-edge coverage.
    pub conservative: bool,
    /// Included for Cargo lock compatibility (weak forwarding), not build-time activation.
    pub lock_only: bool,
    /// Feature requests propagated along this inclusion edge.
    pub features: Vec<String>,
    /// Whether this edge requests the dependency default feature.
    pub default_features: bool,
}
/// One immutable metadata record selected for the proposed filtered index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannedVersion {
    /// Canonical name.
    pub name: String,
    /// Exact version.
    pub version: String,
    /// Index archive checksum; no archive has been fetched or verified.
    pub checksum: String,
    /// Original JSON record bytes including its original line delimiter.
    pub index_line: Vec<u8>,
    /// Inclusion chains, available without Cargo diagnostics.
    pub reasons: Vec<Reason>,
}
/// Native existence witness for one root/profile, not an arbitrary-project build guarantee.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Witness {
    /// Exact root/profile.
    pub root: Root,
    /// Version identities in the coherent witness.
    pub versions: Vec<String>,
}
/// Serializable metadata-only Phase 0 report. Not a production frozen fetch plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    /// Research report schema.
    pub schema: u32,
    /// Selected records, exact original lines, and reasons.
    pub versions: Vec<PlannedVersion>,
    /// Separate witnesses for independent roots.
    pub witnesses: Vec<Witness>,
    /// Aggregate upstream metadata bytes observed.
    pub metadata_bytes: usize,
    /// Number of distinct index files fetched.
    pub index_requests: usize,
    /// Archive requests made by this metadata-only API (always zero).
    pub archive_requests: usize,
    /// Native search branches explored.
    pub attempts: usize,
}
struct Planner<'a> {
    source: &'a mut dyn IndexSource,
    bounds: Bounds,
    cache: BTreeMap<String, Vec<Entry>>,
    metadata_bytes: usize,
    work: usize,
    attempts: usize,
    frozen: bool,
    last_report: Option<toml::Value>,
}
impl Planner<'_> {
    fn context(&self, request: &Request, kind: &str, link: Option<&str>) -> Result<toml::Value> {
        #[derive(Serialize)]
        struct Context<'a> {
            kind: &'a str,
            package: &'a str,
            dependency_alias: &'a str,
            requirement: &'a str,
            features: &'a BTreeSet<String>,
            parent: Option<&'a str>,
            link: Option<&'a str>,
            available_versions: Vec<&'a str>,
            context_is_sample: bool,
        }
        Ok(toml::Value::try_from(Context {
            kind,
            package: &request.name,
            dependency_alias: &request.alias,
            requirement: &request.req,
            features: &request.features,
            parent: request.parent.as_deref(),
            link,
            available_versions: self
                .cache
                .get(&request.name)
                .into_iter()
                .flatten()
                .rev()
                .take(10)
                .map(|e| e.record.vers.as_str())
                .collect(),
            context_is_sample: true,
        })?)
    }
    fn annotate(
        &self,
        error: Box<dyn std::error::Error + Send + Sync>,
        root: &Root,
        index: usize,
    ) -> Box<dyn std::error::Error + Send + Sync> {
        if crate::error_kind(error.as_ref()) != ErrorKind::Invalid {
            return error;
        }
        let mut typed = if let Some(typed) = error.downcast_ref::<Error>() {
            typed.clone()
        } else {
            Error::new(ErrorKind::Invalid, error.to_string())
        };
        let report = typed.report.get_or_insert_with(|| {
            self.last_report
                .clone()
                .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()))
        });
        if let Some(table) = report.as_table_mut() {
            table.insert("root_index".into(), toml::Value::Integer(index as i64));
            table.insert("root_name".into(), toml::Value::String(root.name.clone()));
            table.insert(
                "root_version".into(),
                toml::Value::String(root.version.clone()),
            );
        }
        typed.into()
    }
    fn tick(&mut self) -> Result<()> {
        events::check()?;
        self.work += 1;
        if self.work > self.bounds.work {
            return Err(budget("native closure work limit exceeded"));
        }
        Ok(())
    }
    fn load(&mut self, name: &str) -> Result<()> {
        index_path(name)?;
        if self.cache.contains_key(name) {
            return Ok(());
        }
        if self.frozen {
            return Err(invalid("filtered replay requires an unplanned index file"));
        }
        if self.cache.len() >= self.bounds.crates {
            return Err(budget("crate count exceeded"));
        }
        let bytes = self.source.index(name, self.bounds.index_bytes)?;
        if bytes.len() > self.bounds.index_bytes {
            return Err(budget("index source exceeded file bound"));
        }
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| budget("metadata overflow"))?;
        if self.metadata_bytes > self.bounds.total_metadata_bytes {
            return Err(budget("aggregate metadata exceeded"));
        }
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for raw in bytes.split_inclusive(|b| *b == b'\n') {
            self.tick()?;
            let record: Record = serde_json::from_slice(raw)
                .map_err(|_| invalid(format!("invalid index JSON for {name}")))?;
            if record.name != name
                || record.v > 2
                || record.v == 0
                || record.cksum.len() != 64
                || !record.cksum.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(invalid(format!(
                    "unsupported or inconsistent index record for {name}"
                )));
            }
            let version =
                Version::parse(&record.vers).map_err(|_| invalid("invalid index version"))?;
            let mut unique_version = version.clone();
            unique_version.build = semver::BuildMetadata::EMPTY;
            if !seen.insert(unique_version.to_string()) {
                return Err(invalid("duplicate index version"));
            }
            if record.v != 2 && !record.features2.is_empty() {
                return Err(invalid("features2 requires index schema 2"));
            }
            entries.push(Entry {
                record,
                version,
                raw: raw.to_vec(),
            });
        }
        entries.sort_by(|a, b| b.version.cmp(&a.version));
        self.cache.insert(name.to_owned(), entries);
        Ok(())
    }
    fn entry(&mut self, id: &Identity) -> Result<Entry> {
        self.load(&id.0)?;
        self.cache[&id.0]
            .iter()
            .find(|e| e.record.vers == id.1)
            .cloned()
            .ok_or_else(|| invalid(format!("missing exact version {}@{}", id.0, id.1)))
    }
    fn candidates(&mut self, request: &Request) -> Result<Vec<Identity>> {
        self.load(&request.name)?;
        let req = VersionReq::parse(&request.req)
            .map_err(|_| invalid("invalid dependency requirement"))?;
        let entries = &self.cache[&request.name];
        let mut work = self.work;
        let mut result = Vec::new();
        for entry in entries {
            events::check()?;
            work += 1;
            if work > self.bounds.work {
                return Err(budget("candidate evaluation work exceeded"));
            }
            if !entry.record.yanked
                && (request.parent.is_some()
                    || request.req.strip_prefix('=') == Some(entry.record.vers.as_str()))
                && req.matches(&entry.version)
                && request
                    .features
                    .iter()
                    .all(|f| has_feature(&entry.record, f))
            {
                result.push((request.name.clone(), entry.record.vers.clone()));
            }
        }
        self.work = work;
        if result.is_empty() {
            let matching = entries
                .iter()
                .any(|e| !e.record.yanked && req.matches(&e.version));
            self.last_report = Some(self.context(
                request,
                if matching {
                    "missing_feature"
                } else {
                    "no_matching_version"
                },
                None,
            )?);
        }
        Ok(result)
    }

    fn request(&mut self, root: &Root) -> Result<Request> {
        Version::parse(&root.version).map_err(|_| invalid("root needs exact version"))?;
        self.load(&root.name)?;
        Ok(Request {
            name: root.name.clone(),
            req: format!("={}", root.version),
            features: root.features.iter().cloned().collect(),
            defaults: root.default_features,
            parent: None,
            alias: root.name.clone(),
            target: None,
            lock_only: false,
        })
    }
}
#[derive(Clone, Debug)]
struct Request {
    name: String,
    req: String,
    features: BTreeSet<String>,
    defaults: bool,
    parent: Option<String>,
    alias: String,
    target: Option<String>,
    lock_only: bool,
}
#[derive(Default)]
struct Features {
    active: BTreeSet<String>,
    forwarded: BTreeMap<String, BTreeSet<String>>,
    lock_forwarded: BTreeMap<String, BTreeSet<String>>,
}
fn feature_map(record: &Record) -> BTreeMap<String, Vec<String>> {
    let mut map = record.features.clone();
    for (k, v) in &record.features2 {
        map.entry(k.clone()).or_default().extend(v.iter().cloned());
    }
    let suppressed: BTreeSet<String> = map
        .values()
        .flatten()
        .filter_map(|f| f.strip_prefix("dep:").map(str::to_owned))
        .collect();
    for dep in &record.deps {
        if dep.optional && !suppressed.contains(dep.name.as_str()) && !map.contains_key(&dep.name) {
            map.insert(dep.name.clone(), vec![format!("dep:{}", dep.name)]);
        }
    }
    map
}
fn has_feature(record: &Record, name: &str) -> bool {
    feature_map(record).contains_key(name)
}
fn expand(
    record: &Record,
    requested: &BTreeSet<String>,
    defaults: bool,
) -> std::result::Result<Features, String> {
    let map = feature_map(record);
    let mut queue: VecDeque<String> = requested.iter().cloned().collect();
    if defaults && map.contains_key("default") {
        queue.push_back("default".into());
    }
    let mut seen = BTreeSet::new();
    let mut out = Features::default();
    let mut weak = Vec::new();
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(alias) = name.strip_prefix("dep:") {
            if !record.deps.iter().any(|d| d.name == alias && d.optional) {
                return Err(format!("unknown optional dependency {alias}"));
            }
            out.active.insert(alias.into());
        } else if let Some((alias, feature)) = name.split_once('/') {
            if let Some(alias) = alias.strip_suffix('?') {
                weak.push((alias.to_owned(), feature.to_owned()));
            } else {
                out.active.insert(alias.into());
                out.forwarded
                    .entry(alias.into())
                    .or_default()
                    .insert(feature.into());
            }
        } else if let Some(values) = map.get(&name) {
            queue.extend(values.iter().cloned());
        } else {
            return Err(format!(
                "{}@{} has no feature {name}",
                record.name, record.vers
            ));
        }
    }
    for (alias, feature) in weak {
        out.lock_forwarded
            .entry(alias.clone())
            .or_default()
            .insert(feature.clone());
        if out.active.contains(&alias)
            || record
                .deps
                .iter()
                .any(|d| d.name == alias && !d.optional && d.kind.as_deref() != Some("dev"))
        {
            out.forwarded.entry(alias).or_default().insert(feature);
        }
    }
    Ok(out)
}
fn dependencies(entry: &Entry, features: &Features) -> Result<Vec<(usize, Request)>> {
    let mut result = Vec::new();
    for (index, dep) in entry.record.deps.iter().enumerate() {
        if dep.kind.as_deref() == Some("dev") {
            continue;
        }
        if !matches!(dep.kind.as_deref(), None | Some("normal" | "build")) {
            return Err(invalid("unknown dependency kind"));
        }
        if dep.optional
            && !features.active.contains(&dep.name)
            && !features.lock_forwarded.contains_key(&dep.name)
        {
            continue;
        }
        if dep.registry.as_deref().is_some_and(|r| {
            r != "https://github.com/rust-lang/crates.io-index"
                && r != "sparse+https://index.crates.io/"
        }) {
            return Err(invalid("alternate registry dependency is unsupported"));
        }
        let mut wanted: BTreeSet<String> = dep.features.iter().cloned().collect();
        if let Some(extra) = features.forwarded.get(&dep.name) {
            wanted.extend(extra.iter().cloned());
        }
        if let Some(extra) = features.lock_forwarded.get(&dep.name) {
            wanted.extend(extra.iter().cloned());
        }
        result.push((
            index,
            Request {
                name: dep.package().to_owned(),
                req: dep.req.clone(),
                features: wanted,
                defaults: dep.default_features,
                parent: Some(format!("{}@{}", entry.record.name, entry.record.vers)),
                alias: dep.name.clone(),
                target: dep.target.clone(),
                lock_only: dep.optional && !features.active.contains(&dep.name),
            },
        ));
    }
    Ok(result)
}
fn class(version: &Version) -> (u64, u64, u64) {
    if version.major > 0 {
        (version.major, 0, 0)
    } else if version.minor > 0 {
        (0, version.minor, 0)
    } else {
        (0, 0, version.patch)
    }
}
fn edge_key(parent: &Identity, index: usize) -> String {
    format!("{}@{}:{index}", parent.0, parent.1)
}
enum Evaluation {
    Need(String, Request),
    Conflict(String),
    Complete(BTreeMap<Identity, Request>),
}
impl Planner<'_> {
    fn evaluate(
        &mut self,
        root: &Root,
        bindings: &BTreeMap<String, Identity>,
    ) -> Result<Evaluation> {
        let request = self.request(root)?;
        let mut queue = VecDeque::from([(String::from("root"), request)]);
        let mut nodes: BTreeMap<Identity, Request> = BTreeMap::new();
        let mut edges: BTreeMap<Identity, BTreeSet<Identity>> = BTreeMap::new();
        let mut classes = BTreeMap::new();
        let mut links = BTreeMap::new();
        while let Some((key, request)) = queue.pop_front() {
            self.tick()?;
            let Some(id) = bindings.get(&key) else {
                return Ok(Evaluation::Need(key, request));
            };
            let entry = self.entry(id)?;
            let req =
                VersionReq::parse(&request.req).map_err(|_| invalid("invalid requirement"))?;
            if id.0 != request.name || entry.record.yanked || !req.matches(&entry.version) {
                return Ok(Evaluation::Conflict(format!(
                    "{} cannot satisfy {}",
                    request.name, request.req
                )));
            }
            let slot = (id.0.clone(), class(&entry.version));
            if classes
                .insert(slot, id.1.clone())
                .is_some_and(|old| old != id.1)
            {
                self.last_report = Some(self.context(&request, "unification", None)?);
                return Ok(Evaluation::Conflict(format!(
                    "compatible versions of {} must unify",
                    id.0
                )));
            }
            if let Some(link) = &entry.record.links {
                if links
                    .insert(link.clone(), id.clone())
                    .is_some_and(|old| old != *id)
                {
                    self.last_report = Some(self.context(&request, "links", Some(link))?);
                    return Ok(Evaluation::Conflict(format!("links conflict: {link}")));
                }
            }
            if let Some(parent) = &request.parent {
                let (name, version) = parent
                    .split_once('@')
                    .ok_or_else(|| invalid("invalid native graph parent"))?;
                edges
                    .entry((name.into(), version.into()))
                    .or_default()
                    .insert(id.clone());
            }
            let merged = if let Some(old) = nodes.get_mut(id) {
                let before = (old.features.clone(), old.defaults);
                old.features.extend(request.features);
                old.defaults |= request.defaults;
                if before == (old.features.clone(), old.defaults) {
                    continue;
                }
                old.clone()
            } else {
                nodes.insert(id.clone(), request.clone());
                request
            };
            let features = match expand(&entry.record, &merged.features, merged.defaults) {
                Ok(f) => f,
                Err(message) => return Ok(Evaluation::Conflict(message)),
            };
            for (index, mut dep) in dependencies(&entry, &features)? {
                dep.lock_only |= merged.lock_only;
                queue.push_back((edge_key(id, index), dep));
            }
            if nodes.len() > self.bounds.versions {
                return Err(budget("witness version count exceeded"));
            }
        }
        let mut degree: BTreeMap<_, usize> = nodes.keys().cloned().map(|id| (id, 0)).collect();
        for targets in edges.values() {
            for id in targets {
                *degree
                    .get_mut(id)
                    .ok_or_else(|| invalid("unknown native graph node"))? += 1;
            }
        }
        let mut pending: VecDeque<_> = degree
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut visited = 0;
        while let Some(id) = pending.pop_front() {
            self.tick()?;
            visited += 1;
            if let Some(targets) = edges.get(&id) {
                for target in targets {
                    let count = degree.get_mut(target).unwrap();
                    *count -= 1;
                    if *count == 0 {
                        pending.push_back(target.clone());
                    }
                }
            }
        }
        if visited != nodes.len() {
            // Cargo can reject cycles after selecting an otherwise consistent graph,
            // without retrying an older version. A different native witness is not enough.
            let mut error = Error::new(
                ErrorKind::Invalid,
                "cyclic normal/build dependency graph is unsupported by Cargo",
            );
            let mut report = toml::map::Map::new();
            report.insert("kind".into(), toml::Value::String("cycle".into()));
            report.insert("root_name".into(), toml::Value::String(root.name.clone()));
            report.insert(
                "root_version".into(),
                toml::Value::String(root.version.clone()),
            );
            report.insert(
                "involved_versions".into(),
                toml::Value::try_from(
                    degree
                        .iter()
                        .filter(|(_, d)| **d > 0)
                        .take(10)
                        .map(|((n, v), _)| format!("{n}@{v}"))
                        .collect::<Vec<_>>(),
                )?,
            );
            error.report = Some(toml::Value::Table(report));
            return Err(error.into());
        }
        Ok(Evaluation::Complete(nodes))
    }
    fn solve(
        &mut self,
        root: &Root,
        bindings: &mut BTreeMap<String, Identity>,
        _depth: usize,
    ) -> Result<std::result::Result<BTreeMap<Identity, Request>, String>> {
        // Explicit frames avoid one stack frame per dependency edge.
        let mut stack: Vec<(String, std::vec::IntoIter<Identity>)> = Vec::new();
        loop {
            self.tick()?;
            let mut why = match self.evaluate(root, bindings)? {
                Evaluation::Complete(nodes) => return Ok(Ok(nodes)),
                Evaluation::Need(key, request) => {
                    let candidates = self.candidates(&request)?;
                    stack.push((key, candidates.into_iter()));
                    format!("no candidate for {} {}", request.name, request.req)
                }
                Evaluation::Conflict(message) => message,
            };
            loop {
                let Some((key, candidates)) = stack.last_mut() else {
                    return Ok(Err(why));
                };
                bindings.remove(key);
                if let Some(id) = candidates.next() {
                    self.attempts += 1;
                    if self.attempts > self.bounds.attempts {
                        return Err(budget("native search attempts exceeded"));
                    }
                    bindings.insert(key.clone(), id);
                    break;
                }
                stack.pop();
                why = format!("no coherent candidate: {why}");
                if why.len() > 2048 {
                    let mut end = 2048;
                    while !why.is_char_boundary(end) {
                        end -= 1;
                    }
                    why.truncate(end);
                }
            }
        }
    }
    fn conservative(&mut self, root: &Root) -> Result<BTreeMap<Identity, Vec<Request>>> {
        let request = self.request(root)?;
        let mut queue = VecDeque::from([request]);
        let mut seen: BTreeMap<Identity, Request> = BTreeMap::new();
        let mut reasons: BTreeMap<Identity, Vec<Request>> = BTreeMap::new();
        while let Some(request) = queue.pop_front() {
            self.tick()?;
            let id = self
                .candidates(&request)?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    invalid(format!(
                        "no eligible candidate: {} {}",
                        request.name, request.req
                    ))
                })?;
            reasons.entry(id.clone()).or_default().push(request.clone());
            let merged = if let Some(old) = seen.get_mut(&id) {
                let before = (old.features.clone(), old.defaults);
                old.features.extend(request.features);
                old.defaults |= request.defaults;
                if before == (old.features.clone(), old.defaults) {
                    continue;
                }
                old.clone()
            } else {
                seen.insert(id.clone(), request.clone());
                request
            };
            let entry = self.entry(&id)?;
            let features =
                expand(&entry.record, &merged.features, merged.defaults).map_err(invalid)?;
            for (_, mut dep) in dependencies(&entry, &features)? {
                dep.lock_only |= merged.lock_only;
                queue.push_back(dep);
            }
            if seen.len() > self.bounds.versions {
                return Err(budget("conservative version count exceeded"));
            }
        }
        Ok(reasons)
    }
}
/// Resolve exact roots natively with no archive operation or runtime Cargo.
///
/// Performs conservative closure, bounded backtracking and filtered-index witness replay.
/// An error refuses the whole plan. This Phase 0 API does not yet freeze fetch state.
pub fn plan(source: &mut dyn IndexSource, roots: &[Root], bounds: Bounds) -> Result<Plan> {
    if roots.is_empty() || roots.len() > 4096 {
        return Err(invalid("expected 1..4096 exact root profiles"));
    }
    events::stage("resolve")?;
    let mut planner = Planner {
        source,
        bounds,
        cache: BTreeMap::new(),
        metadata_bytes: 0,
        work: 0,
        attempts: 0,
        frozen: false,
        last_report: None,
    };
    let mut planned: BTreeMap<Identity, PlannedVersion> = BTreeMap::new();
    let mut witnesses = Vec::new();
    for (root_index, root) in roots.iter().enumerate() {
        planner.last_report = None;
        let conservative = planner
            .conservative(root)
            .map_err(|e| planner.annotate(e, root, root_index))?;
        let mut bindings = BTreeMap::new();
        let witness = planner
            .solve(root, &mut bindings, 0)
            .map_err(|e| planner.annotate(e, root, root_index))?
            .map_err(|message| {
                planner.annotate(
                    invalid(format!("{}@{}: {message}", root.name, root.version)),
                    root,
                    root_index,
                )
            })?;
        for (id, requests, extra) in conservative.into_iter().map(|(id, r)| (id, r, true)).chain(
            witness
                .iter()
                .map(|(id, r)| (id.clone(), vec![r.clone()], false)),
        ) {
            let entry = planner.entry(&id)?;
            let version = planned.entry(id.clone()).or_insert_with(|| PlannedVersion {
                name: id.0.clone(),
                version: id.1.clone(),
                checksum: entry.record.cksum.clone(),
                index_line: entry.raw.clone(),
                reasons: Vec::new(),
            });
            for request in requests {
                version.reasons.push(Reason {
                    root: root_index,
                    parent: request.parent,
                    alias: request.alias,
                    requirement: request.req,
                    target: request.target,
                    conservative: extra,
                    lock_only: request.lock_only,
                    features: request.features.into_iter().collect(),
                    default_features: request.defaults,
                });
            }
        }
        witnesses.push(Witness {
            root: root.clone(),
            versions: witness.keys().map(|(n, v)| format!("{n}@{v}")).collect(),
        });
        if planned.len() > planner.bounds.versions {
            return Err(budget("plan version count exceeded"));
        }
    }
    events::stage("closure-check")?;
    planner.frozen = true;
    for entries in planner.cache.values_mut() {
        entries.retain(|e| planned.contains_key(&(e.record.name.clone(), e.record.vers.clone())));
    }
    for root in roots {
        if let Err(message) = planner.solve(root, &mut BTreeMap::new(), 0)? {
            return Err(invalid(format!("filtered index replay: {message}")));
        }
    }
    for version in planned.values_mut() {
        version.reasons.sort();
        version.reasons.dedup();
    }
    Ok(Plan {
        schema: 0,
        versions: planned.into_values().collect(),
        witnesses,
        metadata_bytes: planner.metadata_bytes,
        index_requests: planner.cache.len(),
        archive_requests: 0,
        attempts: planner.attempts,
    })
}
/// Run Phase 0 planning under an embedding application's existing observer/cancellation session.
pub fn plan_with_control(
    source: &mut dyn IndexSource,
    roots: &[Root],
    bounds: Bounds,
    control: &events::Control,
) -> Result<Plan> {
    control.run("crates-plan", || plan(source, roots, bounds))
}
