//! Private, locked crate transfers; completed immutable schema-4/5 output is exposed only after verification.
use super::*;
use crate::{resume, SnapshotFile, Transfer};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

/// Options for a new or explicitly resumed managed crate workspace.
#[derive(Clone, Debug)]
pub struct FetchOptions {
    /// Workspace path; its completed served tree is `mirror/`.
    pub output: PathBuf,
    /// Permit only matching initialized managed work. Never overwrite a completed tree.
    pub resume: bool,
    /// Validated previous completed generation when the frozen plan references one.
    pub previous: Option<PathBuf>,
}
/// Completed crate generation, with exact paths/hashes rather than estimates.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    /// Inventory schema 5 for plan 2, or legacy schema 4 for plan 1. Older readers refuse new schemas.
    pub schema: u32,
    /// Exactly `crates`.
    pub kind: String,
    /// Complete nonsecret frozen provenance and approval identity.
    pub plan: FrozenPlan,
    /// Served files, excluding this inventory marker itself.
    pub files: Vec<SnapshotFile>,
}
/// Successful local fetch; no publication or activation is implied.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FetchReport {
    /// Completed served mirror path.
    pub mirror: PathBuf,
    /// Exact approved plan identity.
    pub plan_id: String,
    /// SHA-256 of the completed inventory marker.
    pub inventory_sha256: String,
    /// Newly accepted archive bytes in this invocation.
    pub transferred_bytes: u64,
    /// Persisted accepted archive bytes across this run's resumes and retries.
    pub total_transferred_bytes: u64,
    /// Verified bytes reused from local completed files or validated prefixes.
    pub reused_bytes: u64,
    /// Number of fully verified archives, not metadata files.
    pub files_completed: usize,
    /// Planned archive count.
    pub files_total: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
    plan_id: String,
    approved_id: String,
    phase: String,
    transferred_bytes: u64,
    state_sha256: String,
}
struct Session {
    root: PathBuf,
    state: Mutex<State>,
    _lock: File,
}
fn regular(path: &Path) -> Result<Option<u64>> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(Some(m.len())),
        Ok(_) => Err(invalid("managed file must not be a link or directory")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn real_directory(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(invalid("managed directory must not be a link"));
    }
    Ok(())
}
fn token(row: &VersionRow) -> String {
    crate::sha256(format!("{}\0{}", row.name, row.version).as_bytes())
}
fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if regular(path)?.ok_or_else(|| invalid("missing managed metadata"))? > limit {
        return Err(budget("managed metadata exceeds bound"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(budget("managed metadata grew beyond bound"));
    }
    Ok(bytes)
}
fn state_hash(state: &State) -> Result<String> {
    let mut value = state.clone();
    value.state_sha256.clear();
    Ok(crate::sha256(&serde_json::to_vec(&value)?))
}
fn atomic_state(root: &Path, state: &State) -> Result<()> {
    let mut state = state.clone();
    state.state_sha256 = state_hash(&state)?;
    let next = root.join(".download/state.next");
    regular(&next)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&next)?;
    file.write_all(&serde_json::to_vec(&state)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(next, root.join(".download/state.json"))?;
    Ok(())
}
impl Session {
    fn open(plan: &FrozenPlan, options: &FetchOptions, approval: &Approval) -> Result<Self> {
        if approval.plan_id != plan.plan_id {
            return Err(invalid("approval does not match this frozen preview"));
        }
        if !options.resume {
            fs::create_dir(&options.output)?;
        }
        real_directory(&options.output)?;
        for entry in fs::read_dir(&options.output)? {
            let entry = entry?;
            if ![
                ".crates.lock",
                ".download",
                "plan.json",
                "mirror",
                ".rust-mirror-prune.lock",
            ]
            .iter()
            .any(|n| entry.file_name() == *n)
            {
                return Err(invalid("crate output contains unrelated files"));
            }
        }
        let lockpath = options.output.join(".crates.lock");
        if regular(&lockpath)?.is_some_and(|n| n != 0) {
            return Err(Error::new(ErrorKind::Locked, "invalid crate writer lock").into());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lockpath)?;
        lock.try_lock().map_err(|_| {
            Error::new(
                ErrorKind::Locked,
                "another crate writer owns this workspace",
            )
        })?;
        let saved = options.output.join("plan.json");
        if regular(&saved)?.is_some() {
            if FrozenPlan::load(&saved)?.plan_id != plan.plan_id {
                return Err(Error::new(
                    ErrorKind::ResumeMismatch,
                    "workspace belongs to another frozen plan",
                )
                .into());
            }
        } else {
            plan.save_new(&saved)?;
        }
        let private = options.output.join(".download");
        if !private.exists() {
            fs::create_dir(&private)?;
        }
        real_directory(&private)?;
        let state_path = private.join("state.json");
        let state = if regular(&state_path)?.is_some() {
            let state: State = serde_json::from_slice(&bounded_read(&state_path, 16384)?)
                .map_err(|_| invalid("invalid crate resume receipt"))?;
            if state_hash(&state)? != state.state_sha256 {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "crate resume receipt checksum changed",
                )
                .into());
            }
            if state.schema != 1
                || state.plan_id != plan.plan_id
                || state.approved_id != approval.plan_id
                || !matches!(
                    state.phase.as_str(),
                    "fetching" | "assembling" | "completed"
                )
            {
                return Err(
                    Error::new(ErrorKind::ResumeMismatch, "crate resume receipt mismatch").into(),
                );
            }
            state
        } else {
            if private.join("files").exists()
                && fs::read_dir(private.join("files"))?.next().is_some()
            {
                return Err(Error::new(
                    ErrorKind::ResumeMismatch,
                    "missing receipt for existing crate payload state",
                )
                .into());
            }
            State {
                schema: 1,
                plan_id: plan.plan_id.clone(),
                approved_id: approval.plan_id.clone(),
                phase: "fetching".into(),
                transferred_bytes: 0,
                state_sha256: String::new(),
            }
        };
        if state.transferred_bytes > plan.catalog.crate_policy.max_transfer_bytes {
            return Err(budget(
                "persisted transfer counter exceeds this plan's budget",
            ));
        }
        let files = private.join("files");
        if !files.exists() {
            fs::create_dir(&files)?;
        }
        real_directory(&files)?;
        let session = Self {
            root: options.output.clone(),
            state: Mutex::new(state),
            _lock: lock,
        };
        session.validate_paths(plan)?;
        atomic_state(
            &session.root,
            &session.state.lock().unwrap_or_else(|p| p.into_inner()),
        )?;
        Ok(session)
    }
    fn validate_paths(&self, plan: &FrozenPlan) -> Result<()> {
        validate_paths(&self.root, plan)
    }
    fn change_phase(&self, phase: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.phase = phase.into();
        atomic_state(&self.root, &state)
    }
}
struct Shared<'a> {
    session: &'a Session,
    plan: &'a FrozenPlan,
    previous: Option<&'a Path>,
    stop: AtomicBool,
    completed: AtomicUsize,
    reused: Mutex<u64>,
}
impl Shared<'_> {
    fn summary(&self) -> Result<()> {
        if let Some(control) = events::current() {
            let transferred = self
                .session
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .transferred_bytes;
            let reused = *self.reused.lock().unwrap_or_else(|p| p.into_inner());
            control.emit(events::EventKind::CrateSummary {
                files_completed: self.completed.load(Ordering::SeqCst) as u64,
                files_total: Some(self.plan.versions.len() as u64),
                transferred_bytes: transferred,
                reused_bytes: reused,
            })?;
        }
        Ok(())
    }
    fn accepted(&self, count: u64) -> Result<()> {
        events::check()?;
        if self.stop.load(Ordering::SeqCst) {
            return Err(Error::new(ErrorKind::Cancelled, "another crate operation failed").into());
        }
        let mut state = self.session.state.lock().unwrap_or_else(|p| p.into_inner());
        let total = state
            .transferred_bytes
            .checked_add(count)
            .ok_or_else(|| budget("transfer counter overflow"))?;
        if total > self.plan.catalog.crate_policy.max_transfer_bytes {
            return Err(budget("crate transfer budget exceeded"));
        }
        state.transferred_bytes = total;
        atomic_state(&self.session.root, &state)
    }
}
struct Charged<'a> {
    file: File,
    shared: &'a Shared<'a>,
    count: u64,
    path: String,
    offset: u64,
    limit: u64,
}
impl Write for Charged<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .offset
            .checked_add(self.count)
            .and_then(|n| n.checked_add(bytes.len() as u64))
            .is_none_or(|n| n > self.limit)
        {
            return Err(std::io::Error::other(Error::new(
                ErrorKind::Budget,
                "crate artifact exceeds its file budget",
            )));
        }
        self.shared
            .accepted(bytes.len() as u64)
            .map_err(std::io::Error::other)?;
        self.file.write_all(bytes)?;
        self.count += bytes.len() as u64;
        events::bytes(&self.path, self.offset + self.count).map_err(std::io::Error::other)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
