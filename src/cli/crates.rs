//! Crate namespace adapter; all mirroring/validation stays in the public library.
use crate::{
    crates::{
        catalog::Catalog,
        mirror::{self, Approval, FetchOptions, FrozenPlan},
        source::RegistrySource,
    },
    Error, ErrorKind, Result,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
pub(super) fn invalid(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Error::new(ErrorKind::Invalid, message).into()
}
pub(super) fn read(path: &Path, limit: u64) -> Result<String> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > limit {
        return Err(invalid("input must be a bounded regular file"));
    }
    let mut text = String::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(invalid("input exceeds bound"));
    }
    Ok(text)
}
pub(super) fn output<T: serde::Serialize>(report: &T, text: String) -> Result<()> {
    let value = toml::Value::try_from(report)?;
    super::REPORT.with(|slot| *slot.borrow_mut() = Some(value));
    super::emit_output(text)
}
fn take(values: &mut BTreeMap<String, OsString>, key: &str) -> Result<OsString> {
    values
        .remove(key)
        .ok_or_else(|| invalid("missing required crate option"))
}
fn path(values: &mut BTreeMap<String, OsString>, key: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(take(values, key)?))
}
fn text(values: &mut BTreeMap<String, OsString>, key: &str) -> Result<String> {
    take(values, key)?
        .into_string()
        .map_err(|_| invalid("crate option must be UTF-8"))
}
pub(super) fn run(args: impl Iterator<Item = OsString>) -> Result<()> {
    let mut args = args;
    let command = args
        .next()
        .unwrap_or_else(|| "--help".into())
        .into_string()
        .map_err(|_| invalid("crate command must be UTF-8"))?;
    if matches!(command.as_str(), "import-lock" | "remove-lock") {
        return super::lock_import::run(&command, args);
    }
    if command == "--help" || command == "-h" {
        return super::emit_output("rust-mirror crates plan --config CATALOG --output NEW_PLAN.json [--previous MIRROR] [--estimate-sizes]\nrust-mirror crates sync --config FILE --output RUN [--approve-plan ID] [--resume]\nrust-mirror crates show --plan FILE\nrust-mirror crates explain --plan FILE --crate NAME [--version VERSION] [--limit N]\nrust-mirror crates fetch --plan FILE --output NEW_RUN --approve-plan ID [--previous MIRROR] [--resume]\nrust-mirror crates generations --directory DIRECTORY\nrust-mirror crates prune --config FILE --directory DIRECTORY --next-previous MIRROR --as-of YYYY-MM-DD [--dry-run | --apply]\nrust-mirror crates catalog --config FILE\nrust-mirror crates paste --input DEPENDENCIES.toml [--output NEW_CATALOG]\nrust-mirror crates import-lock --config FILE --lock NAME=PATH [--lock NAME=PATH ...] [--write]\nrust-mirror crates remove-lock --config FILE --source NAME [--source NAME ...] [--write]\n".into());
    }
    let mut values = BTreeMap::new();
    let mut resume = false;
    let mut delete = false;
    let mut dry_run = false;
    while let Some(key) = args.next() {
        let key = key
            .into_string()
            .map_err(|_| invalid("crate option must be UTF-8"))?;
        if key == "--apply" || key == "--dry-run" {
            if command != "prune" || delete || dry_run {
                return Err(invalid(
                    "apply/dry-run are mutually exclusive prune options",
                ));
            }
            delete = key == "--apply";
            dry_run = key == "--dry-run";
            continue;
        }
        if key == "--resume" {
            if resume {
                return Err(invalid("duplicate resume option"));
            }
            resume = true;
            continue;
        }
        if ![
            "--config",
            "--output",
            "--plan",
            "--previous",
            "--approve-plan",
            "--crate",
            "--version",
            "--limit",
            "--input",
            "--directory",
            "--next-previous",
            "--as-of",
        ]
        .contains(&key.as_str())
        {
            return Err(invalid("unknown crate option"));
        }
        if values
            .insert(
                key,
                args.next()
                    .ok_or_else(|| invalid("missing crate option value"))?,
            )
            .is_some()
        {
            return Err(invalid("duplicate crate option"));
        }
    }
    let ensure_empty = |values: &BTreeMap<String, OsString>| -> Result<()> {
        if values.is_empty() {
            Ok(())
        } else {
            Err(invalid("option is not supported by this crate command"))
        }
    };
    match command.as_str() {
        "generations" => {
            if resume {
                return Err(invalid("discovery does not resume"));
            }
            let directory = path(&mut values, "--directory")?;
            ensure_empty(&values)?;
            let report = crate::selection::retention::discover(&directory)?;
            let human = serde_json::to_string_pretty(&report)?;
            output(&report, human)
        }
        "prune" => {
            if resume {
                return Err(invalid("prune does not resume deletion"));
            }
            let config = path(&mut values, "--config")?;
            let directory = path(&mut values, "--directory")?;
            let anchor = path(&mut values, "--next-previous")?;
            let as_of = text(&mut values, "--as-of")?;
            ensure_empty(&values)?;
            let catalog = Catalog::parse(&read(
                &config,
                crate::crates::lockfile::CATALOG_LIMIT as u64,
            )?)?;
            let preview =
                crate::crates::retention::preview_directory(&catalog, &directory, &anchor, &as_of)?;
            if delete {
                let report = crate::crates::retention::apply(&preview, true)?;
                let human = serde_json::to_string_pretty(&report)?;
                output(&report, human)?;
                if report.failure.is_some() {
                    let mut error = Error::new(ErrorKind::Io, "crate pruning stopped after a partial deletion; inspect the structured report");
                    error.report = Some(toml::Value::try_from(&report)?);
                    return Err(error.into());
                }
                Ok(())
            } else {
                let human = serde_json::to_string_pretty(&preview)?;
                output(&preview, human)
            }
        }
        "catalog" => {
            if resume {
                return Err(invalid("catalog does not resume"));
            }
            let input = path(&mut values, "--config")?;
            ensure_empty(&values)?;
            let catalog = Catalog::parse(&read(
                &input,
                crate::crates::lockfile::CATALOG_LIMIT as u64,
            )?)?;
            output(&catalog, catalog.to_toml()?)
        }
        "paste" => {
            if resume {
                return Err(invalid("paste does not resume"));
            }
            let input = path(&mut values, "--input")?;
            let destination = values.remove("--output").map(PathBuf::from);
            ensure_empty(&values)?;
            let report = Catalog::from_dependencies(&read(
                &input,
                crate::crates::lockfile::CATALOG_LIMIT as u64,
            )?);
            if let Some(destination) = destination {
                if report.rejected.is_empty() {
                    report
                        .catalog
                        .as_ref()
                        .ok_or_else(|| invalid("paste has no accepted entries"))?
                        .write(&destination, false)?;
                }
            }
            let human = serde_json::to_string_pretty(&report)?;
            output(&report, human)
        }
        "plan" => {
            if resume {
                return Err(invalid("planning cannot resume metadata observations"));
            }
            let config = path(&mut values, "--config")?;
            let destination = path(&mut values, "--output")?;
            let previous = values.remove("--previous").map(PathBuf::from);
            ensure_empty(&values)?;
            let catalog = Catalog::parse(&read(
                &config,
                crate::crates::lockfile::CATALOG_LIMIT as u64,
            )?)?;
            let source = RegistrySource::new(
                &super::network_options::options(catalog.network.as_ref())?,
                catalog.crate_network.clone(),
            )?;
            let plan = mirror::plan(
                &catalog,
                &source,
                previous.as_deref(),
                super::network_options::estimate(),
            )?;
            plan.save_new(&destination)?;
            output(&plan,format!("Frozen preview: {} crates, {} versions, {} files\nPlan: {}\nApprove the complete preview with --approve-plan {}\n",plan.counts.crates,plan.counts.versions,plan.counts.files,destination.display(),plan.plan_id))
        }
        "sync" => {
            let config = path(&mut values, "--config")?;
            let output_path = path(&mut values, "--output")?;
            let previous = values.remove("--previous").map(PathBuf::from);
            let approval = values
                .remove("--approve-plan")
                .map(|v| {
                    v.into_string()
                        .map(|plan_id| Approval { plan_id })
                        .map_err(|_| invalid("approval ID must be UTF-8"))
                })
                .transpose()?;
            ensure_empty(&values)?;
            let catalog = Catalog::parse(&read(
                &config,
                crate::crates::lockfile::CATALOG_LIMIT as u64,
            )?)?;
            let source = RegistrySource::new(
                &super::network_options::options(catalog.network.as_ref())?,
                catalog.crate_network.clone(),
            )?;
            let report = mirror::sync(
                &catalog,
                &source,
                &mirror::SyncOptions {
                    output: output_path,
                    resume,
                    previous,
                    estimate_sizes: super::network_options::estimate(),
                },
                approval.as_ref(),
            )?;
            let human = match &report {
                mirror::SyncReport::NeedsApproval { plan_path, preview, .. } => format!("Preview saved to {}. Review with crates show, then approve with --approve-plan {}", plan_path.display(), preview.plan_id),
                mirror::SyncReport::Completed { report, .. } => format!("Verified crate mirror: {}", report.mirror.display()),
            };
            output(&report, human)
        }
        "show" => {
            if resume {
                return Err(invalid("show does not resume"));
            }
            let input = path(&mut values, "--plan")?;
            ensure_empty(&values)?;
            let plan = FrozenPlan::load(&input)?;
            let human = serde_json::to_string_pretty(&plan)?;
            output(&plan, human)
        }
        "explain" => {
            if resume {
                return Err(invalid("explain does not resume"));
            }
            let input = path(&mut values, "--plan")?;
            let name = text(&mut values, "--crate")?;
            let version = values
                .remove("--version")
                .map(|v| {
                    v.into_string()
                        .map_err(|_| invalid("version must be UTF-8"))
                })
                .transpose()?;
            let limit = values
                .remove("--limit")
                .map(|v| {
                    v.into_string()
                        .map_err(|_| invalid("limit must be UTF-8"))?
                        .parse::<usize>()
                        .map_err(|_| invalid("invalid explanation limit"))
                })
                .transpose()?
                .unwrap_or(64);
            ensure_empty(&values)?;
            let plan = FrozenPlan::load(&input)?;
            let report = mirror::explain(&plan, &name, version.as_deref(), limit)?;
            let human = serde_json::to_string_pretty(&report)?;
            output(&report, human)
        }
        "fetch" => {
            let input = path(&mut values, "--plan")?;
            let output_path = path(&mut values, "--output")?;
            let approved = text(&mut values, "--approve-plan")?;
            let previous = values.remove("--previous").map(PathBuf::from);
            ensure_empty(&values)?;
            let plan = FrozenPlan::load(&input)?;
            let source = RegistrySource::new(
                &super::network_options::options(None)?,
                plan.catalog.crate_network.clone(),
            )?;
            let report = mirror::fetch(
                &plan,
                &source,
                &FetchOptions {
                    output: output_path,
                    resume,
                    previous,
                },
                &Approval { plan_id: approved },
            )?;
            output(
                &report,
                format!(
                    "Verified crate mirror: {}\n{} archives; {} new bytes; {} reused bytes\n",
                    report.mirror.display(),
                    report.files_completed,
                    report.transferred_bytes,
                    report.reused_bytes
                ),
            )
        }
        _ => Err(invalid("unknown crate subcommand")),
    }
}
