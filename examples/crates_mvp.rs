//! Development-source qualification driver; explicit plan-ID consent is required for every fetch invocation.
use rust_mirror::{
    crates::{
        catalog::Catalog,
        mirror::{self, Approval, FetchOptions, FrozenPlan},
        source::RegistrySource,
    },
    NetworkOptions,
};
use std::{fs, path::PathBuf};
fn main() -> rust_mirror::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 {
        return Err(
            "usage: crates_mvp plan CATALOG NEW_PLAN | fetch PLAN NEW_RUN APPROVED_ID [--resume]"
                .into(),
        );
    }
    if args[0] == "plan" {
        if args.len() != 3 {
            return Err("plan needs catalog and new plan path".into());
        }
        let catalog = Catalog::parse(&fs::read_to_string(&args[1])?)?;
        let options = catalog
            .network
            .as_ref()
            .map(|n| n.resolve())
            .transpose()?
            .unwrap_or_default();
        let source = RegistrySource::new(&options, catalog.crate_network.clone())?;
        let plan = mirror::plan(&catalog, &source, None, false)?;
        plan.save_new(&PathBuf::from(&args[2]))?;
        println!("{}", serde_json::to_string(&plan)?);
    } else if args[0] == "fetch" {
        if args.len() != 4 && !(args.len() == 5 && args[4] == "--resume") {
            return Err("fetch needs plan, output and explicit approved ID".into());
        }
        let plan = FrozenPlan::load(&PathBuf::from(&args[1]))?;
        let source = RegistrySource::new(
            &NetworkOptions::default(),
            plan.catalog.crate_network.clone(),
        )?;
        let report = mirror::fetch(
            &plan,
            &source,
            &FetchOptions {
                output: PathBuf::from(&args[2]),
                resume: args.len() == 5,
                previous: None,
            },
            &Approval {
                plan_id: args[3]
                    .clone()
                    .into_string()
                    .map_err(|_| "approval ID must be UTF-8")?,
            },
        )?;
        println!("{}", serde_json::to_string(&report)?);
    } else {
        return Err("unknown research-driver operation".into());
    }
    Ok(())
}
