//! Offline, generation-level retention with an inspectable deletion plan.
//!
//! Supply every candidate in newest-first order, current generation first. Whole
//! generations are kept when needed for declared protection; their files are never
//! compacted in place. No network operations or Nexus deletion are performed.
mod catalog;
use super::{collection, valid_date, Toolchain};
use crate::{resume, sha256, Result};
pub use catalog::{
    discover, preview_directory, Catalog, CatalogIssue, GenerationInfo, RunInfo, RunState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::{Component, Path, PathBuf},
};

/// Explicit offline-client protection, configured in `[retention]`.
///
/// The four original fields are required; keep-newest/generation pins are optional.
/// Omitting the entire table disables pruning.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Full numeric or dated-channel selectors, never moving aliases. Maximum 256.
    pub pinned_releases: Vec<String>,
    /// Number of distinct newest numeric stable releases to protect (0–256).
    pub latest_stable: usize,
    /// Calendar days including `as_of` to protect nightly records (0–36,600).
    pub nightly_days: u32,
    /// Number of preceding generations to keep in addition to current (0–31).
    pub previous_generations: usize,
    #[serde(default)]
    /// Minimum newest roots retained, including current (0–32); defaults to zero.
    pub keep_newest: usize,
    #[serde(default)]
    /// Explicit completed generation paths to retain (at most 32); defaults to empty.
    pub pinned_generations: Vec<PathBuf>,
}
impl Policy {
    /// Validate bounds and unique immutable selectors without reading any files.
    ///
    /// # Errors
    /// Refuses moving aliases, duplicates, malformed selectors or excessive counts.
    pub fn validate(&self) -> Result<()> {
        if self.pinned_releases.len() > 256
            || self.latest_stable > 256
            || self.nightly_days > 36_600
            || self.previous_generations > 31
            || self.keep_newest > 32
            || self.pinned_generations.len() > 32
        {
            return Err(crate::Error::new(
                crate::ErrorKind::Budget,
                "retention policy exceeds bounds",
            )
            .into());
        }
        let mut seen = BTreeSet::new();
        for pin in &self.pinned_releases {
            let spec = Toolchain::parse(pin)?;
            if (spec.date.is_none()
                && ["stable", "beta", "nightly"].contains(&spec.channel.as_str()))
                || !seen.insert(pin)
            {
                return Err(
                    "retention pins must be unique full numeric or dated-channel selectors".into(),
                );
            }
        }
        Ok(())
    }
}

/// One exact regular file approved for deletion; bytes are logical, not disk blocks.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Deletion {
    /// Absolute path within a validated generation.
    pub path: PathBuf,
    /// Verified file length, including `snapshot.toml` for inventory entries.
    pub bytes: u64,
    /// Verified SHA-256 identity used when revalidating a preview.
    pub sha256: String,
}
/// One whole completed folder approved for deletion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct FolderDeletion {
    /// Absolute snapshot root; never a private sync/fetch workspace.
    pub path: PathBuf,
    /// Logical bytes including its inventory marker.
    pub bytes: u64,
}
/// A generation protected by current/rollback order or declared toolchain coverage.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeptGeneration {
    /// Absolute completed snapshot root.
    pub path: PathBuf,
    /// Deterministic human-readable reasons suitable for a GUI.
    pub reasons: Vec<String>,
}
/// Structured dry-run output. Apply always reconstructs this plan before deletion.
///
/// Keep in memory or round-trip with Serde between identical library builds.
/// Deserialization grants no authority: apply bounds and reconstructs every field.
/// A CLI invocation recomputes its preview before applying.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PrunePlan {
    /// Plan representation revision; currently 1, independent of any event protocol.
    pub schema: u32,
    /// Calendar reference date used for nightly policy, independent of filesystem times.
    pub as_of: String,
    /// Whole snapshot roots that would be removed; enclosing workspaces are preserved.
    pub generations: Vec<PathBuf>,
    /// Exact snapshot folders with their individual logical byte totals.
    pub folders: Vec<FolderDeletion>,
    /// Directory discovery snapshot, when planned by `preview_directory`.
    pub catalog: Option<Catalog>,
    /// Exact regular files to remove in execution order (inventory marker first).
    pub files: Vec<Deletion>,
    /// Sum of file lengths, not a promise about physical space reclaimed.
    pub total_bytes: u64,
    /// Protected generations and their reasons.
    pub kept: Vec<KeptGeneration>,
    policy: Policy,
    roots: Vec<PathBuf>,
    identities: Vec<String>,
    binding: Option<catalog::Binding>,
}
/// An apply failure after preflight, including any preceding successful deletions.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApplyFailure {
    /// File or directory whose removal failed.
    pub path: PathBuf,
    /// Local error description; no network credentials are involved.
    pub message: String,
}
/// Structured apply outcome. A failure stops processing; no rollback is promised.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApplyReport {
    /// Successfully unlinked regular files, in order.
    pub deleted_files: Vec<Deletion>,
    /// Logical bytes successfully unlinked.
    pub deleted_bytes: u64,
    /// Snapshot roots completely removed.
    pub removed_generations: Vec<PathBuf>,
    /// First failure, if any. Earlier deletions remain applied.
    pub failure: Option<ApplyFailure>,
}