fn verified(path: &Path, row: &VersionRow, limit: u64, label: &str) -> Result<Option<u64>> {
    let Some(size) = regular(path)? else {
        return Ok(None);
    };
    if size > limit {
        return Err(budget("cached crate exceeds its file budget"));
    }
    events::artifact(label, "verify", 0, Some(size))?;
    let (bytes, hash) = resume::hash_file_as(path, limit, label)?;
    if hash == row.cksum {
        events::complete(label, "verify", Some(bytes), true)?;
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}
fn download(row: &VersionRow, source: &dyn CrateSource, shared: &Shared<'_>) -> Result<(u64, u64)> {
    let path = archive_path(&row.name, &row.version)?;
    let id = token(row);
    let dir = shared.session.root.join(".download/files");
    let final_path = dir.join(format!("{id}.crate"));
    let partial = dir.join(format!("{id}.part"));
    let limit = shared.plan.catalog.crate_policy.max_file_bytes;
    if let Some(bytes) = verified(&final_path, row, limit, &path)? {
        events::artifact(&path, "reuse", 0, Some(bytes))?;
        events::complete(&path, "reuse", Some(bytes), true)?;
        return Ok((bytes, bytes));
    }
    if regular(&final_path)?.is_some() {
        fs::remove_file(&final_path)?;
    }
    if row.retained_from_previous {
        let previous = shared
            .previous
            .ok_or_else(|| invalid("retained archive requires its predecessor"))?;
        let bytes = copy_checked(&previous.join(&path), &final_path, &row.cksum, limit, &path)?;
        if Some(bytes) != row.expected_bytes {
            return Err(Error::new(ErrorKind::Integrity, "retained archive length changed").into());
        }
        events::artifact(&path, "reuse", 0, Some(bytes))?;
        events::complete(&path, "reuse", Some(bytes), true)?;
        return Ok((bytes, bytes));
    }
    if let Some(bytes) = verified(&partial, row, limit, &path)? {
        fs::rename(&partial, &final_path)?;
        return Ok((bytes, bytes));
    }
    let mut repaired = false;
    let attempts = source
        .attempts()
        .min(shared.plan.catalog.crate_network.attempts)
        .max(1);
    let mut attempt = 0;
    loop {
        events::check()?;
        let offset = regular(&partial)?.unwrap_or(0);
        if offset > limit {
            return Err(budget("partial crate exceeds budget"));
        }
        let operation = if offset > 0 { "resume" } else { "download" };
        events::artifact(&path, operation, offset, row.expected_bytes)?;
        let mut output = Charged {
            file: OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial)?,
            shared,
            count: 0,
            path: path.clone(),
            offset,
            limit,
        };
        attempt += 1;
        let result = source.archive(&row.name, &row.version, &mut output, offset, limit - offset);
        output.file.sync_all()?;
        drop(output);
        match result {
            Ok(Transfer::Restart) => {
                events::complete(&path, operation, Some(offset), false)?;
                if repaired {
                    return Err(Error::new(
                        ErrorKind::ResumeMismatch,
                        "upstream repeatedly refused the crate resume range",
                    )
                    .into());
                }
                events::artifact(&path, "restart", 0, row.expected_bytes)?;
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&partial)?;
                repaired = true;
            }
            Ok(Transfer::Complete) => {
                let bytes = regular(&partial)?.unwrap();
                events::complete(&path, operation, Some(bytes), false)?;
                if let Some(bytes) = verified(&partial, row, limit, &path)? {
                    fs::rename(&partial, &final_path)?;
                    return Ok((bytes, offset));
                }
                if offset > 0 && !repaired {
                    events::artifact(&path, "restart", 0, row.expected_bytes)?;
                    OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&partial)?;
                    repaired = true;
                    if attempt >= attempts {
                        return Err(Error::new(
                            ErrorKind::Integrity,
                            "resumed prefix checksum failed; no permitted restart attempt remains",
                        )
                        .into());
                    }
                    continue;
                }
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "crate checksum differs from frozen index cksum",
                )
                .into());
            }
            Err(error) => {
                if attempt >= attempts || !source.retry(error.as_ref(), attempt, &path)? {
                    return Err(error);
                }
            }
        }
        if attempt >= attempts {
            return Err(Error::new(ErrorKind::Network, "crate attempts exhausted").into());
        }
    }
}
fn copy_checked(
    source: &Path,
    destination: &Path,
    hash: &str,
    limit: u64,
    label: &str,
) -> Result<u64> {
    regular(destination)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let size = regular(source)?.ok_or_else(|| invalid("missing verified crate source"))?;
    events::artifact(label, "copy", 0, Some(size))?;
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0; 65536];
    loop {
        events::check()?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or_else(|| budget("copy count overflow"))?;
        if count > limit {
            return Err(budget("crate copy exceeds byte budget"));
        }
        output.write_all(&buffer[..n])?;
        digest.update(&buffer[..n]);
        events::bytes(label, count)?;
    }
    output.sync_all()?;
    if hex::encode(digest.finalize()) != hash {
        return Err(Error::new(
            ErrorKind::Integrity,
            "crate changed while copying into generation",
        )
        .into());
    }
    events::complete(label, "copy", Some(count), true)?;
    Ok(count)
}
impl Inventory {
    /// Verify full local schema-4/5 closure and all bytes. Does not publish or activate anything.
    pub fn load(root: &Path) -> Result<Self> {
        Self::read(root, true)
    }
    /// Read-only bounded metadata discovery; does not claim payload verification.
    pub fn inspect(root: &Path) -> Result<Self> {
        Self::read(root, false)
    }
    fn read(root: &Path, payloads: bool) -> Result<Self> {
        real_directory(root)?;
        let raw = bounded_read(&root.join("snapshot.toml"), PLAN_LIMIT)?;
        let inventory: Self = toml::from_str(std::str::from_utf8(&raw)?)
            .map_err(|_| invalid("invalid crate inventory"))?;
        if !matches!((inventory.schema, inventory.plan.schema), (4, 1) | (5, 2))
            || inventory.kind != "crates"
        {
            return Err(invalid("unsupported crate inventory kind/schema"));
        }
        if payloads {
            inventory.plan.validate()?;
        } else {
            inventory.plan.inspect_metadata()?;
        }
        let generated = generated_files(&inventory.plan)?;
        let archives: BTreeMap<_, _> = inventory
            .plan
            .versions
            .iter()
            .map(|r| Ok((archive_path(&r.name, &r.version)?, r)))
            .collect::<Result<_>>()?;
        if inventory.files.len() != generated.len() + archives.len() {
            return Err(invalid("crate inventory file count disagrees with closure"));
        }
        let mut names = BTreeSet::from([PathBuf::from("snapshot.toml")]);
        let mut total = raw.len() as u64;
        for file in &inventory.files {
            // Cargo archive names can contain '+' in validated SemVer build metadata.
            // Only exact paths derived by archive_path receive this exception.
            if !archives.contains_key(&file.path) {
                crate::safe_path(&file.path)?;
            }
            crate::valid_hash(&file.sha256)?;
            if !names.insert(PathBuf::from(&file.path))
                || file.bytes > inventory.plan.catalog.crate_policy.max_file_bytes
            {
                return Err(invalid("duplicate or over-budget crate inventory path"));
            }
            if let Some(row) = archives.get(&file.path) {
                if file.sha256 != row.cksum {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "archive inventory differs from upstream checksum",
                    )
                    .into());
                }
            } else if let Some(bytes) = generated.get(&file.path) {
                if file.sha256 != crate::sha256(bytes) || file.bytes != bytes.len() as u64 {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "generated sparse metadata differs from frozen records",
                    )
                    .into());
                }
            } else {
                return Err(invalid("crate inventory contains an unplanned path"));
            }
            total = total
                .checked_add(file.bytes)
                .ok_or_else(|| budget("generation byte count overflow"))?;
            if payloads {
                events::artifact(&file.path, "verify", 0, Some(file.bytes))?;
                if resume::hash_file_as(&root.join(&file.path), file.bytes, &file.path)?
                    != (file.bytes, file.sha256.clone())
                {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "completed crate generation changed",
                    )
                    .into());
                }
                events::complete(&file.path, "verify", Some(file.bytes), true)?;
            }
        }
        if total > inventory.plan.catalog.crate_policy.max_final_bytes
            || raw.len() as u64 > inventory.plan.catalog.crate_policy.max_file_bytes
        {
            return Err(budget("completed generation exceeds frozen budget"));
        }
        if payloads {
            resume::validate_tree(root, &names)?;
        }
        Ok(inventory)
    }
}
/// Fetch approved frozen input into a locked private workspace, preserving resumable state on failure.
pub fn fetch(
    plan: &FrozenPlan,
    source: &dyn CrateSource,
    options: &FetchOptions,
    approval: &Approval,
) -> Result<FetchReport> {
    plan.validate()?;
    if approval.plan_id != plan.plan_id {
        return Err(invalid(
            "whole-preview approval ID does not match frozen plan",
        ));
    }
    if plan.budgets.exceeds_final_budget
        || plan.budgets.exceeds_transfer_budget
        || !plan.budgets.oversized_files.is_empty()
    {
        return Err(budget("preview exceeds a known crate byte budget"));
    }
    match (&options.previous, &plan.previous_inventory_sha256) {
        (Some(previous), Some(expected)) => {
            let parent = options
                .output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let output = if options.output.try_exists()? {
                fs::canonicalize(&options.output)?
            } else {
                fs::canonicalize(parent)?.join(
                    options
                        .output
                        .file_name()
                        .ok_or_else(|| invalid("output needs a final directory name"))?,
                )
            };
            let predecessor = fs::canonicalize(previous)?;
            if output.starts_with(&predecessor) || predecessor.starts_with(&output) {
                return Err(invalid("crate output overlaps its previous generation"));
            }
            let inventory = Inventory::load(previous)?;
            let raw = bounded_read(&previous.join("snapshot.toml"), PLAN_LIMIT)?;
            if crate::sha256(&raw) != *expected {
                return Err(Error::new(
                    ErrorKind::ResumeMismatch,
                    "previous inventory differs from frozen plan",
                )
                .into());
            }
            retained::check_previous(plan, &inventory)?;
        }
        (None, None) => {}
        _ => {
            return Err(Error::new(
                ErrorKind::ResumeMismatch,
                "previous generation must match frozen plan",
            )
            .into())
        }
    }
    let session = Session::open(plan, options, approval)?;
    let initial = session
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .transferred_bytes;
    let mirror = session.root.join("mirror");
    if mirror.exists() {
        let inventory = Inventory::load(&mirror)?;
        if inventory.plan.plan_id != plan.plan_id {
            return Err(Error::new(
                ErrorKind::ResumeMismatch,
                "completed mirror belongs to another plan",
            )
            .into());
        }
        return Ok(FetchReport {
            mirror: mirror.clone(),
            plan_id: plan.plan_id.clone(),
            inventory_sha256: crate::sha256(&bounded_read(
                &mirror.join("snapshot.toml"),
                PLAN_LIMIT,
            )?),
            transferred_bytes: 0,
            total_transferred_bytes: initial,
            reused_bytes: inventory
                .files
                .iter()
                .filter(|f| f.path.ends_with(".crate"))
                .map(|f| f.bytes)
                .sum(),
            files_completed: plan.versions.len(),
            files_total: plan.versions.len(),
        });
    }
    session.change_phase("fetching")?;
    events::stage("download")?;
    let shared = Shared {
        session: &session,
        plan,
        previous: options.previous.as_deref(),
        stop: AtomicBool::new(false),
        completed: AtomicUsize::new(0),
        reused: Mutex::new(0),
    };
    let next = AtomicUsize::new(0);
    let error: Mutex<Option<Box<dyn std::error::Error + Send + Sync>>> = Mutex::new(None);
    let control = events::current();
    let concurrent = events::concurrent();
    std::thread::scope(|scope| {
        for _ in 0..plan
            .catalog
            .crate_network
            .concurrency
            .min(plan.versions.len())
        {
            let shared = &shared;
            let next = &next;
            let error = &error;
            let control = control.clone();
            scope.spawn(move || {
                let result = events::worker(control, || {
                    loop {
                        events::check()?;
                        if shared.stop.load(Ordering::SeqCst) {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::SeqCst);
                        let Some(row) = plan.versions.get(index) else {
                            break;
                        };
                        let (_, reused) = download(row, source, shared)?;
                        *shared.reused.lock().unwrap_or_else(|p| p.into_inner()) += reused;
                        shared.completed.fetch_add(1, Ordering::SeqCst);
                        shared.summary()?;
                    }
                    Ok(())
                });
                if let Err(failure) = result {
                    shared.stop.store(true, Ordering::SeqCst);
                    let mut slot = error.lock().unwrap_or_else(|p| p.into_inner());
                    if slot.is_none() {
                        *slot = Some(failure);
                    }
                }
            });
        }
    });
    drop(concurrent);
    if let Some(error) = error.into_inner().unwrap_or_else(|p| p.into_inner()) {
        return Err(error);
    }
    session.change_phase("assembling")?;
    events::stage("assemble")?;
    let tree = session.root.join(".download/tree");
    if !tree.exists() {
        fs::create_dir(&tree)?;
    }
    real_directory(&tree)?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for row in &plan.versions {
        let path = archive_path(&row.name, &row.version)?;
        let source = session
            .root
            .join(format!(".download/files/{}.crate", token(row)));
        let bytes = copy_checked(
            &source,
            &tree.join(&path),
            &row.cksum,
            plan.catalog.crate_policy.max_file_bytes,
            &path,
        )?;
        total = total
            .checked_add(bytes)
            .ok_or_else(|| budget("generation count overflow"))?;
        if total > plan.catalog.crate_policy.max_final_bytes {
            return Err(budget("crate generation exceeds final budget"));
        }
        files.push(SnapshotFile {
            path,
            sha256: row.cksum.clone(),
            bytes,
        });
    }
    for (path, data) in generated_files(plan)? {
        if data.len() as u64 > plan.catalog.crate_policy.max_file_bytes {
            return Err(budget("generated sparse file exceeds file budget"));
        }
        let destination = tree.join(&path);
        regular(&destination)?;
        fs::create_dir_all(destination.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&destination)?;
        file.write_all(&data)?;
        file.sync_all()?;
        total = total
            .checked_add(data.len() as u64)
            .ok_or_else(|| budget("generation metadata overflow"))?;
        files.push(SnapshotFile {
            path,
            sha256: crate::sha256(&data),
            bytes: data.len() as u64,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let inventory = Inventory {
        schema: if plan.schema == 1 { 4 } else { 5 },
        kind: "crates".into(),
        plan: plan.clone(),
        files,
    };
    let raw = toml::to_string(&inventory)?.into_bytes();
    if raw.len() as u64 > PLAN_LIMIT
        || raw.len() as u64 > plan.catalog.crate_policy.max_file_bytes
        || total
            .checked_add(raw.len() as u64)
            .is_none_or(|n| n > plan.catalog.crate_policy.max_final_bytes)
    {
        return Err(budget("final crate inventory exceeds generation budget"));
    }
    let marker = tree.join("snapshot.toml");
    regular(&marker)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&marker)?;
    file.write_all(&raw)?;
    file.sync_all()?;
    drop(file);
    Inventory::load(&tree)?;
    events::check()?;
    if mirror.exists() {
        return Err(Error::new(
            ErrorKind::OutputExists,
            "refusing to overwrite a completed mirror",
        )
        .into());
    }
    fs::rename(tree, &mirror)?;
    session.change_phase("completed")?;
    let transferred = session
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .transferred_bytes;
    let reused = *shared.reused.lock().unwrap_or_else(|p| p.into_inner());
    Ok(FetchReport {
        mirror,
        plan_id: plan.plan_id.clone(),
        inventory_sha256: crate::sha256(&raw),
        transferred_bytes: transferred - initial,
        total_transferred_bytes: transferred,
        reused_bytes: reused,
        files_completed: plan.versions.len(),
        files_total: plan.versions.len(),
    })
}
/// Controlled approved fetch; worker threads share the observer/token while one parent heartbeat remains independent.
pub fn fetch_with_control(
    plan: &FrozenPlan,
    source: &dyn CrateSource,
    options: &FetchOptions,
    approval: &Approval,
    control: &events::Control,
) -> Result<FetchReport> {
    control.run("crates-fetch", || fetch(plan, source, options, approval))
}

fn validate_paths(root: &Path, plan: &FrozenPlan) -> Result<()> {
    if regular(&root.join(".rust-mirror-prune.lock"))?.is_some_and(|bytes| bytes != 0) {
        return Err(invalid("invalid shared prune lock"));
    }
    let mut allowed = BTreeSet::from([
        PathBuf::from(".crates.lock"),
        PathBuf::from(".rust-mirror-prune.lock"),
        PathBuf::from("plan.json"),
        PathBuf::from(".download/state.json"),
        PathBuf::from(".download/state.next"),
    ]);
    for row in &plan.versions {
        let id = token(row);
        allowed.insert(PathBuf::from(format!(".download/files/{id}.part")));
        allowed.insert(PathBuf::from(format!(".download/files/{id}.crate")));
    }
    let mut served: BTreeSet<_> = generated_files(plan)?.keys().cloned().collect();
    for row in &plan.versions {
        served.insert(archive_path(&row.name, &row.version)?);
    }
    served.insert("snapshot.toml".into());
    for path in served {
        allowed.insert(PathBuf::from(format!(".download/tree/{path}")));
        allowed.insert(PathBuf::from(format!("mirror/{path}")));
    }
    resume::validate_tree(root, &allowed)
}

pub(crate) fn inspect_run_metadata(root: &Path, plan: &FrozenPlan) -> Result<bool> {
    let mut completed = false;
    validate_paths(root, plan)?;
    if root.join(".download").try_exists()? {
        let state: State =
            serde_json::from_slice(&bounded_read(&root.join(".download/state.json"), 16384)?)
                .map_err(|_| invalid("invalid crate run receipt"))?;
        if state.schema != 1
            || state.plan_id != plan.plan_id
            || state.approved_id != plan.plan_id
            || state.state_sha256 != state_hash(&state)?
            || !matches!(
                state.phase.as_str(),
                "fetching" | "assembling" | "completed"
            )
            || state.transferred_bytes > plan.catalog.crate_policy.max_transfer_bytes
        {
            return Err(invalid("crate run receipt disagrees with its preview"));
        }
        completed = state.phase == "completed";
    }
    if root.join("mirror/snapshot.toml").try_exists()?
        && Inventory::inspect(&root.join("mirror"))?.plan.plan_id != plan.plan_id
    {
        return Err(invalid("crate run mirror disagrees with its preview"));
    }
    Ok(completed)
}
