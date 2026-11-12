//! One-shot orchestration. Publication, activation and pruning are separate operations.
//!
//! [run] orchestrates a full [Config] with verified local reuse, retained history
//! and frozen-cohort resume. Always use the returned [SyncResult::mirror] path.
//! [run_forced] is an explicitly destructive restart and makes no backup.
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
/// Diagnostic counters for a successful one-shot synchronization.
///
/// Reports are not readiness/freshness certificates. No schema field is serialized;
/// byte counters describe this operation, not all retained storage or every network byte.
pub struct Report {
    /// `created`, `validated` (completed resume), or `unchanged` (previous mirror returned).
    pub status: String,
    /// Number of configured selections in this cohort.
    pub selections: usize,
    /// Resolved selection identities used for this run.
    pub frozen: Vec<SelectionStatus>,
    /// Counted source metadata requests; not a payload or HTTP-byte counter.
    pub metadata_requests: u64,
    /// Payload bytes written from verified local reuse during this invocation.
    pub local_payload_bytes: u64,
    /// Payload bytes written from upstream during this invocation, including partial attempts.
    pub upstream_payload_bytes: u64,
    /// Counted upstream payload transfer attempts, including failed/interrupted ones.
    pub upstream_attempts: u64,
    /// Number of recorded final payload/metadata files, excluding the inventory itself.
    pub files: usize,
    /// Sum of recorded final file bytes, excluding the inventory itself.
    pub recorded_bytes: u64,
    /// Whether this invocation used the explicit resume route.
    pub resumed: bool,
}
#[derive(Debug, Serialize)]
/// Frozen identity of one selection in a synchronization report.
pub struct SelectionStatus {
    /// Configuration selection label.
    pub name: String,
    /// Originally requested selector, which may be a moving channel.
    pub requested: String,
    /// Frozen numeric or dated-channel selector actually used.
    pub pinned_toolchain: String,
    /// Resolved numeric Rust compiler release.
    pub release: String,
    /// Resolved upstream manifest date.
    pub date: String,
}
#[derive(Debug)]
/// Completed local mirror path and diagnostic report.
///
/// Always use [`Self::mirror`]: an unchanged run can return the previous generation
/// without creating the requested new output path.
pub struct SyncResult {
    /// Actual completed mirror root; this may point at the supplied previous generation.
    pub mirror: PathBuf,
    /// Counters/identities for this invocation; not a substitute for file verification.
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
thread_local! {static ESTIMATE_SIZES:std::cell::Cell<bool>=const {std::cell::Cell::new(false)};}
/// Optional sync execution controls; estimates never change frozen plan identities.
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    /// Resume exactly the initialized frozen config/previous cohort.
    pub resume: bool,
    /// Explicitly discard old managed output/progress; mutually exclusive with resume.
    pub force: bool,
    /// Opt in to HEAD estimates and aggregate progress events.
    pub estimate_sizes: bool,
}
/// Run sync with explicit estimate/resume/force options and ordinary control semantics.
pub fn run_with_options(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    options: &RunOptions,
    source: &impl Source,
    control: &crate::events::Control,
) -> Result<SyncResult> {
    if options.resume && options.force {
        return Err(crate::Error::new(
            crate::ErrorKind::Invalid,
            "resume and force are mutually exclusive",
        )
        .into());
    }
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ESTIMATE_SIZES.with(|v| v.set(self.0));
        }
    }
    let _restore = Restore(ESTIMATE_SIZES.with(|v| v.replace(options.estimate_sizes)));
    control.run("sync", || {
        if options.force {
            run_forced(config, previous, output, source)
        } else {
            run(config, previous, output, options.resume, source)
        }
    })
}
struct Progress {
    lengths: BTreeMap<String, Option<u64>>,
    coverage: BTreeMap<String, u64>,
    transferred: u64,
    start: std::time::Instant,
}
impl Progress {
    fn emit(&self) -> Result<()> {
        let total = self
            .lengths
            .values()
            .try_fold(0u64, |sum, n| sum.checked_add((*n)?));
        let completed = self
            .coverage
            .values()
            .try_fold(0u64, |sum, n| sum.checked_add(*n))
            .ok_or_else(|| crate::Error::new(crate::ErrorKind::Budget, "progress sum overflow"))?;
        let elapsed = self.start.elapsed().as_secs_f64();
        let rate = if self.transferred > 0 && elapsed > 0.0 {
            Some(self.transferred as f64 / elapsed)
        } else {
            None
        };
        if let Some(control) = crate::events::current() {
            control.emit(crate::events::EventKind::TransferSummary {
                completed_bytes: completed,
                transferred_bytes: self.transferred,
                expected_bytes: total,
                bytes_per_second: rate,
                eta_seconds: total
                    .zip(rate)
                    .map(|(total, rate)| total.saturating_sub(completed) as f64 / rate),
                estimated: true,
            })?;
        }
        Ok(())
    }
}
struct PayloadSource<'a, S> {
    inner: &'a S,
    known: &'a BTreeMap<String, PathBuf>,
    expected: &'a BTreeMap<String, String>,
    counts: &'a Mutex<Counts>,
    progress: Option<&'a Mutex<Progress>>,
}
struct Counter<'a> {
    out: &'a mut dyn Write,
    counts: &'a Mutex<Counts>,
    local: bool,
    progress: Option<&'a Mutex<Progress>>,
    path: String,
    position: u64,
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
        drop(c);
        self.position += n as u64;
        if let Some(progress) = self.progress {
            let mut p = progress.lock().unwrap();
            let old = p.coverage.entry(self.path.clone()).or_default();
            *old = (*old).max(self.position);
            if !self.local {
                p.transferred += n as u64;
            }
            p.emit().map_err(io::Error::other)?;
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
                return Err(crate::Error::new(
                    crate::ErrorKind::Integrity,
                    "verified local input changed during sync",
                )
                .into());
            }
            if offset > n {
                return Ok(Transfer::Restart);
            }
            if n - offset > limit {
                return Err(crate::Error::new(
                    crate::ErrorKind::Budget,
                    "local reuse exceeds remaining workspace budget",
                )
                .into());
            }
            crate::events::artifact(
                url.strip_prefix(crate::ORIGIN).unwrap_or("local artifact"),
                "reuse",
                offset,
                Some(n),
            )?;
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(offset))?;
            let mut writer = Counter {
                out,
                counts: self.counts,
                local: true,
                progress: self.progress,
                path: url.into(),
                position: offset,
            };
            crate::copy_bounded(file, &mut writer, limit)?;
            Ok(Transfer::Complete)
        } else {
            self.counts.lock().unwrap().attempts += 1;
            if let Some(progress) = self.progress {
                let p = progress.lock().unwrap();
                crate::events::artifact(
                    url.strip_prefix(crate::ORIGIN).unwrap_or("artifact"),
                    if offset > 0 { "resume" } else { "download" },
                    offset,
                    p.lengths.get(url).copied().flatten(),
                )?;
            }
            let mut writer = Counter {
                out,
                counts: self.counts,
                local: false,
                progress: self.progress,
                path: url.into(),
                position: offset,
            };
            self.inner.fetch_from(url, &mut writer, offset, limit)
        }
    }
}
fn stage(message: &str) -> Result<()> {
    crate::events::stage(message)?;
    let _ = writeln!(io::stderr().lock(), "sync: {message}");
    Ok(())
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
        n.checked_add(e.bytes)
            .ok_or_else(|| crate::Error::new(crate::ErrorKind::Budget, "sync byte count overflow"))
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
                return Err(crate::Error::new(crate::ErrorKind::Integrity, "channel changed between resolutions or selections conflict; resolve a fresh coherent run").into());
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
        return Err(crate::Error::new(
            crate::ErrorKind::Budget,
            "sync generation file/record limit exceeded",
        )
        .into());
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
/// Explicitly discard a managed run's progress and mirror, then resolve a fresh
/// cohort at the same path. No backup is kept; `previous` must not overlap.
///
/// Validates managed output and holds its lock while removing old progress and
/// mirror files. Fresh resolution may fail after old data is gone. New output
/// behaves like a fresh [run]. Remote storage and previous inputs are untouched.
///
/// # Errors
/// Refuses unrelated/linked/special paths, previous/output overlap, competing
/// writers, invalid state or any validation/transfer/I/O error from [run].
pub fn run_forced(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    source: &impl Source,
) -> Result<SyncResult> {
    crate::events::check()?;
    config.validate()?;
    if exists(output)? {
        real_dir(output)?;
        children(output, &named(&[".sync.lock", ".sync", "mirror"]))?;
        if exists(&output.join(".sync"))? {
            real_dir(&output.join(".sync"))?;
        } else {
            // A failed fresh resolution after force leaves only this lock marker.
            children(output, &named(&[".sync.lock"]))?;
        }
        regular(&output.join(".sync.lock"), 0)?;
        let target = output.canonicalize()?;
        if let Some(previous) = previous {
            let prior = previous.canonicalize()?;
            if prior.starts_with(&target) || target.starts_with(&prior) {
                return Err("force output and previous mirror must not overlap".into());
            }
            collection::read_input(previous)?;
        }
        let guard = lock(output)?;
        // Validate the full destructive scope before deleting anything. Never follow
        // links/junctions, and keep the lock file in place throughout restart.
        let mut entries = 0;
        for name in ["mirror", ".sync"] {
            let path = target.join(name);
            if exists(&path)? {
                validate_reset_tree(&path, &target, 0, &mut entries)?;
            }
        }
        stage("--force: discarding existing mirror and progress; no backup")?;
        for name in ["mirror", ".sync"] {
            crate::events::check()?;
            let path = target.join(name);
            if exists(&path)? {
                fs::remove_dir_all(path)?;
            }
        }
        return run_inner(config, previous, output, false, source, Some(guard));
    }
    run(config, previous, output, false, source)
}
fn validate_reset_tree(path: &Path, root: &Path, depth: usize, count: &mut usize) -> Result<()> {
    *count += 1;
    if depth > 64 || *count > 300_000 {
        return Err(crate::Error::new(
            crate::ErrorKind::Budget,
            "force reset tree exceeds validation budget",
        )
        .into());
    }
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("force refuses reparse points".into());
        }
    }
    if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
        return Err("force refuses links and special files".into());
    }
    let canonical = path.canonicalize()?;
    if canonical == root || !canonical.starts_with(root) {
        return Err("force reset path escapes the output directory".into());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            validate_reset_tree(&entry?.path(), root, depth + 1, count)?;
        }
    }
    Ok(())
}
/// Synchronize all configured selections into a new immutable generation. No server
/// writes, client activation or pruning. Resume requires exact config/previous identity.
///
/// Config defines the whole cohort; previous may be a completed schema-2/3 root.
/// Output must be new unless resume is true, which requires initialized progress
/// with identical config/previous identity. An unchanged run may return previous
/// without creating output: always use [SyncResult::mirror].
///
/// Fresh runs read live metadata; resumes replay frozen metadata. Missing
/// payloads still need source access.
///
/// # Errors
/// Refuses invalid inputs, incompatible state, writer conflicts, changed identity,
/// unavailable selections, resource/hash failures or transport/local I/O errors.
/// Failures can leave private resumable state.
pub fn run(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    resume: bool,
    source: &impl Source,
) -> Result<SyncResult> {
    run_inner(config, previous, output, resume, source, None)
}
fn run_inner(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    resume: bool,
    source: &impl Source,
    forced_guard: Option<File>,
) -> Result<SyncResult> {
    let forced = forced_guard.is_some();
    crate::events::check()?;
    config.validate()?;
    if !resume && !forced && exists(output)? {
        return Err(crate::Error::new(crate::ErrorKind::OutputExists, "sync output already exists; use --resume with unchanged configuration, or --force to discard it and start fresh").into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let target = parent
        .canonicalize()?
        .join(output.file_name().ok_or_else(|| {
            crate::Error::new(
                crate::ErrorKind::OutputExists,
                "sync needs a new output directory",
            )
        })?);
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
    })?;
    let old = previous.map(collection::read_input).transpose()?;
    if old
        .as_ref()
        .is_some_and(|i| i.rustup != config.rustup_version)
    {
        return Err("previous and configured Rustup bootstrap versions differ".into());
    }
    let mut frozen_config = config.clone();
    frozen_config.network = None; // Routing/trust options never change the frozen payload recipe.
    let config_hash = sha256(toml::to_string_pretty(&frozen_config)?.as_bytes());
    let previous_hash = old.as_ref().map(|i| i.identity.clone());
    let root = output.join(".sync");
    let plan_root = root.join("plans");
    let snapshot_root = root.join("snapshots");
    let allowed_indices: BTreeSet<_> = (0..config.selections.len())
        .map(|n| PathBuf::from(n.to_string()))
        .collect();
    let mut guard = forced_guard;
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
            return Err(crate::Error::new(
                crate::ErrorKind::ResumeMismatch,
                "sync resume identity/config/previous mismatch",
            )
            .into());
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
                return Err(crate::Error::new(
                    crate::ErrorKind::ResumeMismatch,
                    "saved sync plan differs from initialized run",
                )
                .into());
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
            ))?;
            plans.push(super::resolve(config, &selected.name, &metadata)?);
        }
        plans
    };
    let unchanged = preflight(&resolved, old.as_ref(), config.max_download_bytes)?;
    if unchanged && !resume && !forced {
        if exists(output)? {
            return Err("sync output appeared during resolution".into());
        }
        let old = old.as_ref().unwrap();
        stage("unchanged; previous generation is verified, no payload downloads")?;
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
        if !forced {
            fs::create_dir(output)?;
            guard = Some(lock(output)?);
        } else {
            children(output, &named(&[".sync.lock"]))?;
        }
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
    let progress = if ESTIMATE_SIZES.with(|v| v.get()) {
        stage("estimate payload sizes (HEAD)")?;
        let mut lengths = BTreeMap::new();
        let mut sum = 0u64;
        for url in expected.keys() {
            crate::events::check()?;
            let n = source.content_length(url)?;
            if let Some(n) = n {
                sum = sum.checked_add(n).ok_or_else(|| {
                    crate::Error::new(crate::ErrorKind::Budget, "estimate sum overflow")
                })?;
            }
            lengths.insert(url.clone(), n);
        }
        let progress = Mutex::new(Progress {
            lengths,
            coverage: BTreeMap::new(),
            transferred: 0,
            start: std::time::Instant::now(),
        });
        progress.lock().unwrap().emit()?;
        Some(progress)
    } else {
        None
    };
    let _hints = crate::events::SizeHints::new(
        progress
            .as_ref()
            .map(|p| {
                p.lock()
                    .unwrap()
                    .lengths
                    .iter()
                    .filter_map(|(url, size)| {
                        size.map(|n| (url.strip_prefix(crate::ORIGIN).unwrap_or(url).to_owned(), n))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    );
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
        ))?;
        let out = snapshot_root.join(index.to_string());
        let payload = PayloadSource {
            inner: source,
            known: &known,
            expected: &expected,
            counts: &counts,
            progress: progress.as_ref(),
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
        remaining = remaining.checked_sub(bytes).ok_or_else(|| {
            crate::Error::new(
                crate::ErrorKind::Budget,
                "selected working snapshots exceed configured byte budget",
            )
        })?;
        if let Some(progress) = &progress {
            let mut p = progress.lock().unwrap();
            for (path, file) in &input.files {
                let url = format!("{}{path}", crate::ORIGIN);
                if p.lengths.contains_key(&url) {
                    p.coverage.insert(url, file.bytes);
                }
            }
            p.emit()?;
        }
        for e in input.files.values() {
            known.insert(e.sha256.clone(), e.source.clone());
        }
        inputs.push(mirror);
    }
    stage("verify and assemble generation")?;
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
        crate::events::check()?;
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
    crate::events::check()?;
    fs::rename(temporary, report)?;
    stage(&format!(
        "{}; {} files, {} upstream payload bytes, {} local payload bytes",
        result.report.status,
        result.report.files,
        result.report.upstream_payload_bytes,
        result.report.local_payload_bytes
    ))?;
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
    f.try_lock().map_err(|_| {
        crate::Error::new(
            crate::ErrorKind::Locked,
            "another sync owns this workspace, or file locking is unavailable",
        )
    })?;
    Ok(f)
}

/// Synchronize with an observer, independent heartbeat and cooperative cancellation.
/// # Errors
/// Returns typed sync/refusal/cancellation errors; initialized frozen progress supports explicit resume.
pub fn run_with_control(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    resume: bool,
    source: &impl Source,
    control: &crate::events::Control,
) -> Result<SyncResult> {
    control.run("sync", || run(config, previous, output, resume, source))
}
/// Explicit destructive restart with progress/cancel; discarded old state is not backed up.
/// # Errors
/// Returns typed restart/sync errors. Cancellation does not undo an authorized reset.
pub fn run_forced_with_control(
    config: &Config,
    previous: Option<&Path>,
    output: &Path,
    source: &impl Source,
    control: &crate::events::Control,
) -> Result<SyncResult> {
    control.run("sync force", || {
        run_forced(config, previous, output, source)
    })
}