// Day ordinal; valid_date supplies the calendar bounds, including leap years.
fn day(date: &str) -> Result<i64> {
    if !valid_date(date) {
        return Err("retention requires a calendar date YYYY-MM-DD (year >= 2010)".into());
    }
    let y: i64 = date[..4].parse()?;
    let m: i64 = date[5..7].parse()?;
    let d: i64 = date[8..].parse()?;
    let y = y - i64::from(m <= 2);
    let era = y / 400;
    let yo = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    Ok(era * 146097 + yo * 365 + yo / 4 - yo / 100 + (153 * mp + 2) / 5 + d - 1)
}
fn numeric(value: &str) -> Option<(u64, u64, u64)> {
    crate::version(value).ok()?;
    let mut p = value.split('.');
    Some((
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    ))
}
// Reject links in every component before canonicalizing, including ancestors.
fn real_directory(path: &Path) -> Result<PathBuf> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("retention paths must not contain parent traversal".into());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut checked = PathBuf::new();
    for part in absolute.components() {
        if matches!(part, Component::ParentDir) {
            return Err("retention paths must not contain parent traversal".into());
        }
        checked.push(part.as_os_str());
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        let m = fs::symlink_metadata(&checked)?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err("retention paths must be real directories without links".into());
        }
    }
    let canonical = absolute.canonicalize()?;
    canonical
        .to_str()
        .ok_or("retention roots must be UTF-8 for structured reports")?;
    Ok(canonical)
}

fn retention_input(root: &Path) -> Result<collection::Input> {
    let raw = catalog::read_inventory(root)?;
    let value: toml::Value = toml::from_str(std::str::from_utf8(&raw)?)?;
    if !matches!(
        value.get("schema").and_then(toml::Value::as_integer),
        Some(4 | 5)
    ) {
        return collection::read_input(root);
    }
    let inventory = crate::crates::mirror::Inventory::load(root)?;
    let files = inventory
        .files
        .into_iter()
        .map(|f| {
            (
                f.path.clone(),
                collection::Entry {
                    source: root.join(&f.path),
                    sha256: f.sha256,
                    bytes: f.bytes,
                },
            )
        })
        .collect();
    Ok(collection::Input {
        rustup: String::new(),
        records: Vec::new(),
        files,
        identity: sha256(&raw),
    })
}

