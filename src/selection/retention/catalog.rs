//! Read-only local discovery and directory-bound retention planning.
use super::*;
use crate::selection::{collection::ToolchainRecord, SelectionSnapshot};
use sha2::{Digest, Sha256};
use std::time::UNIX_EPOCH;

/// Display metadata from a completed inventory marker; payload verification is separate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GenerationInfo {
    /// Absolute schema-2/3 snapshot root.
    pub path: PathBuf,
    /// Inventory schema, not an event protocol version.
    pub schema: u32,
    /// Inventory SHA-256 used to identify paused-run dependencies.
    pub inventory_sha256: String,
    /// Recorded payload lengths plus the inventory length; not physical disk usage.
    pub recorded_bytes: u64,
    /// Inventory last-write timestamp in nanoseconds since Unix epoch; used only for ordering.
    pub inventory_modified_ns: u64,
    /// Preserved toolchain choices. Discovery does not verify their payload closure.
    pub toolchains: Vec<ToolchainRecord>,
}
/// Conservative display state of a managed sync/fetch workspace.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Completed marker/report present; preview still verifies the generation.
    Completed,
    /// Paused/unlocked work; never a deletion candidate.
    InProgress,
    /// A writer owns a lock, or an existing lock cannot be safely inspected.
    Locked,
    /// State is missing/malformed; directory pruning is refused.
    Unknown,
}
/// One immediate managed sync/fetch workspace, retained intact by this API.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunInfo {
    /// Absolute workspace path, not its served mirror child.
    pub path: PathBuf,
    /// `sync` or `fetch`.
    pub kind: String,
    /// Conservative state; not a resume-compatibility guarantee.
    pub state: RunState,
    /// Completed mirror child if its inventory can be parsed locally.
    pub mirror: Option<PathBuf>,
    /// Frozen previous-inventory identity when available in a sync state.
    pub previous_inventory_sha256: Option<String>,
}
/// A managed path that could not be classified safely.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CatalogIssue {
    /// Affected local path.
    pub path: PathBuf,
    /// Bounded display diagnostic. No network credentials are read.
    pub message: String,
}
/// Serializable listing of immediate local generations and managed workspaces.
///
/// No recursive discovery of arbitrary user directories, downloads or lock-file creation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Catalog {
    /// Explicit directory selected by the operator.
    pub directory: PathBuf,
    /// Generations newest inventory timestamp first, then ascending path on ties.
    pub generations: Vec<GenerationInfo>,
    /// Sync/fetch runs, including paused and locked runs.
    pub runs: Vec<RunInfo>,
    /// Unrecognized immediate entries; never deletion candidates.
    pub unmanaged: Vec<PathBuf>,
    /// Unsafe managed entries; preview refuses these instead of guessing.
    pub issues: Vec<CatalogIssue>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct Binding {
    pub(super) directory: PathBuf,
    pub(super) next_previous: PathBuf,
    pub(super) fingerprint: String,
    pub(super) runs: Vec<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncState {
    schema: u32,
    config_sha256: String,
    previous_sha256: Option<String>,
    plans: Vec<String>,
}
fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("expected an unlinked regular file".into());
    }
    crate::selection::read_file(path, limit)
}
fn busy(workspace: &Path, owned: &BTreeSet<PathBuf>) -> bool {
    for name in [".sync.lock", ".fetch.lock"] {
        let path = workspace.join(name);
        if owned.contains(&path) {
            continue;
        }
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() == 0 => (),
            _ => return true,
        }
        let Ok(file) = File::open(&path) else {
            return true;
        };
        if file.try_lock_shared().is_err() {
            return true;
        }
    }
    false
}
fn generation(path: &Path) -> Result<GenerationInfo> {
    let path = real_directory(path)?;
    let raw = read_regular(&path.join("snapshot.toml"), 1024 * 1024)?;
    let text = std::str::from_utf8(&raw)?;
    let value: toml::Value = toml::from_str(text)?;
    let (schema, toolchains, files) = match value.get("schema").and_then(toml::Value::as_integer) {
        Some(2) => {
            let s: SelectionSnapshot = toml::from_str(text)?;
            (
                2,
                vec![ToolchainRecord {
                    selection: s.selection,
                    release: s.release,
                    date: s.date,
                    pinned_toolchain: s.pinned_toolchain,
                }],
                s.files,
            )
        }
        Some(3) => {
            let s: collection::CollectionSnapshot = toml::from_str(text)?;
            (3, s.toolchains, s.files)
        }
        _ => return Err("discovery accepts only schema-2/3 inventories".into()),
    };
    if toolchains.is_empty() || toolchains.len() > 256 || files.is_empty() || files.len() > 4096 {
        return Err("inventory display bounds exceeded".into());
    }
    let recorded_bytes = files.iter().try_fold(raw.len() as u64, |n, f| {
        n.checked_add(f.bytes).ok_or("inventory size overflow")
    })?;
    let time = fs::metadata(path.join("snapshot.toml"))?
        .modified()?
        .duration_since(UNIX_EPOCH)?;
    let inventory_modified_ns = u64::try_from(time.as_nanos())?;
    Ok(GenerationInfo {
        path,
        schema,
        inventory_sha256: sha256(&raw),
        recorded_bytes,
        inventory_modified_ns,
        toolchains,
    })
}
fn run(workspace: &Path, owned: &BTreeSet<PathBuf>) -> Result<Option<RunInfo>> {
    let sync = present(&workspace.join(".sync"))? || present(&workspace.join(".sync.lock"))?;
    let fetch = present(&workspace.join(".download"))? || present(&workspace.join(".fetch.lock"))?;
    if !sync && !fetch {
        return Ok(None);
    }
    let mut info = RunInfo {
        path: workspace.to_path_buf(),
        kind: if sync { "sync" } else { "fetch" }.into(),
        state: RunState::InProgress,
        mirror: None,
        previous_inventory_sha256: None,
    };
    if busy(workspace, owned) {
        info.state = RunState::Locked;
        return Ok(Some(info));
    }
    if sync && fetch {
        info.state = RunState::Unknown;
        return Ok(Some(info));
    }
    if sync {
        real_directory(&workspace.join(".sync"))?;
        let state: SyncState = toml::from_str(std::str::from_utf8(&read_regular(
            &workspace.join(".sync/state.toml"),
            64 * 1024,
        )?)?)?;
        if state.schema != 1 || state.plans.is_empty() || state.plans.len() > 32 {
            return Err("invalid sync state schema/plan count".into());
        }
        crate::valid_hash(&state.config_sha256)?;
        for hash in &state.plans {
            crate::valid_hash(hash)?;
        }
        if let Some(hash) = &state.previous_sha256 {
            crate::valid_hash(hash)?;
        }
        info.previous_inventory_sha256 = state.previous_sha256;
    }
    if present(&workspace.join("mirror/snapshot.toml"))? {
        info.mirror = Some(generation(&workspace.join("mirror"))?.path);
        if sync {
            if present(&workspace.join(".sync/report.toml"))? {
                let report: toml::Value = toml::from_str(std::str::from_utf8(&read_regular(
                    &workspace.join(".sync/report.toml"),
                    1024 * 1024,
                )?)?)?;
                if matches!(
                    report.get("status").and_then(toml::Value::as_str),
                    Some("created" | "validated")
                ) && !present(&workspace.join(".sync/tree"))?
                    && !present(&workspace.join(".sync/report.tmp"))?
                {
                    info.state = RunState::Completed;
                }
            }
        } else {
            info.state = RunState::Completed;
        }
    }
    Ok(Some(info))
}
/// List immediate `DIR/mirror` generations, direct schema-2/3 roots and sync/fetch runs.
///
/// Reads bounded inventory/state/report metadata only, never payload archives.
/// Existing lock probing is transient/nonblocking and creates no files. Unknown/locked runs
/// are displayed conservatively. Newest means inventory mtime, not active routing.
///
/// # Errors
/// Refuses a linked/missing directory, more than 128 immediate entries or local I/O failures.
pub fn discover(directory: &Path) -> Result<Catalog> {
    discover_owned(directory, &BTreeSet::new())
}
fn discover_owned(directory: &Path, owned: &BTreeSet<PathBuf>) -> Result<Catalog> {
    let directory = real_directory(directory)?;
    if private_path(&directory) {
        return Err("select a generation directory outside private working state".into());
    }
    for marker in [".sync", ".download", ".sync.lock", ".fetch.lock"] {
        if present(&directory.join(marker))? {
            return Err(
                "select the parent directory containing runs, not a run workspace itself".into(),
            );
        }
    }
    let mut result = Catalog {
        directory: directory.clone(),
        generations: vec![],
        runs: vec![],
        unmanaged: vec![],
        issues: vec![],
    };
    let mut entries = fs::read_dir(&directory)?
        .map(|e| Ok(e?.path()))
        .collect::<Result<Vec<_>>>()?;
    entries.sort();
    if entries.len() > 128 {
        return Err("discovery accepts at most 128 immediate entries".into());
    }
    for path in entries {
        let meta = fs::symlink_metadata(&path)?;
        if path
            .file_name()
            .is_some_and(|n| n == ".rust-mirror-prune.lock")
            && meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.len() == 0
        {
            continue;
        }
        if !meta.is_dir() || meta.file_type().is_symlink() {
            result.unmanaged.push(path);
            continue;
        }
        let path = real_directory(&path)?;
        match run(&path, owned) {
            Ok(Some(info)) => {
                if info.state == RunState::Locked {
                    result.runs.push(info);
                    continue;
                }
                if let Some(root) = &info.mirror {
                    match generation(root) {
                        Ok(g) => result.generations.push(g),
                        Err(e) => result.issues.push(CatalogIssue {
                            path: root.clone(),
                            message: e.to_string(),
                        }),
                    }
                }
                result.runs.push(info);
            }
            Err(e) => {
                result.issues.push(CatalogIssue {
                    path: path.clone(),
                    message: e.to_string(),
                });
                result.runs.push(RunInfo {
                    path,
                    kind: "sync/fetch".into(),
                    state: RunState::Unknown,
                    mirror: None,
                    previous_inventory_sha256: None,
                });
            }
            Ok(None) => {
                let root = if present(&path.join("snapshot.toml"))? {
                    path.clone()
                } else {
                    path.join("mirror")
                };
                if present(&root.join("snapshot.toml"))? {
                    match generation(&root) {
                        Ok(g) => result.generations.push(g),
                        Err(e) => result.issues.push(CatalogIssue {
                            path: root,
                            message: e.to_string(),
                        }),
                    }
                } else {
                    result.unmanaged.push(path);
                }
            }
        }
    }
    result.generations.sort_by(|a, b| {
        b.inventory_modified_ns
            .cmp(&a.inventory_modified_ns)
            .then(a.path.cmp(&b.path))
    });
    Ok(result)
}
// Hash every managed workspace entry, including private state/copies. Unmanaged
// immediate entries are bound by names/type only and are never traversed/deleted.
fn fingerprint(catalog: &Catalog) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(toml::to_string(catalog)?.as_bytes());
    for path in &catalog.unmanaged {
        let meta = fs::symlink_metadata(path)?;
        hash.update(path.to_str().ok_or("non-UTF-8 unmanaged entry")?.as_bytes());
        hash.update([
            u8::from(meta.is_dir()),
            u8::from(meta.is_file()),
            u8::from(meta.file_type().is_symlink()),
        ]);
        hash.update(meta.len().to_le_bytes());
        hash.update(
            meta.modified()?
                .duration_since(UNIX_EPOCH)?
                .as_nanos()
                .to_le_bytes(),
        );
    }
    let roots: BTreeSet<_> = catalog
        .runs
        .iter()
        .map(|r| r.path.clone())
        .chain(catalog.generations.iter().map(|g| {
            if g.path.file_name().is_some_and(|n| n == "mirror")
                && g.path.parent() != Some(catalog.directory.as_path())
            {
                g.path.parent().unwrap().to_path_buf()
            } else {
                g.path.clone()
            }
        }))
        .collect();
    let mut stack: Vec<_> = roots.into_iter().collect();
    let mut count = 0usize;
    let mut bytes = 0u64;
    while let Some(path) = stack.pop() {
        count += 1;
        if count > 300_000 {
            return Err("directory preview exceeds 300000 entries".into());
        }
        let meta = fs::symlink_metadata(&path)?;
        if path
            .file_name()
            .is_some_and(|n| n == ".rust-mirror-prune.lock")
            && meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.len() == 0
        {
            continue;
        }
        let relative = path.strip_prefix(&catalog.directory)?;
        if relative.components().count() > 64 || meta.file_type().is_symlink() {
            return Err("directory preview refuses links/excessive depth".into());
        }
        hash.update(
            relative
                .to_str()
                .ok_or("non-UTF-8 directory entry")?
                .as_bytes(),
        );
        hash.update([0]);
        hash.update([u8::from(meta.permissions().readonly())]);
        if let Ok(created) = meta
            .created()
            .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(std::io::Error::other))
        {
            hash.update(created.as_nanos().to_le_bytes());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            hash.update(meta.mode().to_le_bytes());
            hash.update(meta.uid().to_le_bytes());
            hash.update(meta.gid().to_le_bytes());
        }
        if meta.is_dir() {
            hash.update(b"directory");
            let mut children = fs::read_dir(&path)?
                .map(|e| Ok(e?.path()))
                .collect::<Result<Vec<_>>>()?;
            children.sort();
            stack.extend(children);
        } else if meta.is_file() {
            bytes = bytes
                .checked_add(meta.len())
                .ok_or("directory size overflow")?;
            if bytes > 100 * 1024 * 1024 * 1024 {
                return Err("directory preview exceeds 100 GiB read budget".into());
            }
            let stamp = meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
            hash.update(stamp.to_le_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                hash.update(meta.dev().to_le_bytes());
                hash.update(meta.ino().to_le_bytes());
            }
            let (len, digest) = if meta.len() == 0 {
                (0, sha256(&[]))
            } else {
                resume::hash_file(&path, crate::ARTIFACT_LIMIT)?
            };
            hash.update(len.to_le_bytes());
            hash.update(digest.as_bytes());
        } else {
            return Err("directory preview refuses special files".into());
        }
    }
    Ok(hex::encode(hash.finalize()))
}
/// Discover and plan deletion, automatically protecting the exact next `--previous` root.
///
/// `next_previous` must be a listed completed root (use `SyncResult::mirror`, not
/// an mtime guess). Paused-run previous identities are protected too. Busy/unknown
/// runs, missing dependencies and changed directory contents refuse planning.
/// Only whole completed mirror roots are candidates; private run state is retained.
///
/// # Errors
/// Includes discovery/validation/byte bounds, missing anchor/dependencies and unsafe run states.
pub fn preview_directory(
    policy: &Policy,
    directory: &Path,
    next_previous: &Path,
    as_of: &str,
) -> Result<PrunePlan> {
    preview_owned(policy, directory, next_previous, as_of, &BTreeSet::new())
}
pub(super) fn preview_owned(
    policy: &Policy,
    directory: &Path,
    next_previous: &Path,
    as_of: &str,
    owned: &BTreeSet<PathBuf>,
) -> Result<PrunePlan> {
    let catalog = discover_owned(directory, owned)?;
    if !catalog.issues.is_empty()
        || catalog
            .runs
            .iter()
            .any(|r| matches!(r.state, RunState::Locked | RunState::Unknown))
    {
        return Err(
            "directory contains locked/unknown runs or invalid managed paths; inspect discovery"
                .into(),
        );
    }
    let before = fingerprint(&catalog)?;
    let anchor = real_directory(next_previous)?;
    if !catalog.generations.iter().any(|g| g.path == anchor) {
        return Err("next previous must be a discovered completed generation".into());
    }
    let mut protected = BTreeSet::from([anchor.clone()]);
    for run in catalog
        .runs
        .iter()
        .filter(|r| r.state == RunState::InProgress)
    {
        if let Some(identity) = &run.previous_inventory_sha256 {
            let found: Vec<_> = catalog
                .generations
                .iter()
                .filter(|g| &g.inventory_sha256 == identity)
                .collect();
            if found.is_empty() {
                return Err("paused run previous generation is missing from discovery".into());
            }
            protected.extend(found.into_iter().map(|g| g.path.clone()));
        }
        if let Some(root) = &run.mirror {
            protected.insert(root.clone());
        }
    }
    let roots: Vec<_> = catalog.generations.iter().map(|g| g.path.clone()).collect();
    let mut plan = super::preview_inner(policy, &roots, as_of, &protected, owned, true)?;
    let after = discover_owned(&catalog.directory, owned)?;
    if after != catalog || fingerprint(&after)? != before {
        return Err("directory changed while preparing preview".into());
    }
    plan.catalog = Some(catalog.clone());
    plan.binding = Some(Binding {
        directory: catalog.directory,
        next_previous: anchor,
        fingerprint: before,
        runs: catalog.runs.into_iter().map(|r| r.path).collect(),
    });
    Ok(plan)
}
pub(super) fn check_roots(
    roots: &[PathBuf],
    owned: &BTreeSet<PathBuf>,
    allow_paused: bool,
) -> Result<()> {
    for root in roots {
        if private_path(root) {
            return Err("pruning private sync/download working state is not supported".into());
        }
        if let Some(parent) = root.parent() {
            if let Some(info) = run(parent, owned)? {
                if info.state == RunState::Locked
                    || info.state == RunState::Unknown
                    || (!allow_paused && info.state == RunState::InProgress)
                {
                    return Err("candidate belongs to an in-progress, locked or unknown run".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn workspace_paused(parent: &Path, owned: &BTreeSet<PathBuf>) -> Result<bool> {
    match run(parent, owned)? {
        Some(info) if matches!(info.state, RunState::Locked | RunState::Unknown) => {
            Err("workspace is locked or unknown".into())
        }
        Some(info) => Ok(info.state == RunState::InProgress),
        None => Ok(false),
    }
}

fn private_path(path: &Path) -> bool {
    path.components().any(|part| {
        if let Component::Normal(name) = part {
            name.to_str().is_some_and(|s| {
                if cfg!(windows) {
                    s.eq_ignore_ascii_case(".sync") || s.eq_ignore_ascii_case(".download")
                } else {
                    s == ".sync" || s == ".download"
                }
            })
        } else {
            false
        }
    })
}
