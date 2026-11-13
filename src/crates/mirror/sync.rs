//! One-shot crate synchronization with an explicit saved-preview approval boundary.
use super::*;
use std::{fs::OpenOptions, path::PathBuf};

/// Inputs for one-shot planning or approval of an existing saved preview.
#[derive(Clone, Debug)]
pub struct SyncOptions {
    /// New managed workspace, or the workspace containing the preview being approved.
    pub output: PathBuf,
    /// Require explicit resume for already initialized payload work.
    pub resume: bool,
    /// Completed predecessor bound into the preview, when present.
    pub previous: Option<PathBuf>,
    /// Opt into archive HEAD estimates during initial planning only.
    pub estimate_sizes: bool,
}
/// Successful synchronization outcome; waiting for consent is not an error.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SyncReport {
    /// No archive transfer has been authorized; display this exact saved preview.
    NeedsApproval {
        /// Exact whole-preview approval identity.
        plan_id: String,
        /// Location of the locally reloadable frozen JSON preview.
        plan_path: PathBuf,
        /// Complete preview, including identities, sizes, reasons and budget flags.
        preview: Box<FrozenPlan>,
    },
    /// Approved archive transfers and immutable assembly completed.
    Completed {
        /// Exact approved plan identity.
        plan_id: String,
        /// Frozen preview used for this completed operation.
        plan_path: PathBuf,
        /// Verified completed generation; publication remains separate.
        mirror: PathBuf,
        /// Verified local generation report; publication remains separate.
        report: FetchReport,
    },
}
fn mismatch(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::ResumeMismatch, message).into()
}
fn validate_inputs(plan: &FrozenPlan, catalog: &Catalog, options: &SyncOptions) -> Result<()> {
    let mut semantic = catalog.clone();
    semantic.network = None;
    if serde_json::to_value(semantic)? != serde_json::to_value(&plan.catalog)? {
        return Err(mismatch(
            "crate sync config differs from the saved preview; create a new workspace",
        ));
    }
    let previous = options
        .previous
        .as_ref()
        .map(|p| -> Result<String> {
            Inventory::load(p)?;
            let raw = fs::read(p.join("snapshot.toml"))?;
            Ok(crate::sha256(&raw))
        })
        .transpose()?;
    if previous != plan.previous_inventory_sha256 {
        return Err(mismatch(
            "crate sync previous generation differs from the saved preview",
        ));
    }
    Ok(())
}
/// Plan without payloads, or fetch only the exact previously saved and explicitly approved plan.
/// Repeated calls do not refresh upstream metadata for an existing preview.
pub fn sync(
    catalog: &Catalog,
    source: &dyn CrateSource,
    options: &SyncOptions,
    approval: Option<&Approval>,
) -> Result<SyncReport> {
    catalog.validate()?;
    let plan_path = options.output.join("plan.json");
    if !options.output.try_exists()? {
        if options.resume || approval.is_some() {
            return Err(mismatch(
                "crate sync approval/resume requires an existing saved preview",
            ));
        }
        // Resolve before reserving output. Cancellation cannot leave a partially written preview.
        let preview = plan(
            catalog,
            source,
            options.previous.as_deref(),
            options.estimate_sizes,
        )?;
        fs::create_dir(&options.output)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(options.output.join(".crates.lock"))?;
        lock.try_lock()
            .map_err(|_| Error::new(ErrorKind::Locked, "crate workspace is locked"))?;
        preview.save_new(&plan_path)?;
        events::stage("approval")?;
        return Ok(SyncReport::NeedsApproval {
            plan_id: preview.plan_id.clone(),
            plan_path,
            preview: Box::new(preview),
        });
    }
    let metadata = fs::symlink_metadata(&options.output)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid(
            "crate sync output must be a real managed directory",
        ));
    }
    for entry in fs::read_dir(&options.output)? {
        let entry = entry?;
        if ![
            ".crates.lock",
            "plan.json",
            ".download",
            "mirror",
            ".rust-mirror-prune.lock",
        ]
        .iter()
        .any(|n| entry.file_name() == *n)
        {
            return Err(invalid("crate sync output contains unrelated files"));
        }
    }
    let preview = FrozenPlan::load(&plan_path)?;
    validate_inputs(&preview, catalog, options)?;
    if options.output.join(".download").try_exists()? && !options.resume {
        return Err(Error::new(
            ErrorKind::OutputExists,
            "initialized crate work requires --resume",
        )
        .into());
    }
    if let Some(approval) = approval {
        let report = fetch(
            &preview,
            source,
            &FetchOptions {
                output: options.output.clone(),
                resume: true,
                previous: options.previous.clone(),
            },
            approval,
        )?;
        Ok(SyncReport::Completed {
            plan_id: preview.plan_id.clone(),
            plan_path,
            mirror: report.mirror.clone(),
            report,
        })
    } else {
        events::stage("approval")?;
        Ok(SyncReport::NeedsApproval {
            plan_id: preview.plan_id.clone(),
            plan_path,
            preview: Box::new(preview),
        })
    }
}
/// Controlled sync using the same observer/cancellation and independent heartbeat as other operations.
pub fn sync_with_control(
    catalog: &Catalog,
    source: &dyn CrateSource,
    options: &SyncOptions,
    approval: Option<&Approval>,
    control: &events::Control,
) -> Result<SyncReport> {
    control.run("crates-sync", || sync(catalog, source, options, approval))
}