/// Validate all supplied schema-2/3 toolchain or schema-4 crate snapshots and compute a read-only preview.
///
/// `roots` contains 1–32 nonoverlapping completed mirror directories in explicitly
/// declared newest-first order, current first. Protection is evaluated only across
/// this set, never upstream. Every distinct protected toolchain record (including
/// platforms/components and bootstrap version) retains its newest exact copy.
/// Pins must exist. The current generation is always kept, even with zero counts.
/// Reads/hashes every input; creates no files/persistent locks or network requests.
/// Only existing writer locks are probed transiently.
///
/// # Errors
/// Refuses missing pins, invalid policy/date, duplicate/overlapping/linked roots,
/// invalid snapshot inventories/closures, unexpected files, corruption or I/O errors.
pub fn preview(policy: &Policy, roots: &[PathBuf], as_of: &str) -> Result<PrunePlan> {
    preview_inner(
        policy,
        roots,
        as_of,
        &BTreeSet::new(),
        &BTreeSet::new(),
        false,
    )
}
fn preview_inner(
    policy: &Policy,
    roots: &[PathBuf],
    as_of: &str,
    protected: &BTreeSet<PathBuf>,
    owned: &BTreeSet<PathBuf>,
    allow_paused: bool,
) -> Result<PrunePlan> {
    policy.validate()?;
    let today = day(as_of)?;
    if roots.is_empty() || roots.len() > 32 {
        return Err("retention requires 1-32 generations, newest first".into());
    }
    crate::events::stage("validate retention inputs")?;
    let roots = roots
        .iter()
        .map(|p| real_directory(p))
        .collect::<Result<Vec<_>>>()?;
    for (i, root) in roots.iter().enumerate() {
        if roots
            .iter()
            .skip(i + 1)
            .any(|r| r.starts_with(root) || root.starts_with(r))
        {
            return Err("retention generations overlap or are duplicated".into());
        }
    }
    catalog::check_roots(&roots, owned, allow_paused)?;
    let inputs = roots
        .iter()
        .map(|r| retention_input(r))
        .collect::<Result<Vec<_>>>()?;
    let mut stable = BTreeSet::new();
    for record in inputs.iter().flat_map(|i| &i.records) {
        let spec = Toolchain::parse(&record.selection.toolchain)?;
        if spec.channel == "stable" || numeric(&spec.channel).is_some() {
            stable.insert(
                numeric(&record.release).ok_or("invalid stable release in retention input")?,
            );
        }
        if day(&record.date)? > today {
            return Err("generation contains a manifest date after retention as_of".into());
        }
    }
    let stable: BTreeSet<_> = stable
        .into_iter()
        .rev()
        .take(policy.latest_stable)
        .collect();
    let mut reasons: Vec<BTreeSet<String>> = vec![BTreeSet::new(); roots.len()];
    reasons[0].insert("current generation".into());
    for reason in reasons.iter_mut().skip(1).take(policy.previous_generations) {
        reason.insert("rollback generation".into());
    }
    for reason in reasons.iter_mut().take(policy.keep_newest) {
        reason.insert("keep newest generation".into());
    }
    let pinned = policy
        .pinned_generations
        .iter()
        .map(|p| real_directory(p))
        .collect::<Result<BTreeSet<_>>>()?;
    if pinned.len() != policy.pinned_generations.len() || pinned.iter().any(|p| !roots.contains(p))
    {
        return Err("pinned generations must be unique supplied completed roots".into());
    }
    for (i, root) in roots.iter().enumerate() {
        if pinned.contains(root) {
            reasons[i].insert("pinned generation".into());
        }
        if protected.contains(root) {
            reasons[i].insert("next previous or paused-run dependency".into());
        }
    }
    let mut found = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for (i, input) in inputs.iter().enumerate() {
        for record in &input.records {
            let spec = Toolchain::parse(&record.selection.toolchain)?;
            let mut protection = Vec::new();
            for pin in &policy.pinned_releases {
                if pin == &record.pinned_toolchain
                    || (numeric(pin).is_some() && pin == &record.release)
                {
                    found.insert(pin.clone());
                    protection.push(format!("pinned {pin}"));
                }
            }
            if (spec.channel == "stable" || numeric(&spec.channel).is_some())
                && numeric(&record.release).is_some_and(|r| stable.contains(&r))
            {
                protection.push(format!("latest stable {}", record.release));
            }
            if spec.channel == "nightly"
                && today - day(&record.date)? < i64::from(policy.nightly_days)
            {
                protection.push(format!("nightly {}", record.date));
            }
            if !protection.is_empty() {
                // Names are administrative labels; every other selection field is coverage.
                let mut identity = record.clone();
                identity.selection.name.clear();
                let manifest = format!("dist/{}/channel-rust-{}.toml", record.date, spec.channel);
                let bootstrap_hashes: Vec<_> = record
                    .selection
                    .hosts
                    .iter()
                    .map(|host| {
                        input.files[&super::bootstrap_path(&input.rustup, host)]
                            .sha256
                            .as_str()
                    })
                    .collect();
                // Preserve optional manager/current and numeric lookup capabilities too.
                let minor = record
                    .release
                    .rsplit_once('.')
                    .map(|(p, _)| p)
                    .unwrap_or("");
                let endpoints: Vec<_> = input
                    .files
                    .iter()
                    .filter(|(p, _)| {
                        p.starts_with("rustup/")
                            || [record.release.as_str(), minor].iter().any(|v| {
                                let base = format!("dist/channel-rust-{v}.toml");
                                **p == base
                                    || **p == format!("{base}.sha256")
                                    || **p == format!("{base}.asc")
                            })
                    })
                    .map(|(p, e)| (p, &e.sha256))
                    .collect();
                let identity = format!(
                    "{}\n{}\n{}\n{:?}\n{:?}",
                    input.rustup,
                    toml::to_string(&identity)?,
                    input.files[&manifest].sha256,
                    bootstrap_hashes,
                    endpoints
                );
                if covered.insert(identity) {
                    reasons[i].extend(protection);
                }
            }
        }
    }
    if policy.pinned_releases.iter().any(|p| !found.contains(p)) {
        return Err("a pinned release is absent from the supplied generations".into());
    }
    let mut plan = PrunePlan {
        schema: 1,
        as_of: as_of.into(),
        generations: vec![],
        folders: vec![],
        catalog: None,
        binding: None,
        files: vec![],
        total_bytes: 0,
        kept: vec![],
        policy: policy.clone(),
        roots: roots.clone(),
        identities: inputs.iter().map(|i| i.identity.clone()).collect(),
    };
    for ((root, input), reason) in roots.iter().zip(inputs).zip(reasons) {
        if !reason.is_empty() {
            plan.kept.push(KeptGeneration {
                path: root.clone(),
                reasons: reason.into_iter().collect(),
            });
            continue;
        }
        plan.generations.push(root.clone());
        // Invalidate completed marker first; interrupted generations must not be published.
        let raw = catalog::read_inventory(root)?;
        plan.files.push(Deletion {
            path: root.join("snapshot.toml"),
            bytes: raw.len() as u64,
            sha256: sha256(&raw),
        });
        for (path, entry) in input.files {
            plan.files.push(Deletion {
                path: root.join(path),
                bytes: entry.bytes,
                sha256: entry.sha256,
            });
        }
    }
    for root in &plan.generations {
        let bytes = plan
            .files
            .iter()
            .filter(|f| f.path.starts_with(root))
            .try_fold(0u64, |n, f| {
                n.checked_add(f.bytes).ok_or_else(|| {
                    crate::Error::new(crate::ErrorKind::Budget, "folder size overflow")
                })
            })?;
        plan.folders.push(FolderDeletion {
            path: root.clone(),
            bytes,
        });
    }
    plan.total_bytes = plan.files.iter().try_fold(0u64, |n, f| {
        n.checked_add(f.bytes)
            .ok_or_else(|| crate::Error::new(crate::ErrorKind::Budget, "retention size overflow"))
    })?;
    Ok(plan)
}

