use rust_mirror::{
    crates::{catalog::Catalog, lockfile::*},
    events::Control,
};
fn input(text: String) -> LockInput {
    LockInput {
        source_name: "service-a".into(),
        file_name: "Cargo.lock".into(),
        text,
    }
}
fn package(version: u32) -> String {
    format!("{}[[package]]\nname=\"itoa\"\nversion=\"1.0.15\"\nsource=\"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum=\"{}\"\n",if version>=3 {format!("version={version}\n")}else{String::new()},"a".repeat(64))
}
#[test]
fn inline_formats_and_transaction_roundtrip() {
    for version in [2, 3, 4] {
        let report = parse_with_control(&input(package(version)), &Control::default()).unwrap();
        assert!(report.accepted, "{:?}", report.diagnostics);
        assert_eq!(report.source.unwrap().lock_version, version);
    }
    let base = Catalog::parse("[crates]\nmanual=\"1\"\n").unwrap();
    let first = preview_import(&base, &[input(package(4))], 1, &Control::default()).unwrap();
    assert!(first.accepted, "{:?}", first.diagnostics);
    let c = first.catalog.unwrap();
    assert!(c.crates.contains_key("manual"));
    let c = Catalog::parse(&c.to_toml().unwrap()).unwrap();
    assert_eq!(c.crate_locks.len(), 1);
    let again = preview_import(&c, &[input(package(4))], 2, &Control::default()).unwrap();
    assert!(again.sources[0].added.is_empty());
    assert_eq!(again.sources[0].unchanged.len(), 1);
    let removed = preview_remove(&c, &["service-a".into()], &Control::default())
        .unwrap()
        .catalog
        .unwrap();
    assert!(removed.crate_locks.is_empty());
    assert!(removed.crates.contains_key("manual"));
}
#[test]
fn legacy_checksums_and_local_git_graph() {
    let text=format!("[root]\nname=\"app\"\nversion=\"0.1.0\"\ndependencies=[\"itoa 1.0.15 (registry+https://github.com/rust-lang/crates.io-index)\"]\n[[package]]\nname=\"itoa\"\nversion=\"1.0.15\"\nsource=\"registry+https://github.com/rust-lang/crates.io-index\"\n[metadata]\n\"checksum itoa 1.0.15 (registry+https://github.com/rust-lang/crates.io-index)\"=\"{}\"\n","b".repeat(64));
    let p = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(p.accepted, "{:?}", p.diagnostics);
    let s = p.source.unwrap();
    assert_eq!(s.lock_version, 1);
    assert_eq!(s.external_packages[0].dependencies[0].name, "itoa");
    assert_eq!(p.skipped.len(), 1);
    let text=package(4)+"[[package]]\nname=\"gitlib\"\nversion=\"0.1.0\"\nsource=\"git+https://secret:password@example.org/repo#revision\"\ndependencies=[\"itoa\"]\n";
    let p = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(p.accepted, "{:?}", p.diagnostics);
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("password"));
    assert_eq!(p.skipped[0].kind, SkipKind::Git);
}
#[test]
fn rejects_missing_checksum_alternates_and_bad_versions_with_positions() {
    let text = package(4).replace(&format!("checksum=\"{}\"\n", "a".repeat(64)), "");
    let p = parse_with_control(&input(text), &Control::default()).unwrap();
    assert_eq!(p.diagnostics[0].kind, DiagnosticKind::MissingChecksum);
    assert!(p.diagnostics[0].line.is_some());
    let text = package(4).replace(
        "registry+https://github.com/rust-lang/crates.io-index",
        "registry+https://user:SECRET@private.invalid/index",
    );
    let p = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(!p.accepted);
    assert_eq!(p.diagnostics[0].kind, DiagnosticKind::AlternateRegistry);
    assert!(!serde_json::to_string(&p).unwrap().contains("SECRET"));
    let p = parse_with_control(&input(package(5)), &Control::default()).unwrap();
    assert!(!p.accepted);
    assert_eq!(p.diagnostics[0].kind, DiagnosticKind::UnsupportedVersion);
}
#[test]
fn bounded_reader_and_names() {
    assert!(validate_source_name("project-a").is_ok());
    for bad in ["..", "a/b", "a b", ""] {
        assert!(validate_source_name(bad).is_err());
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("Cargo.lock");
    std::fs::write(&p, package(4)).unwrap();
    assert_eq!(read_input("p", &p).unwrap().text, package(4));
    std::fs::write(&p, [255]).unwrap();
    assert!(read_input("p", &p).is_err());
    #[cfg(unix)]
    {
        std::fs::remove_file(&p).unwrap();
        std::os::unix::fs::symlink(d.path(), &p).unwrap();
        assert!(read_input("p", &p).is_err());
    }
}

use rust_mirror::{
    crates::{mirror, source::CrateSource},
    Result, Transfer,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture {
    index: BTreeMap<String, Vec<u8>>,
    payload: Vec<u8>,
    gets: AtomicUsize,
}
impl CrateSource for Fixture {
    fn index(&self, n: &str, _: usize) -> Result<Vec<u8>> {
        Ok(self.index.get(n).ok_or("missing index")?.clone())
    }
    fn head(&self, _: &str, _: &str) -> Result<Option<u64>> {
        Ok(Some(self.payload.len() as u64))
    }
    fn archive(
        &self,
        _: &str,
        _: &str,
        out: &mut dyn Write,
        offset: u64,
        _: u64,
    ) -> Result<Transfer> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        out.write_all(&self.payload[offset as usize..])?;
        Ok(Transfer::Complete)
    }
}
fn locked_fixture() -> (Catalog, Fixture) {
    let payload = b"fixture archive bytes".to_vec();
    let hash = hex::encode(Sha256::digest(&payload));
    let lock=format!("version=4\n[[package]]\nname=\"app\"\nversion=\"0.1.0\"\ndependencies=[\"a\",\"dev\"]\n[[package]]\nname=\"a\"\nversion=\"1.0.0\"\nsource=\"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum=\"{hash}\"\n[[package]]\nname=\"dev\"\nversion=\"1.0.0\"\nsource=\"sparse+https://index.crates.io/\"\nchecksum=\"{hash}\"\n");
    let base=Catalog::parse("[crates]\n[crate-policy]\nhosts=[\"x86_64-unknown-linux-gnu\"]\ntargets=[\"x86_64-unknown-linux-gnu\"]\n[crate-output]\nbase-url=\"https://mirror.example/one\"\n").unwrap();
    let c = preview_import(&base, &[input(lock)], 1, &Control::default())
        .unwrap()
        .catalog
        .unwrap();
    let index=[("a",true),("dev",false)].into_iter().map(|(n,y)|{let row=serde_json::json!({"name":n,"vers":"1.0.0","cksum":hash,"deps":[],"features":{"default":["unselected"]},"yanked":y});(n.into(),(serde_json::to_string(&row).unwrap()+"\n").into_bytes())}).collect();
    (
        c,
        Fixture {
            index,
            payload,
            gets: AtomicUsize::new(0),
        },
    )
}
#[test]
fn exact_lock_plan_includes_dev_and_yanks_without_default_expansion() {
    let (c, source) = locked_fixture();
    let p = mirror::plan(&c, &source, None, true).unwrap();
    assert_eq!(p.schema, 2);
    assert_eq!(p.versions.len(), 2);
    assert!(p.roots.is_empty());
    assert_eq!(source.gets.load(Ordering::SeqCst), 0);
    let row = p.versions.iter().find(|r| r.name == "a").unwrap();
    assert!(row.yanked && row.locked_yanked);
    assert_eq!(row.sources[0].name, "service-a");
    let e = mirror::explain(&p, "a", None, 10).unwrap();
    assert_eq!(e.chains[0].source.as_ref().unwrap().name, "service-a");
    let dir = tempfile::tempdir().unwrap();
    p.save_new(&dir.path().join("frozen.json")).unwrap();
    let p = mirror::FrozenPlan::load(&dir.path().join("frozen.json")).unwrap();
    let report = mirror::fetch(
        &p,
        &source,
        &mirror::FetchOptions {
            output: dir.path().join("run"),
            resume: false,
            previous: None,
        },
        &mirror::Approval {
            plan_id: p.plan_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(mirror::Inventory::load(&report.mirror).unwrap().schema, 5);
    let mut manual = c.clone();
    manual.crates.insert(
        "a".into(),
        rust_mirror::crates::catalog::Dependency {
            version: "=1.0.0".into(),
            default_features: false,
            ..Default::default()
        },
    );
    assert!(mirror::plan(&manual, &source, None, false).is_err());
}
#[test]
fn index_mismatch_and_lock_edge_refusals_are_typed_and_archive_free() {
    let (mut c, mut s) = locked_fixture();
    c.crate_locks.get_mut("service-a").unwrap().pins[0].checksum = "0".repeat(64);
    let e = mirror::plan(&c, &s, None, false).unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(e.as_ref()),
        rust_mirror::ErrorKind::Integrity
    );
    let (c, _) = locked_fixture();
    let mut row: serde_json::Value = serde_json::from_slice(&s.index["a"]).unwrap();
    row["deps"] = serde_json::json!([{"name":"missing","req":"1","optional":false}]);
    s.index
        .insert("a".into(), (row.to_string() + "\n").into_bytes());
    let e = mirror::plan(&c, &s, None, false).unwrap_err();
    assert!(e.to_string().contains("consistency"));
    assert_eq!(s.gets.load(Ordering::SeqCst), 0);
}
#[test]
fn old_plan_identity_and_schema_four_payloads_remain_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.json");
    std::fs::write(&path, include_str!("fixtures/crates-plan-v1.json")).unwrap();
    let plan = mirror::FrozenPlan::load(&path).unwrap();
    assert_eq!(plan.schema, 1);
    assert_eq!(
        plan.plan_id,
        "905f2c167ff32f7f58d1c4cbf9da15272d78c06dbbabb9e5aefc91b8efe63f85"
    );
    let source = Fixture {
        index: BTreeMap::new(),
        payload: (0..1048576).map(|n| n as u8).collect(),
        gets: AtomicUsize::new(0),
    };
    let report = mirror::fetch(
        &plan,
        &source,
        &mirror::FetchOptions {
            output: dir.path().join("old-run"),
            resume: false,
            previous: None,
        },
        &mirror::Approval {
            plan_id: plan.plan_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(mirror::Inventory::load(&report.mirror).unwrap().schema, 4);
}
#[test]
fn lock_pin_protection_has_typed_reasons_and_stale_reasons_refuse() {
    use rust_mirror::crates::retention;
    let (mut c, s) = locked_fixture();
    c.crate_retention = Some(Default::default());
    let dir = tempfile::tempdir().unwrap();
    let p = mirror::plan(&c, &s, None, false).unwrap();
    let a = mirror::fetch(
        &p,
        &s,
        &mirror::FetchOptions {
            output: dir.path().join("one"),
            resume: false,
            previous: None,
        },
        &mirror::Approval {
            plan_id: p.plan_id.clone(),
        },
    )
    .unwrap();
    c.crate_output.base_url = "https://mirror.example/two".into();
    let p = mirror::plan(&c, &s, Some(&a.mirror), false).unwrap();
    let b = mirror::fetch(
        &p,
        &s,
        &mirror::FetchOptions {
            output: dir.path().join("two"),
            resume: false,
            previous: Some(a.mirror),
        },
        &mirror::Approval {
            plan_id: p.plan_id.clone(),
        },
    )
    .unwrap();
    let plan = retention::preview_directory(&c, dir.path(), &b.mirror, "2026-11-13").unwrap();
    assert!(plan
        .keep_sources
        .iter()
        .flat_map(|r| &r.sources)
        .any(|r| r.kind == mirror::SourceReasonKind::Lock
            && r.name == "service-a"
            && r.digest == Some(c.crate_locks["service-a"].sha256.clone())));
    let mut changed = plan.clone();
    changed.keep_sources.clear();
    assert!(retention::apply(&changed, true).is_err());
    assert!(retention::apply(&plan, true).unwrap().failure.is_none());
    assert!(retention::preview_directory(&c, dir.path(), &b.mirror, "2026-11-13").is_ok());
}

#[test]
fn conflicts_are_atomic_and_have_source_package_and_position() {
    let first = preview_import(
        &Catalog::default(),
        &[input(package(4))],
        1,
        &Control::default(),
    )
    .unwrap()
    .catalog
    .unwrap();
    let mut other = input(package(4).replace(&"a".repeat(64), &"b".repeat(64)));
    other.source_name = "service-b".into();
    let rejected = preview_import(&first, &[other], 2, &Control::default()).unwrap();
    assert!(!rejected.accepted && rejected.catalog.is_none());
    let d = &rejected.diagnostics[0];
    assert_eq!(d.kind, DiagnosticKind::ConflictingChecksum);
    assert_eq!(d.source_name, "service-b");
    assert_eq!(d.package.as_deref(), Some("itoa"));
    assert!(d.line.is_some() && d.column.is_some());
    assert_eq!(first.crate_locks.len(), 1);
}
#[test]
fn diagnostic_truncation_and_cancellation_are_explicit() {
    let p = parse_with_control(&input("[[package]]\n".repeat(1500)), &Control::default()).unwrap();
    assert!(!p.accepted && p.truncated);
    assert_eq!(p.diagnostics.len(), 1024);
    assert_eq!(p.total_diagnostics, 1500);
    let cancel = rust_mirror::events::CancelToken::new();
    cancel.cancel();
    let control = Control::new(
        std::sync::Arc::new(rust_mirror::events::JsonLines::new(Vec::<u8>::new())),
        cancel,
    );
    let e = preview_import(&Catalog::default(), &[input(package(4))], 1, &control).unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(e.as_ref()),
        rust_mirror::ErrorKind::Cancelled
    );
    let text = package(4).replace(
        "registry+https://github.com/rust-lang/crates.io-index",
        "registry+https://user:PRIVATE@private.invalid/index?token=TOKEN",
    );
    let data = input(text);
    assert!(!format!("{data:?}").contains("PRIVATE"));
    let p = parse_with_control(&data, &Control::default()).unwrap();
    assert_eq!(
        p.diagnostics[0].registry_origin.as_deref(),
        Some("https://private.invalid")
    );
    assert!(!serde_json::to_string(&p).unwrap().contains("TOKEN"));
}
#[test]
fn cli_import_preview_write_and_remove_share_library_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = dir.path().join("catalog.toml");
    let lock = dir.path().join("Cargo.lock");
    std::fs::write(&catalog, "[crates]\nmanual=\"1\"\n").unwrap();
    std::fs::write(&lock, package(4)).unwrap();
    let before = std::fs::read(&catalog).unwrap();
    let call = |verb: &str, args: Vec<std::ffi::OsString>| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
            .args(["--events", "jsonl", "crates", verb])
            .arg("--config")
            .arg(&catalog)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let rows: Vec<serde_json::Value> = String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(rows[0]["command"], format!("crates-{verb}"));
        rows.last().unwrap()["report"].clone()
    };
    let arg = format!("service={}", lock.display());
    let preview = call("import-lock", vec!["--lock".into(), arg.clone().into()]);
    assert_eq!(preview["written"], false);
    assert_eq!(std::fs::read(&catalog).unwrap(), before);
    let wrote = call(
        "import-lock",
        vec!["--lock".into(), arg.into(), "--write".into()],
    );
    assert_eq!(wrote["written"], true);
    std::fs::remove_file(&lock).unwrap();
    let typed = Catalog::parse(&std::fs::read_to_string(&catalog).unwrap()).unwrap();
    assert!(typed.crates.contains_key("manual") && typed.crate_locks.contains_key("service"));
    let removed = call(
        "remove-lock",
        vec!["--source".into(), "service".into(), "--write".into()],
    );
    assert_eq!(removed["sources"][0]["action"], "removed");
    assert!(Catalog::parse(&std::fs::read_to_string(&catalog).unwrap())
        .unwrap()
        .crates
        .contains_key("manual"));
}

