//! Crate pin protection layered on the shared whole-generation deletion engine.
use super::{
    catalog::Catalog,
    mirror::{Identity, Inventory},
};
use crate::{selection::retention as core, Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// Explicit crate-generation protection; omitting the table disables pruning.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Policy {
    /// Minimum newest completed generations, including current (0–32).
    pub keep_newest: usize,
    /// Previous rollback generations beyond current (0–31).
    pub previous_generations: usize,
    /// Explicit completed roots to protect, independent of crate presence (at most 32).
    pub pinned_generations: Vec<PathBuf>,
}
impl Policy {
    /// Validate resource bounds without reading disk or environment.
    pub fn validate(&self) -> Result<()> {
        if self.keep_newest > 32
            || self.previous_generations > 31
            || self.pinned_generations.len() > 32
        {
            return Err(
                Error::new(ErrorKind::Budget, "crate retention policy exceeds bounds").into(),
            );
        }
        Ok(())
    }
}
/// Exact pinned crate and declared root feature profile requiring a preserved witness.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct PinnedCrate {
    /// Exact registry package/version.
    pub identity: Identity,
    /// Sorted explicit root features.
    pub features: Vec<String>,
    /// Whether the root requests defaults.
    pub default_features: bool,
}
/// Structured preview bound to the directory, next-previous anchor and declared exact crate pins.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PrunePlan {
    /// Wrapper schema 1, independent of inventory/event schemas.
    pub schema: u32,
    /// Exact shared-engine file/folder deletions, bytes and keep reasons.
    pub deletion: core::PrunePlan,
    /// Current crate-generation policy.
    pub policy: Policy,
    /// Exact pins derived from catalog rules; feature closure is protected as part of the whole generation.
    pub pinned_crates: Vec<PinnedCrate>,
    /// Explicit candidate directory.
    pub directory: PathBuf,
    /// Exact generation needed as the next previous input, never an mtime guess.
    pub next_previous: PathBuf,
    /// Explicit date reference carried by the shared engine.
    pub as_of: String,
}
fn preview_pins(
    policy: &Policy,
    pins: &[PinnedCrate],
    directory: &Path,
    next_previous: &Path,
    as_of: &str,
) -> Result<PrunePlan> {
    policy.validate()?;
    let listing = core::discover(directory)?;
    if !listing.issues.is_empty() || listing.generations.iter().any(|g| g.kind != "crates") {
        return Err(Error::new(
            ErrorKind::Invalid,
            "crate pruning needs a safely recognized crate-generation directory",
        )
        .into());
    }
    if pins.len() > 4096 {
        return Err(
            Error::new(ErrorKind::Budget, "crate pin count exceeds retention bound").into(),
        );
    }
    let mut protected: BTreeSet<_> = policy
        .pinned_generations
        .iter()
        .map(|p| p.canonicalize())
        .collect::<std::io::Result<_>>()?;
    if protected.len() != policy.pinned_generations.len() {
        return Err(Error::new(ErrorKind::Invalid, "duplicate pinned generation").into());
    }
    let mut remaining: BTreeSet<_> = pins.iter().cloned().collect();
    if remaining.len() != pins.len() {
        return Err(Error::new(ErrorKind::Invalid, "duplicate crate pin").into());
    }
    for pin in &remaining {
        super::source::archive_path(&pin.identity.name, &pin.identity.version)?;
    }
    // Newest exact copy is sufficient: the completed inventory verifies all recorded closures.
    // Distinct explicitly pinned generations remain protected separately.
    for generation in &listing.generations {
        crate::events::check()?;
        let inventory = Inventory::load(&generation.path)?;
        let covered: Vec<_> = remaining
            .iter()
            .filter(|pin| {
                inventory.plan.roots.iter().any(|root| {
                    let features: Vec<_> = root
                        .profile
                        .features
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    root.profile.name == pin.identity.name
                        && root.profile.version == pin.identity.version
                        && root.profile.default_features == pin.default_features
                        && features == pin.features
                })
            })
            .cloned()
            .collect();
        if !covered.is_empty() {
            protected.insert(generation.path.clone());
        }
        for pin in covered {
            remaining.remove(&pin);
        }
    }
    if !remaining.is_empty() {
        return Err(Error::new(
            ErrorKind::Invalid,
            "a pinned crate version is absent from completed generations",
        )
        .into());
    }
    let shared = core::Policy {
        pinned_releases: Vec::new(),
        latest_stable: 0,
        nightly_days: 0,
        previous_generations: policy.previous_generations,
        keep_newest: policy.keep_newest,
        pinned_generations: protected.into_iter().collect(),
    };
    let deletion = core::preview_directory(&shared, directory, next_previous, as_of)?;
    Ok(PrunePlan {
        schema: 1,
        deletion,
        policy: policy.clone(),
        pinned_crates: pins.to_vec(),
        directory: directory.canonicalize()?,
        next_previous: next_previous.canonicalize()?,
        as_of: as_of.into(),
    })
}
/// Preview exact whole-generation deletions, protecting catalog pins and paused-run dependencies.
/// Reads and verifies local candidates; never modifies files or contacts a server.
pub fn preview_directory(
    catalog: &Catalog,
    directory: &Path,
    next_previous: &Path,
    as_of: &str,
) -> Result<PrunePlan> {
    catalog.validate()?;
    let policy = catalog.crate_retention.as_ref().ok_or_else(|| {
        Error::new(
            ErrorKind::Invalid,
            "crate retention is disabled; configure crate-retention explicitly",
        )
    })?;
    let mut pins = BTreeSet::new();
    for (alias, rule) in &catalog.crate_rules {
        let dependency = &catalog.crates[alias];
        for version in &rule.pins {
            pins.insert(PinnedCrate {
                identity: Identity {
                    name: dependency.package.as_deref().unwrap_or(alias).into(),
                    version: version.clone(),
                },
                features: dependency
                    .features
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                default_features: dependency.default_features,
            });
        }
    }
    preview_pins(
        policy,
        &pins.into_iter().collect::<Vec<_>>(),
        directory,
        next_previous,
        as_of,
    )
}
/// Reconstruct the complete preview and refuse stale disk/policy state before invoking shared locked deletion.
/// Requires explicit deletion authority; remote deletion is never supported.
pub fn apply(plan: &PrunePlan, allow_delete: bool) -> Result<core::ApplyReport> {
    if !allow_delete {
        return Err(Error::new(
            ErrorKind::Invalid,
            "crate prune apply requires explicit deletion authority",
        )
        .into());
    }
    if plan.schema != 1 {
        return Err(Error::new(ErrorKind::Invalid, "unsupported crate retention plan").into());
    }
    let fresh = preview_pins(
        &plan.policy,
        &plan.pinned_crates,
        &plan.directory,
        &plan.next_previous,
        &plan.as_of,
    )?;
    if &fresh != plan {
        return Err(Error::new(
            ErrorKind::ResumeMismatch,
            "crate prune preview is stale; preview again",
        )
        .into());
    }
    core::apply(&plan.deletion, true)
}
/// Controlled crate pruning preview with independent heartbeat and cooperative verification cancellation.
pub fn preview_directory_with_control(
    catalog: &Catalog,
    directory: &Path,
    next_previous: &Path,
    as_of: &str,
    control: &crate::events::Control,
) -> Result<PrunePlan> {
    control.run("crates-prune-preview", || {
        preview_directory(catalog, directory, next_previous, as_of)
    })
}
/// Controlled application; cancellation can leave a reported successful deletion prefix.
pub fn apply_with_control(
    plan: &PrunePlan,
    allow_delete: bool,
    control: &crate::events::Control,
) -> Result<core::ApplyReport> {
    control.run("crates-prune-apply", || apply(plan, allow_delete))
}