fn lock_file(path: &Path, create: bool) -> Result<File> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() == 0 => (),
        Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => (),
        _ => return Err("retention lock must be an empty regular file".into()),
    }
    let f = File::options()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path)?;
    f.try_lock().map_err(|_| {
        crate::Error::new(
            crate::ErrorKind::Locked,
            "generation workspace busy, or filesystem locking unavailable",
        )
    })?;
    Ok(f)
}

/// Apply exactly a previously inspected preview, with explicit deletion authority.
///
/// `allow_delete` must be true. Locks each enclosing workspace (and any existing
/// sync/fetch locks), reconstructs the complete preview and refuses any change
/// before unlinking files. Only listed files and their empty parent directories
/// inside selected roots are removed; enclosing workspaces/locks are preserved.
/// Stop serving/using candidate generations and prevent external writers first:
/// advisory locks do not coordinate publishers, readers or hostile filesystem edits.
///
/// # Errors
/// Returns an error before deletion for missing authorization, locking/preflight
/// failure or a stale/modified plan. Once deletion starts, I/O failures are returned
/// in [`ApplyReport::failure`] with the successful prefix. There is no rollback;
/// an interrupted partial root requires manual inspection, not blind retry.
pub fn apply(plan: &PrunePlan, allow_delete: bool) -> Result<ApplyReport> {
    if !allow_delete {
        return Err("deletion is disabled; explicitly authorize apply".into());
    }
    plan.policy.validate()?;
    if plan.schema != 1
        || plan.roots.is_empty()
        || plan.roots.len() > 32
        || plan.identities.len() != plan.roots.len()
        || plan.generations.len() > 32
        || plan.folders.len() > 32
        || plan.kept.len() > 32
        || plan.files.len() > 32 * 4097
        || plan.binding.as_ref().is_some_and(|b| b.runs.len() > 128)
        || plan.catalog.as_ref().is_some_and(|c| {
            c.runs.len() > 128
                || c.generations.len() > 32
                || c.unmanaged.len() > 128
                || c.issues.len() > 128
        })
    {
        return Err("unsupported or excessive retention plan envelope".into());
    }
    let parents: BTreeSet<_> = plan
        .roots
        .iter()
        .map(|r| r.parent().ok_or("generation needs parent"))
        .collect::<std::result::Result<_, _>>()?;
    let mut locks = Vec::new();
    let mut owned = BTreeSet::new();
    for parent in parents {
        real_directory(parent)?;
        for name in [".sync.lock", ".fetch.lock", ".crates.lock"] {
            let p = parent.join(name);
            match fs::symlink_metadata(&p) {
                Ok(_) => {
                    locks.push(lock_file(&p, false)?);
                    owned.insert(p);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        if !catalog::workspace_paused(parent, &owned)? {
            locks.push(lock_file(&parent.join(".rust-mirror-prune.lock"), true)?);
        }
    }
    if let Some(binding) = &plan.binding {
        for workspace in &binding.runs {
            real_directory(workspace)?;
            for name in [".sync.lock", ".fetch.lock", ".crates.lock"] {
                let p = workspace.join(name);
                if owned.contains(&p) {
                    continue;
                }
                match fs::symlink_metadata(&p) {
                    Ok(_) => {
                        locks.push(lock_file(&p, false)?);
                        owned.insert(p);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    let fresh = if let Some(binding) = &plan.binding {
        catalog::preview_owned(
            &plan.policy,
            &binding.directory,
            &binding.next_previous,
            &plan.as_of,
            &owned,
        )?
    } else {
        preview_inner(
            &plan.policy,
            &plan.roots,
            &plan.as_of,
            &BTreeSet::new(),
            &owned,
            false,
        )?
    };
    if &fresh != plan {
        return Err("retention preview is stale or modified; preview again".into());
    }
    let mut report = ApplyReport {
        deleted_files: vec![],
        deleted_bytes: 0,
        removed_generations: vec![],
        failure: None,
    };
    for root in &plan.generations {
        let files: Vec<_> = plan
            .files
            .iter()
            .filter(|f| f.path.starts_with(root))
            .collect();
        let mut dirs = BTreeSet::from([root.clone()]);
        for file in files {
            if let Err(error) = crate::events::check() {
                let mut error = crate::Error::new(crate::ErrorKind::Cancelled, error.to_string());
                error.report = Some(toml::Value::try_from(&report)?);
                return Err(error.into());
            }
            let result = (|| -> Result<()> {
                crate::events::artifact(
                    &file.path.to_string_lossy(),
                    "delete",
                    0,
                    Some(file.bytes),
                )?;
                real_directory(file.path.parent().ok_or("file needs parent")?)?;
                if resume::hash_file(&file.path, file.bytes)? != (file.bytes, file.sha256.clone()) {
                    return Err(crate::Error::new(
                        crate::ErrorKind::Integrity,
                        "file changed during pruning",
                    )
                    .into());
                }
                fs::remove_file(&file.path)?;
                Ok(())
            })();
            if let Err(e) = result {
                if crate::error_kind(e.as_ref()) == crate::ErrorKind::Cancelled {
                    let mut error = crate::Error::new(crate::ErrorKind::Cancelled, e.to_string());
                    error.report = Some(toml::Value::try_from(&report)?);
                    return Err(error.into());
                }
                report.failure = Some(ApplyFailure {
                    path: file.path.clone(),
                    message: e.to_string(),
                });
                return Ok(report);
            }
            report.deleted_bytes += file.bytes;
            report.deleted_files.push(file.clone());
            let mut parent = file.path.parent();
            while let Some(p) = parent.filter(|p| p.starts_with(root)) {
                dirs.insert(p.to_path_buf());
                parent = p.parent();
            }
        }
        let mut dirs: Vec<_> = dirs.into_iter().collect();
        dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for dir in dirs {
            if let Err(e) = real_directory(&dir).and_then(|_| Ok(fs::remove_dir(&dir)?)) {
                report.failure = Some(ApplyFailure {
                    path: dir,
                    message: e.to_string(),
                });
                return Ok(report);
            }
        }
        report.removed_generations.push(root.clone());
    }
    Ok(report)
}

/// Explicit-root preview with progress, independent heartbeats and cancellation.
/// # Errors
/// Returns typed preview/cancellation errors; no deletion occurs.
pub fn preview_with_control(
    policy: &Policy,
    roots: &[PathBuf],
    as_of: &str,
    control: &crate::events::Control,
) -> Result<PrunePlan> {
    control.run("prune preview", || preview(policy, roots, as_of))
}
/// Discovery-backed preview with progress/cancel and the same managed-state binding.
/// # Errors
/// Returns typed preview/cancellation errors; no deletion occurs.
pub fn preview_directory_with_control(
    policy: &Policy,
    directory: &Path,
    previous: &Path,
    as_of: &str,
    control: &crate::events::Control,
) -> Result<PrunePlan> {
    control.run("prune preview", || {
        preview_directory(policy, directory, previous, as_of)
    })
}
/// Apply with progress/cancel. Cancelled errors carry a successful-prefix report if deletion started.
/// # Errors
/// Returns typed errors; cancellation never rolls back already unlinked approved files.
pub fn apply_with_control(
    plan: &PrunePlan,
    allow_delete: bool,
    control: &crate::events::Control,
) -> Result<ApplyReport> {
    control.run("prune apply", || apply(plan, allow_delete))
}
