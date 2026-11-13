use rust_mirror::{
    crates::{
        catalog::Catalog,
        mirror::{self, FrozenPlan},
        source::CrateSource,
    },
    Result, Transfer,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Source {
    files: BTreeMap<String, Vec<u8>>,
    heads: AtomicUsize,
    archives: AtomicUsize,
}
impl CrateSource for Source {
    fn index(&self, name: &str, _limit: usize) -> Result<Vec<u8>> {
        Ok(self.files.get(name).ok_or("unknown index")?.clone())
    }
    fn head(&self, _name: &str, _version: &str) -> Result<Option<u64>> {
        self.heads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(7))
    }
    fn archive(
        &self,
        _name: &str,
        _version: &str,
        _out: &mut dyn Write,
        _offset: u64,
        _limit: u64,
    ) -> Result<Transfer> {
        self.archives.fetch_add(1, Ordering::SeqCst);
        panic!("planning must never request an archive")
    }
}
fn fixture() -> (Catalog, Source) {
    let catalog = Catalog::parse(
        r#"
[crates]
alias={package="x",version="1"}
[crate-rules.alias]
latest=2
pins=["0.9.0"]
[crate-policy]
hosts=["x86_64-pc-windows-msvc"]
targets=["x86_64-pc-windows-msvc"]
[crate-output]
base-url="https://mirror.example/gen-001/"
"#,
    )
    .unwrap();
    let data=["0.9.0","1.0.0","1.1.0","2.0.0"].into_iter().flat_map(|version|{let mut data=serde_json::to_vec(&json!({"name":"x","vers":version,"deps":[],"features":{},"cksum":"0".repeat(64),"yanked":false})).unwrap();data.push(b'\n');data}).collect();
    (
        catalog,
        Source {
            files: BTreeMap::from([("x".into(), data)]),
            heads: AtomicUsize::new(0),
            archives: AtomicUsize::new(0),
        },
    )
}
#[test]
fn frozen_catalog_selection_heads_and_local_roundtrip_are_archive_free() {
    let (catalog, source) = fixture();
    let plan = mirror::plan(&catalog, &source, None, true).unwrap();
    assert_eq!(plan.roots.len(), 3);
    assert_eq!(plan.versions.len(), 3);
    assert_eq!(source.heads.load(Ordering::SeqCst), 3);
    assert_eq!(source.archives.load(Ordering::SeqCst), 0);
    assert_eq!(plan.size_estimate.expected_archive_bytes, Some(21));
    assert!(plan.versions.iter().all(|v| v.root_aliases == ["alias"]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plan.json");
    plan.save_new(&path).unwrap();
    assert!(plan.save_new(&path).is_err());
    let loaded = FrozenPlan::load(&path).unwrap();
    assert_eq!(plan.plan_id, loaded.plan_id);
}
#[test]
fn altered_preview_or_root_omissions_cannot_be_fetched_as_valid_plans() {
    let (catalog, source) = fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let mut bad = plan.clone();
    bad.versions[0].cksum = "1".repeat(64);
    assert!(bad.validate().is_err());
    let mut bad = plan.clone();
    bad.roots.pop();
    assert!(bad.validate().is_err());
    let mut bad = plan.clone();
    bad.counts.files += 1;
    assert!(bad.validate().is_err());
    let mut bad = plan;
    bad.catalog.crate_output.base_url = "https://user:password@example.test/x".into();
    assert!(bad.validate().is_err());
    assert_eq!(source.archives.load(Ordering::SeqCst), 0);
}
#[test]
fn known_budget_violations_are_visible_preview_flags_not_invented_zeroes() {
    let (mut catalog, source) = fixture();
    catalog.crate_policy.max_file_bytes = 5;
    catalog.crate_policy.max_transfer_bytes = 10;
    catalog.crate_policy.max_final_bytes = 10;
    let plan = mirror::plan(&catalog, &source, None, true).unwrap();
    assert!(plan.budgets.exceeds_final_budget);
    assert!(plan.budgets.exceeds_transfer_budget);
    assert!(plan.budgets.oversized_files.len() >= 3);
    let unknown = mirror::plan(&catalog, &source, None, false).unwrap();
    assert_eq!(unknown.size_estimate.unknown_archives, 3);
    assert_eq!(unknown.size_estimate.expected_archive_bytes, None);
    assert_eq!(source.archives.load(Ordering::SeqCst), 0);
}

#[test]
fn explain_and_client_configuration_are_structured_and_local() {
    let (catalog, source) = fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let explanation = mirror::explain(&plan, "x", None, 1).unwrap();
    assert_eq!(explanation.chains.len(), 1);
    assert_eq!(explanation.total_chains, Some(3));
    assert!(explanation.truncated);
    assert_eq!(explanation.chains[0].root_alias, "alias");
    let config = mirror::client_config(&plan).unwrap();
    assert!(config.contains("sparse+https://mirror.example/gen-001/cargo/index/"));
    assert_eq!(source.archives.load(Ordering::SeqCst), 0);
}

struct PayloadSource {
    metadata: Vec<u8>,
    payload: Vec<u8>,
    fail: std::sync::atomic::AtomicBool,
    calls: AtomicUsize,
}
impl CrateSource for PayloadSource {
    fn index(&self, _name: &str, _limit: usize) -> Result<Vec<u8>> {
        Ok(self.metadata.clone())
    }
    fn archive(
        &self,
        _name: &str,
        _version: &str,
        out: &mut dyn Write,
        offset: u64,
        _limit: u64,
    ) -> Result<Transfer> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let start = offset as usize;
        if start >= self.payload.len() {
            return Ok(Transfer::Restart);
        }
        if self.fail.swap(false, Ordering::SeqCst) {
            out.write_all(&self.payload[start..start + 3])?;
            return Err(rust_mirror::Error::new(
                rust_mirror::ErrorKind::Network,
                "interrupted fixture transfer",
            )
            .into());
        }
        out.write_all(&self.payload[start..])?;
        Ok(Transfer::Complete)
    }
    fn attempts(&self) -> usize {
        4
    }
}
fn payload_fixture() -> (Catalog, PayloadSource) {
    use sha2::{Digest, Sha256};
    let payload = b"payload".to_vec();
    let hash = hex::encode(Sha256::digest(&payload));
    let mut metadata = serde_json::to_vec(
        &json!({"name":"x","vers":"1.0.0","deps":[],"features":{},"cksum":hash,"yanked":false}),
    )
    .unwrap();
    metadata.push(b'\n');
    let catalog=Catalog::parse("[crates]\nx='=1.0.0'\n[crate-policy]\nhosts=['x86_64-unknown-linux-gnu']\ntargets=['x86_64-unknown-linux-gnu']\n[crate-output]\nbase-url='https://mirror.example/generation-001'\n").unwrap();
    (
        catalog,
        PayloadSource {
            metadata,
            payload,
            fail: std::sync::atomic::AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        },
    )
}
#[test]
fn approved_fetch_exposes_only_verified_schema_four_sparse_output() {
    use mirror::{Approval, FetchOptions, Inventory};
    let (catalog, source) = payload_fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("run");
    let options = FetchOptions {
        output: output.clone(),
        resume: false,
        previous: None,
    };
    assert!(mirror::fetch(
        &plan,
        &source,
        &options,
        &Approval {
            plan_id: "wrong".into()
        }
    )
    .is_err());
    assert!(!output.exists());
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    let report = mirror::fetch(
        &plan,
        &source,
        &options,
        &Approval {
            plan_id: plan.plan_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(report.transferred_bytes, 7);
    assert_eq!(report.files_completed, 1);
    let inventory = Inventory::load(&report.mirror).unwrap();
    assert_eq!(inventory.schema, 4);
    assert_eq!(
        std::fs::read(report.mirror.join("cargo/index/1/x")).unwrap(),
        source.metadata
    );
    assert_eq!(
        std::fs::read(report.mirror.join("cargo/crates/x/x-1.0.0.crate")).unwrap(),
        source.payload
    );
    let marker = std::fs::read_to_string(report.mirror.join("snapshot.toml")).unwrap();
    assert!(toml::from_str::<rust_mirror::selection::SelectionSnapshot>(&marker).is_err());
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(report.mirror.join("cargo/index/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        config["dl"],
        "https://mirror.example/generation-001/cargo/crates/{crate}/{crate}-{version}.crate"
    );
}
#[test]
fn interrupted_bytes_resume_under_the_same_approval_and_persisted_budget() {
    use mirror::{Approval, FetchOptions, Inventory};
    let (catalog, source) = payload_fixture();
    source.fail.store(true, Ordering::SeqCst);
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut options = FetchOptions {
        output: dir.path().join("run"),
        resume: false,
        previous: None,
    };
    let approval = Approval {
        plan_id: plan.plan_id.clone(),
    };
    assert!(mirror::fetch(&plan, &source, &options, &approval).is_err());
    assert!(!options.output.join("mirror").exists());
    options.resume = true;
    let report = mirror::fetch(&plan, &source, &options, &approval).unwrap();
    assert_eq!(report.transferred_bytes, 4);
    assert_eq!(report.total_transferred_bytes, 7);
    assert_eq!(report.reused_bytes, 3);
    Inventory::load(&report.mirror).unwrap();
    let again = mirror::fetch(&plan, &source, &options, &approval).unwrap();
    assert_eq!(again.transferred_bytes, 0);
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
}
#[test]
fn completed_corruption_and_unknown_workspace_files_are_never_overwritten() {
    use mirror::{Approval, FetchOptions};
    let (catalog, source) = payload_fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut options = FetchOptions {
        output: dir.path().join("run"),
        resume: false,
        previous: None,
    };
    let approval = Approval {
        plan_id: plan.plan_id.clone(),
    };
    let report = mirror::fetch(&plan, &source, &options, &approval).unwrap();
    std::fs::write(
        report.mirror.join("cargo/crates/x/x-1.0.0.crate"),
        b"corrupt",
    )
    .unwrap();
    let snippet = mirror::client_config_for_generation(&report.mirror).unwrap();
    assert!(snippet.contains("sparse+https://mirror.example/generation-001/cargo/index/"));
    assert!(mirror::Inventory::load(&report.mirror).is_err());
    options.resume = true;
    assert!(mirror::fetch(&plan, &source, &options, &approval).is_err());
    assert_eq!(
        std::fs::read(report.mirror.join("cargo/crates/x/x-1.0.0.crate")).unwrap(),
        b"corrupt"
    );
    std::fs::write(options.output.join("user-note"), b"user work").unwrap();
    assert!(mirror::fetch(&plan, &source, &options, &approval).is_err());
    assert_eq!(
        std::fs::read(options.output.join("user-note")).unwrap(),
        b"user work"
    );
}

#[test]
fn sync_saves_one_preview_and_requires_its_exact_approval_without_refresh() {
    use mirror::{Approval, SyncOptions, SyncReport};
    let (catalog, mut source) = payload_fixture();
    let dir = tempfile::tempdir().unwrap();
    let mut options = SyncOptions {
        output: dir.path().join("sync"),
        resume: false,
        previous: None,
        estimate_sizes: false,
    };
    let SyncReport::NeedsApproval {
        preview, plan_path, ..
    } = mirror::sync(&catalog, &source, &options, None).unwrap()
    else {
        panic!("must require consent")
    };
    assert!(plan_path.is_file());
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    assert!(!options.output.join("mirror").exists());
    // Once saved, neither newer versions nor an unavailable index changes this preview.
    source.metadata = b"not an index".to_vec();
    let SyncReport::NeedsApproval { preview: same, .. } =
        mirror::sync(&catalog, &source, &options, None).unwrap()
    else {
        panic!("must retain preview")
    };
    assert_eq!(same.plan_id, preview.plan_id);
    let report_value = toml::Value::try_from(&SyncReport::NeedsApproval {
        plan_path,
        preview: same,
        plan_id: preview.plan_id.clone(),
    })
    .unwrap();
    assert_eq!(report_value["status"].as_str(), Some("needs_approval"));
    let mut changed = catalog.clone();
    changed
        .crates
        .get_mut("x")
        .unwrap()
        .features
        .push("new-feature".into());
    assert!(mirror::sync(&changed, &source, &options, None).is_err());
    assert!(mirror::sync(
        &catalog,
        &source,
        &options,
        Some(&Approval {
            plan_id: "wrong".into()
        })
    )
    .is_err());
    let approval = Approval {
        plan_id: preview.plan_id.clone(),
    };
    let SyncReport::Completed { report, .. } =
        mirror::sync(&catalog, &source, &options, Some(&approval)).unwrap()
    else {
        panic!("approved transfer must complete")
    };
    assert_eq!(report.transferred_bytes, 7);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert!(mirror::sync(&catalog, &source, &options, Some(&approval)).is_err());
    options.resume = true;
    let SyncReport::Completed { report, .. } =
        mirror::sync(&catalog, &source, &options, Some(&approval)).unwrap()
    else {
        panic!("resume complete")
    };
    assert_eq!(report.transferred_bytes, 0);
}

#[test]
fn successive_generations_preserve_closures_bytes_and_separate_yank_observations() {
    use mirror::{Approval, FetchOptions, Inventory};
    let (mut catalog, mut source) = payload_fixture();
    catalog.crates.get_mut("x").unwrap().version = "*".into();
    let directory = tempfile::tempdir().unwrap();
    let mut previous = None;
    let mut first_line = Vec::new();
    for generation in 1..=3 {
        catalog.crate_output.base_url = format!("https://mirror.example/generation-{generation}");
        let original: serde_json::Value = serde_json::from_slice(
            source
                .metadata
                .split_inclusive(|b| *b == b'\n')
                .next()
                .unwrap(),
        )
        .unwrap();
        source.metadata.clear();
        for version in 1..=generation {
            let mut record = original.clone();
            record["vers"] = json!(format!("{version}.0.0"));
            record["yanked"] = json!(version == 1 && generation >= 2);
            source.metadata.extend(serde_json::to_vec(&record).unwrap());
            source.metadata.push(b'\n');
        }
        if generation == 1 {
            first_line = source.metadata.clone();
        }
        let before = source.calls.load(Ordering::SeqCst);
        let plan = mirror::plan(&catalog, &source, previous.as_deref(), false).unwrap();
        assert_eq!(plan.versions.len(), generation);
        assert_eq!(plan.changes.added.len(), 1);
        assert_eq!(plan.changes.retained.len(), generation - 1);
        assert_eq!(
            plan.changes.newly_yanked_retained.len(),
            usize::from(generation == 2)
        );
        if generation > 1 {
            assert_eq!(plan.versions[0].index_line.as_bytes(), first_line);
            assert_eq!(
                plan.changes.no_longer_selected_retained.len(),
                generation - 1
            );
        }
        let output = directory.path().join(format!("run-{generation}"));
        let saved = directory.path().join(format!("plan-{generation}.json"));
        plan.save_new(&saved).unwrap();
        let plan = FrozenPlan::load(&saved).unwrap();
        let report = mirror::fetch(
            &plan,
            &source,
            &FetchOptions {
                output,
                resume: false,
                previous: previous.clone(),
            },
            &Approval {
                plan_id: plan.plan_id.clone(),
            },
        )
        .unwrap();
        assert_eq!(report.transferred_bytes, 7);
        assert_eq!(report.reused_bytes, ((generation - 1) * 7) as u64);
        assert_eq!(source.calls.load(Ordering::SeqCst), before + 1);
        Inventory::load(&report.mirror).unwrap();
        previous = Some(report.mirror);
    }
    // Later metadata cannot mutate a previously approved checksum or feature graph.
    let old: Vec<_> = source
        .metadata
        .split_inclusive(|b| *b == b'\n')
        .map(|l| l.to_vec())
        .collect();
    let mut record: serde_json::Value = serde_json::from_slice(&old[0]).unwrap();
    record["cksum"] = json!("0".repeat(64));
    source.metadata = serde_json::to_vec(&record).unwrap();
    source.metadata.push(b'\n');
    for line in &old[1..] {
        source.metadata.extend(line);
    }
    assert!(mirror::plan(&catalog, &source, previous.as_deref(), false).is_err());
}

#[test]
fn crate_publish_refuses_a_different_base_before_remote_requests() {
    use mirror::{Approval, FetchOptions};
    let (catalog, source) = payload_fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let fetched = mirror::fetch(
        &plan,
        &source,
        &FetchOptions {
            output: directory.path().join("run"),
            resume: false,
            previous: None,
        },
        &Approval {
            plan_id: plan.plan_id.clone(),
        },
    )
    .unwrap();
    let options = rust_mirror::publish::Options {
        base_url: "https://127.0.0.1:1/wrong".into(),
        ca_file: None,
        credentials: None,
        allow_loopback_http: false,
    };
    let error = rust_mirror::publish::preflight(&fetched.mirror, &options)
        .err()
        .unwrap();
    assert_eq!(
        rust_mirror::error_kind(error.as_ref()),
        rust_mirror::ErrorKind::Invalid
    );
    assert!(error
        .to_string()
        .contains("https://mirror.example/generation-001"));
    assert!(error.to_string().contains("https://127.0.0.1:1/wrong"));
}

#[test]
fn cli_saved_preview_uses_protocol_one_without_network_or_payloads() {
    let (catalog, source) = fixture();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preview.json");
    plan.save_new(&path).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .args(["--events", "jsonl", "crates", "show", "--plan"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events[0]["type"], "hello");
    assert_eq!(events[0]["command"], "crates-show");
    assert_eq!(events[0]["protocol"], 1);
    assert_eq!(events.iter().filter(|e| e["type"] == "result").count(), 1);
    let report = &events.last().unwrap()["report"];
    assert_eq!(report["plan_id"], plan.plan_id);
    assert_eq!(
        report["versions"][0].get("expected_bytes"),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        report["size_estimate"].get("expected_archive_bytes"),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        report["versions"][0]["reasons"][0].get("parent"),
        Some(&serde_json::Value::Null)
    );
    assert!(report["previous_inventory_sha256"].is_null());
}

#[test]
fn semver_build_metadata_has_a_portable_archive_path_without_weakening_toolchain_paths() {
    use mirror::{Approval, FetchOptions, Inventory};
    let (mut catalog, mut source) = payload_fixture();
    let mut record: serde_json::Value = serde_json::from_slice(&source.metadata).unwrap();
    record["vers"] = json!("1.0.0+fixture.1");
    source.metadata = serde_json::to_vec(&record).unwrap();
    source.metadata.push(b'\n');
    catalog.crates.get_mut("x").unwrap().version = "=1.0.0+fixture.1".into();
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let report = mirror::fetch(
        &plan,
        &source,
        &FetchOptions {
            output: dir.path().join("run"),
            resume: false,
            previous: None,
        },
        &Approval {
            plan_id: plan.plan_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        std::fs::read(report.mirror.join("cargo/crates/x/x-1.0.0+fixture.1.crate")).unwrap(),
        source.payload
    );
    Inventory::load(&report.mirror).unwrap();
}

#[test]
fn crate_directory_prune_keeps_paused_previous_and_refuses_stale_disk() {
    use mirror::{Approval, FetchOptions, SyncOptions, SyncReport};
    use rust_mirror::crates::retention;
    let (mut catalog, source) = payload_fixture();
    catalog.crate_retention = Some(retention::Policy::default());
    let dir = tempfile::tempdir().unwrap();
    let mut mirrors = Vec::new();
    for index in 1..=3 {
        catalog.crate_output.base_url = format!("https://mirror.example/generation-{index}");
        let plan = mirror::plan(&catalog, &source, None, false).unwrap();
        let report = mirror::fetch(
            &plan,
            &source,
            &FetchOptions {
                output: dir.path().join(format!("run-{index}")),
                resume: false,
                previous: None,
            },
            &Approval {
                plan_id: plan.plan_id.clone(),
            },
        )
        .unwrap();
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1700000000 + index);
        std::fs::OpenOptions::new()
            .write(true)
            .open(report.mirror.join("snapshot.toml"))
            .unwrap()
            .set_modified(time)
            .unwrap();
        mirrors.push(report.mirror);
    }
    let paused = mirror::sync(
        &catalog,
        &source,
        &SyncOptions {
            output: dir.path().join("paused"),
            resume: false,
            previous: Some(mirrors[0].clone()),
            estimate_sizes: false,
        },
        None,
    )
    .unwrap();
    assert!(matches!(paused, SyncReport::NeedsApproval { .. }));
    let listing = rust_mirror::selection::retention::discover(dir.path()).unwrap();
    assert!(listing.issues.is_empty(), "{:?}", listing.issues);
    assert_eq!(listing.generations.len(), 3);
    assert!(listing
        .generations
        .iter()
        .all(|g| g.kind == "crates" && g.crate_count == Some(1) && g.version_count == Some(1)));
    assert!(listing.runs.iter().any(|r| r.kind == "crates"
        && r.state == rust_mirror::selection::retention::RunState::InProgress));
    let plan =
        retention::preview_directory(&catalog, dir.path(), &mirrors[2], "2026-11-13").unwrap();
    assert_eq!(
        plan.deletion.generations,
        [mirrors[1].canonicalize().unwrap()]
    );
    assert!(retention::apply(&plan, false).is_err());
    let changed = mirrors[1].join("cargo/crates/x/x-1.0.0.crate");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&changed)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .unwrap();
    assert!(retention::apply(&plan, true).is_err());
    assert!(mirrors[1].exists());
    let fresh =
        retention::preview_directory(&catalog, dir.path(), &mirrors[2], "2026-11-13").unwrap();
    let applied = retention::apply(&fresh, true).unwrap();
    assert!(applied.failure.is_none(), "{:?}", applied.failure);
    assert!(!mirrors[1].exists());
    assert!(mirrors[0].exists() && mirrors[2].exists());
    assert!(dir.path().join("run-2/.download/state.json").exists());
}

#[test]
fn custom_transfer_sources_cannot_bypass_the_file_budget() {
    use mirror::{Approval, FetchOptions};
    use sha2::{Digest, Sha256};
    let (mut catalog, mut source) = payload_fixture();
    source.payload = vec![42; 4096];
    let mut record: serde_json::Value = serde_json::from_slice(&source.metadata).unwrap();
    record["cksum"] = json!(hex::encode(Sha256::digest(&source.payload)));
    source.metadata = serde_json::to_vec(&record).unwrap();
    source.metadata.push(b'\n');
    catalog.crate_policy.max_file_bytes = 2048;
    let plan = mirror::plan(&catalog, &source, None, false).unwrap();
    assert!(plan.budgets.oversized_files.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("run");
    let error = mirror::fetch(
        &plan,
        &source,
        &FetchOptions {
            output: output.clone(),
            resume: false,
            previous: None,
        },
        &Approval {
            plan_id: plan.plan_id.clone(),
        },
    )
    .unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(error.as_ref()),
        rust_mirror::ErrorKind::Budget
    );
    assert!(!output.join("mirror").exists());
    let partial = std::fs::read_dir(output.join(".download/files"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(partial.metadata().unwrap().len(), 0);
}

#[test]
fn missing_root_features_and_transitive_failures_have_structured_rule_context() {
    let (mut catalog, mut source) = fixture();
    catalog
        .crates
        .get_mut("alias")
        .unwrap()
        .features
        .push("absent".into());
    let error = mirror::plan(&catalog, &source, None, false).unwrap_err();
    let report = error
        .downcast_ref::<rust_mirror::Error>()
        .unwrap()
        .report
        .as_ref()
        .unwrap();
    assert_eq!(report["alias"].as_str(), Some("alias"));
    assert_eq!(report["feature"].as_str(), Some("absent"));
    assert_eq!(report["kind"].as_str(), Some("missing_feature"));
    catalog.crates.get_mut("alias").unwrap().features.clear();
    catalog.crate_rules.clear();
    let mut root: serde_json::Value = serde_json::from_slice(
        source.files["x"]
            .split_inclusive(|b| *b == b'\n')
            .nth(1)
            .unwrap(),
    )
    .unwrap();
    root["deps"] = json!([{"name":"y","req":"1","features":["absent"]}]);
    source.files.insert(
        "x".into(),
        [serde_json::to_vec(&root).unwrap(), vec![b'\n']].concat(),
    );
    source.files.insert("y".into(), [serde_json::to_vec(&json!({"name":"y","vers":"1.0.0","deps":[],"features":{},"cksum":"0".repeat(64),"yanked":false})).unwrap(), vec![b'\n']].concat());
    let error = mirror::plan(&catalog, &source, None, false).unwrap_err();
    let report = error
        .downcast_ref::<rust_mirror::Error>()
        .unwrap()
        .report
        .as_ref()
        .unwrap();
    assert_eq!(report["alias"].as_str(), Some("alias"));
    assert_eq!(report["kind"].as_str(), Some("missing_feature"));
    assert_eq!(report["package"].as_str(), Some("y"));
    assert_eq!(report["requirement"].as_str(), Some("1"));
}
