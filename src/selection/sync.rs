//! One-shot orchestration. Publication, activation and pruning are separate operations.
use super::{
    collection::{self, CollectionSnapshot, Input, ToolchainRecord},
    Config, Plan, ResolvedPlan, Result,
};
use crate::{sha256, Source, Transfer, ARTIFACT_LIMIT};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
#[derive(Default, Debug, Serialize)]
pub struct Report {
    pub status: String,
    pub selections: usize,
    pub frozen: Vec<SelectionStatus>,
    pub metadata_requests: u64,
    pub local_payload_bytes: u64,
    pub upstream_payload_bytes: u64,
    pub upstream_attempts: u64,
    pub files: usize,
    pub recorded_bytes: u64,
    pub resumed: bool,
}
#[derive(Debug, Serialize)]
pub struct SelectionStatus {
    pub name: String,
    pub requested: String,
    pub pinned_toolchain: String,
    pub release: String,
    pub date: String,
}
#[derive(Debug)]
pub struct SyncResult {
    pub mirror: PathBuf,
    pub report: Report,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
    config_sha256: String,
    previous_sha256: Option<String>,
    plans: Vec<String>,
}
#[derive(Default)]
struct Counts {
    metadata: u64,
    local: u64,
    upstream: u64,
    attempts: u64,
}
struct MetadataSource<'a, S> {
    inner: &'a S,
    counts: &'a Mutex<Counts>,
}
impl<S: Source> Source for MetadataSource<'_, S> {
    fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
        self.counts.lock().unwrap().metadata += 1;
        self.inner.fetch(url, out, limit)
    }
}
struct PayloadSource<'a, S> {
    inner: &'a S,
    known: &'a BTreeMap<String, PathBuf>,
    expected: &'a BTreeMap<String, String>,
    counts: &'a Mutex<Counts>,
}
struct Counter<'a> {
    out: &'a mut dyn Write,
    counts: &'a Mutex<Counts>,
    local: bool,
}
impl Write for Counter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.out.write(buf)?;
        let mut c = self.counts.lock().unwrap();
        if self.local {
            c.local += n as u64;
        } else {
            c.upstream += n as u64;
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
impl<S: Source> Source for PayloadSource<'_, S> {
    fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
        match self.fetch_from(url, out, 0, limit)? {
            Transfer::Complete => Ok(()),
            Transfer::Restart => Err("payload source requested unexpected restart".into()),
        }
    }
    fn fetch_from(
        &self,
        url: &str,
        out: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        let hash = self
            .expected
            .get(url)
            .ok_or("unplanned sync payload request")?;
        if let Some(path) = self.known.get(hash) {
            let (n, actual) = crate::resume::hash_file(path, ARTIFACT_LIMIT)?;
            if &actual != hash {
                return Err("verified local input changed during sync".into());
            }
            if offset > n {
                return Ok(Transfer::Restart);
            }
            if n - offset > limit {
                return Err("local reuse exceeds remaining workspace budget".into());
            }
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(offset))?;
            let mut writer = Counter {
                out,
                counts: self.counts,
                local: true,
            };
            crate::copy_bounded(file, &mut writer, limit)?;
            Ok(Transfer::Complete)
        } else {
            self.counts.lock().unwrap().attempts += 1;
            let mut writer = Counter {
                out,
                counts: self.counts,
                local: false,
            };
            self.inner.fetch_from(url, &mut writer, offset, limit)
        }
    }
}
fn stage(message: &str) {
    let _ = writeln!(io::stderr().lock(), "sync: {message}");
}
fn real_dir(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("sync directory must not be a link".into());
    }
    Ok(())
}
fn regular(path: &Path, limit: u64) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > limit {
        return Err("invalid sync state file".into());
    }
    Ok(())
}
fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn children(root: &Path, names: &BTreeSet<PathBuf>) -> Result<()> {
    real_dir(root)?;
    for entry in fs::read_dir(root)? {
        let e = entry?;
        if !names.contains(&PathBuf::from(e.file_name())) {
            return Err("unexpected sync workspace entry".into());
        }
        if e.file_type()?.is_symlink() {
            return Err("sync workspace links are refused".into());
        }
    }
    Ok(())
}
fn named(names: &[&str]) -> BTreeSet<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}
fn plan_hash(p: &Plan) -> Result<String> {
    Ok(sha256(toml::to_string_pretty(p)?.as_bytes()))
}
fn record(p: &Plan) -> ToolchainRecord {
    ToolchainRecord {
        selection: p.selection.clone(),
        release: p.resolved_release.clone(),
        date: p.date.clone(),
        pinned_toolchain: p.pinned_toolchain.clone(),
    }
}
fn read_inventory(path: &Path) -> Result<CollectionSnapshot> {
    Ok(toml::from_str(std::str::from_utf8(&super::read_file(
        &path.join("snapshot.toml"),
        1024 * 1024,
    )?)?)?)
}
fn checked_total(input: &Input) -> Result<u64> {
    Ok(input.files.values().try_fold(0u64, |n, e| {
        n.checked_add(e.bytes).ok_or("sync byte count overflow")
    })?)
}
/// Detect inconsistencies before payload download. `true` means every requested record
/// and file already exists in the independently verified previous generation.
fn preflight(plans: &[ResolvedPlan], old: Option<&Input>, budget: u64) -> Result<bool> {
    let mut desired = BTreeMap::new();
    let mut records = old.map(|i| i.records.clone()).unwrap_or_default();
    let mut unchanged = old.is_some();
    for resolved in plans {
        let p = &resolved.plan;
        let r = record(p);
        if !old.is_some_and(|i| i.records.contains(&r)) {
            unchanged = false;
        }
        if !records.contains(&r) {
            records.push(r);
        }
        let entries = p
            .artifacts
            .iter()
            .map(|a| (a.path.clone(), a.sha256.clone()))
            .chain(
                super::snapshot_metadata(resolved)?
                    .iter()
                    .map(|(p, b)| (p.clone(), sha256(b))),
            )
            .collect::<Vec<_>>();
        for (path, hash) in entries {
            if desired.get(&path).is_some_and(|h| h != &hash) {
                return Err("channel changed between resolutions or selections conflict; resolve a fresh coherent run".into());
            }
            if let Some(prior) = old.and_then(|i| i.files.get(&path)) {
                if prior.sha256 != hash {
                    unchanged = false;
                    if !collection::rolling(&path) {
                        return Err(format!("immutable previous path conflict: {path}").into());
                    }
                }
            } else {
                unchanged = false;
            }
            desired.insert(path, hash);
        }
    }
    let mut paths: BTreeSet<_> = old
        .map(|i| i.files.keys().cloned().collect())
        .unwrap_or_default();
    paths.extend(desired.keys().cloned());
    if paths.len() > 4096 || records.len() > 256 {
        return Err("sync generation file/record limit exceeded".into());
    }
    if let Some(old) = old {
        if checked_total(old)? > budget {
            return Err(
                "previous generation exceeds configured budget; sync never prunes it".into(),
            );
        }
    }
    Ok(unchanged)
}
fn result(
    status: &str,
    mirror: PathBuf,
    plans: &[ResolvedPlan],
    counts: &Mutex<Counts>,
    files: usize,
    bytes: u64,
    resumed: bool,
) -> SyncResult {
    let c = counts.lock().unwrap();
    SyncResult {
        mirror,
        report: Report {
            status: status.into(),
            selections: plans.len(),
            frozen: plans
                .iter()
                .map(|p| SelectionStatus {
                    name: p.plan.selection.name.clone(),
                    requested: p.plan.selection.toolchain.clone(),
                    pinned_toolchain: p.plan.pinned_toolchain.clone(),
                    release: p.plan.resolved_release.clone(),
                    date: p.plan.date.clone(),
                })
                .collect(),
            metadata_requests: c.metadata,
            local_payload_bytes: c.local,
            upstream_payload_bytes: c.upstream,
            upstream_attempts: c.attempts,
            files,
            recorded_bytes: bytes,
            resumed,
        },
    }
}
/// Synchronize all configured selections into a new immutable generation. No server
/// writes, client activation or pruning. Resume requires exact config/previous identity.
pub fn run(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    resume: bool,
    source: &impl Source,
) -> Result<SyncResult> {
    config.validate()?;
    if !resume && exists(output)? {
        return Err("sync output already exists; use --resume with unchanged configuration".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let target = parent.canonicalize()?.join(
        output
            .file_name()
            .ok_or("sync needs a new output directory")?,
    );
    if let Some(previous) = previous {
        if target.starts_with(previous.canonicalize()?) {
            return Err("sync output must not be inside the previous mirror".into());
        }
    }
    let counts = Mutex::new(Counts::default());
    stage(if previous.is_some() {
        "verify previous generation"
    } else {
        "start new mirror"
    });
    let old = previous.map(collection::read_input).transpose()?;
    if old
        .as_ref()
        .is_some_and(|i| i.rustup != config.rustup_version)
    {
        return Err("previous and configured Rustup bootstrap versions differ".into());
    }
    let config_hash = sha256(toml::to_string_pretty(config)?.as_bytes());
    let previous_hash = old.as_ref().map(|i| i.identity.clone());
    let root = output.join(".sync");
    let plan_root = root.join("plans");
    let snapshot_root = root.join("snapshots");
    let allowed_indices: BTreeSet<_> = (0..config.selections.len())
        .map(|n| PathBuf::from(n.to_string()))
        .collect();
    let mut guard = None;
    let resolved = if resume {
        real_dir(output)?;
        children(output, &named(&[".sync.lock", ".sync", "mirror"]))?;
        // Resume never initializes an unrelated directory, even an empty one.
        real_dir(&root)?;
        regular(&root.join("state.toml"), 64 * 1024)?;
        regular(&output.join(".sync.lock"), 0)?;
        let lock = lock(output)?;
        guard = Some(lock);
        children(
            &root,
            &named(&[
                "state.toml",
                "plans",
                "snapshots",
                "tree",
                "report.toml",
                "report.tmp",
            ]),
        )?;
        regular(&root.join("state.toml"), 64 * 1024)?;
        let state: State = toml::from_str(&fs::read_to_string(root.join("state.toml"))?)?;
        if state.schema != 1
            || state.config_sha256 != config_hash
            || state.previous_sha256 != previous_hash
            || state.plans.len() != config.selections.len()
        {
            return Err("sync resume identity/config/previous mismatch".into());
        }
        children(&plan_root, &allowed_indices)?;
        let mut plans = Vec::new();
        for (index, selected) in config.selections.iter().enumerate() {
            let p = super::load_plan(&plan_root.join(index.to_string()))?;
            if p.plan.selection != *selected
                || p.plan.rustup_version != config.rustup_version
                || p.plan.max_download_bytes != config.max_download_bytes
                || plan_hash(&p.plan)? != state.plans[index]
            {
                return Err("saved sync plan differs from initialized run".into());
            }
            plans.push(p);
        }
        plans
    } else {
        let mut plans = Vec::new();
        let metadata = MetadataSource {
            inner: source,
            counts: &counts,
        };
        for (index, selected) in config.selections.iter().enumerate() {
            stage(&format!(
                "resolve {}/{}: {} ({})",
                index + 1,
                config.selections.len(),
                selected.name,
                selected.toolchain
            ));
            plans.push(super::resolve(config, &selected.name, &metadata)?);
        }
        plans
    };
    let unchanged = preflight(&resolved, old.as_ref(), config.max_download_bytes)?;
    if unchanged && !resume {
        if exists(output)? {
            return Err("sync output appeared during resolution".into());
        }
        let old = old.as_ref().unwrap();
        stage("unchanged; previous generation is verified, no payload downloads");
        return Ok(result(
            "unchanged",
            previous.unwrap().to_path_buf(),
            &resolved,
            &counts,
            old.files.len(),
            checked_total(old)?,
            false,
        ));
    }
    if !resume {
        fs::create_dir(output)?;
        guard = Some(lock(output)?);
        fs::create_dir(&root)?;
        fs::create_dir(&plan_root)?;
        fs::create_dir(&snapshot_root)?;
        for (index, p) in resolved.iter().enumerate() {
            p.save(&plan_root.join(index.to_string()))?;
        }
        let state = State {
            schema: 1,
            config_sha256: config_hash,
            previous_sha256: previous_hash,
            plans: resolved
                .iter()
                .map(|r| plan_hash(&r.plan))
                .collect::<Result<_>>()?,
        };
        super::write_new(
            &root.join("state.toml"),
            toml::to_string_pretty(&state)?.as_bytes(),
        )?;
    }
    let _guard = guard.ok_or("sync lock not acquired")?;
    children(&snapshot_root, &allowed_indices)?;
    if exists(&root.join("report.toml"))? {
        regular(&root.join("report.toml"), 64 * 1024)?;
    }
    let expected: BTreeMap<_, _> = resolved
        .iter()
        .flat_map(|r| {
            r.plan
                .artifacts
                .iter()
                .map(|a| (format!("{}{}", crate::ORIGIN, a.path), a.sha256.clone()))
        })
        .collect();
    let mut known: BTreeMap<_, _> = old
        .as_ref()
        .map(|i| {
            i.files
                .values()
                .map(|e| (e.sha256.clone(), e.source.clone()))
                .collect()
        })
        .unwrap_or_default();
    let mut inputs = Vec::new();
    let mut remaining = config.max_download_bytes;
    for (index, p) in resolved.iter().enumerate() {
        stage(&format!(
            "fetch {}/{}: {} -> {}",
            index + 1,
            resolved.len(),
            p.plan.selection.name,
            p.plan.pinned_toolchain
        ));
        let out = snapshot_root.join(index.to_string());
        let payload = PayloadSource {
            inner: source,
            known: &known,
            expected: &expected,
            counts: &counts,
        };
        let mirror = super::fetch_limited(
            &plan_root.join(index.to_string()),
            &out,
            &payload,
            exists(&out)?,
            remaining,
        )?;
        let input = collection::read_input(&mirror)?;
        let bytes = checked_total(&input)?;
        remaining = remaining
            .checked_sub(bytes)
            .ok_or("selected working snapshots exceed configured byte budget")?;
        for e in input.files.values() {
            known.insert(e.sha256.clone(), e.source.clone());
        }
        inputs.push(mirror);
    }
    stage("verify and assemble generation");
    let (inventory, files) = collection::prepare(&inputs, previous, config.max_download_bytes)?;
    let completed = output.join("mirror");
    let status = if exists(&completed)? {
        collection::read_input(&completed)?;
        if read_inventory(&completed)? != inventory {
            return Err(
                "completed sync generation differs from frozen run; refusing overwrite".into(),
            );
        }
        "validated"
    } else {
        let tree = root.join("tree");
        let expected: BTreeSet<_> = files
            .keys()
            .map(PathBuf::from)
            .chain([PathBuf::from("snapshot.toml")])
            .collect();
        if exists(&tree)? {
            crate::resume::validate_tree(&tree, &expected)?;
            fs::remove_dir_all(&tree)?;
        }
        fs::create_dir(&tree)?;
        collection::write_prepared(&inventory, &files, &tree)?;
        if exists(&completed)? {
            return Err("completed generation appeared during sync".into());
        }
        fs::rename(&tree, &completed)?;
        "created"
    };
    let bytes = inventory.files.iter().map(|f| f.bytes).sum();
    let result = result(
        status,
        completed,
        &resolved,
        &counts,
        inventory.files.len(),
        bytes,
        resume,
    );
    let report = root.join("report.toml");
    let temporary = root.join("report.tmp");
    if exists(&temporary)? {
        regular(&temporary, 64 * 1024)?;
        fs::remove_file(&temporary)?;
    }
    super::write_new(
        &temporary,
        toml::to_string_pretty(&result.report)?.as_bytes(),
    )?;
    fs::rename(temporary, report)?;
    stage(&format!(
        "{}; {} files, {} upstream payload bytes, {} local payload bytes",
        result.report.status,
        result.report.files,
        result.report.upstream_payload_bytes,
        result.report.local_payload_bytes
    ));
    Ok(result)
}
fn lock(output: &Path) -> Result<File> {
    let p = output.join(".sync.lock");
    if exists(&p)? {
        regular(&p, 0)?;
    }
    let f = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(p)?;
    f.try_lock()
        .map_err(|_| "another sync owns this workspace, or file locking is unavailable")?;
    Ok(f)
}