#[test]
fn mixed_checksum_encodings_and_ambiguous_local_refs_refuse() {
    let text=package(4)+&format!("[metadata]\n\"checksum itoa 1.0.15 (registry+https://github.com/rust-lang/crates.io-index)\"=\"{}\"\n","a".repeat(64));
    assert!(
        !parse_with_control(&input(text), &Control::default())
            .unwrap()
            .accepted
    );
    let text=package(4)+"[[package]]\nname=\"itoa\"\nversion=\"2.0.0\"\n[[package]]\nname=\"app\"\nversion=\"1.0.0\"\ndependencies=[\"itoa\"]\n";
    let r = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(!r.accepted);
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::AmbiguousDependency));
    let text=format!("[[package]]\nname=\"itoa\"\nversion=\"1.0.15\"\nsource=\"sparse+https://index.crates.io/\"\n[metadata]\n\"checksum itoa 1.0.15 (registry+https://github.com/rust-lang/crates.io-index)\"=\"{}\"\n","a".repeat(64));
    let r = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(r.accepted, "{:?}", r.diagnostics);
}

#[test]
fn external_patched_edges_remain_external_without_closure_expansion() {
    let (mut c, mut source) = locked_fixture();
    let lock = c.crate_locks.get_mut("service-a").unwrap();
    let external = LockedIdentity {
        name: "patched".into(),
        version: "1.0.0".into(),
        source_kind: SourceKind::Git,
    };
    lock.external_packages.push(ExternalPackage {
        identity: external.clone(),
        dependencies: vec![],
    });
    lock.pins
        .iter_mut()
        .find(|p| p.name == "a")
        .unwrap()
        .dependencies
        .push(external);
    let mut row: serde_json::Value = serde_json::from_slice(&source.index["a"]).unwrap();
    row["deps"] = serde_json::json!([{"name":"patched","req":"1","optional":false}]);
    source
        .index
        .insert("a".into(), (row.to_string() + "\n").into_bytes());
    let p = mirror::plan(&c, &source, None, false).unwrap();
    assert_eq!(p.versions.len(), 2);
    assert!(!p.versions.iter().any(|v| v.name == "patched"));
}
#[test]
fn legacy_historical_cohort_digest_is_preserved() {
    let plan = mirror::FrozenPlan::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/crates-retained-plan-v1.json"
    )))
    .unwrap();
    assert_eq!(plan.schema, 1);
    assert_eq!(plan.cohorts.len(), 2);
    assert_eq!(plan.versions.len(), 1);
}

#[test]
fn malformed_names_and_unused_patch_values_do_not_echo_credentials() {
    let text = package(4).replace("name=\"itoa\"", "name=\"https://user:PRIVATE@host\"");
    let r = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(!r.accepted);
    assert!(!serde_json::to_string(&r).unwrap().contains("PRIVATE"));
    let text =
        package(4) + "[[patch.unused]]\nname=\"https://user:PRIVATE@host\"\nversion=\"password\"\n";
    let r = parse_with_control(&input(text), &Control::default()).unwrap();
    assert!(!r.accepted);
    assert!(!serde_json::to_string(&r).unwrap().contains("PRIVATE"));
}
