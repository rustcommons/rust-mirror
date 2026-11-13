mod common;
use common::{Fixture, HOST, ORIGIN};
use rust_mirror::selection::{collection::assemble, retention::*, Config, Selection};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
// A CLI fork can briefly inherit another test's locked descriptors before exec.
// CLI tests run exclusively; library tests may still run concurrently.
static PROCESS_GATE: std::sync::RwLock<()> = std::sync::RwLock::new(());
const DATE: &str = "2025-09-18";
const BUDGET: u64 = 10 * 1024 * 1024;
fn policy() -> Policy {
    Policy {
        pinned_releases: vec![],
        latest_stable: 0,
        nightly_days: 0,
        previous_generations: 0,
        keep_newest: 0,
        pinned_generations: vec![],
    }
}
fn make(root: &Path, name: &str, release: &str, date: &str, selector: &str) -> PathBuf {
    let mut f = Fixture::new();
    f.set_manifest(
        Fixture::manifest()
            .replace("1.90.0", release)
            .replace(DATE, date),
    );
    let channel = selector.split('-').next().unwrap();
    let metadata = if selector == channel {
        format!("{ORIGIN}dist/channel-rust-{channel}.toml")
    } else {
        format!("{ORIGIN}dist/{date}/channel-rust-{channel}.toml")
    };
    f.files = f
        .files
        .into_iter()
        .map(|(p, bytes)| {
            let path = match p.strip_prefix(&format!("{ORIGIN}dist/channel-rust-1.90.0.toml")) {
                Some(suffix) => format!("{metadata}{suffix}"),
                None => p.replace("1.90.0", release).replace(DATE, date),
            };
            (path, bytes)
        })
        .collect::<BTreeMap<_, _>>();
    let cfg = Config {
        schema: 1,
        rustup_version: "1.28.2".into(),
        max_download_bytes: BUDGET,
        retention: None,
        network: None,
        selections: vec![Selection {
            name: name.into(),
            toolchain: selector.into(),
            hosts: vec![HOST.into()],
            profile: "minimal".into(),
            components: vec![],
            targets: vec![],
        }],
    };
    let plan = root.join(format!("{name}-plan"));
    rust_mirror::selection::resolve(&cfg, name, &f)
        .unwrap()
        .save(&plan)
        .unwrap();
    rust_mirror::selection::fetch(&plan, &root.join(name), &f)
        .unwrap()
        .canonicalize()
        .unwrap()
}
fn sample(root: &Path, name: &str, version: &str) -> PathBuf {
    make(root, name, version, DATE, version)
}
fn roots(t: &Path) -> Vec<PathBuf> {
    vec![sample(t, "current", "1.90.0"), sample(t, "old", "1.89.0")]
}
fn inventory(root: &Path) -> Vec<u8> {
    fs::read(root.join("snapshot.toml")).unwrap()
}
#[test]
fn preview_is_read_only_exact_and_apply_requires_explicit_authority() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let bytes = inventory(&roots[0]);
    let old = inventory(&roots[1]);
    let plan = preview(&policy(), &roots, DATE).unwrap();
    assert_eq!(plan.generations, roots[1..]);
    assert_eq!(plan.files[0].path, roots[1].join("snapshot.toml"));
    assert_eq!(plan.files[0].bytes, old.len() as u64);
    assert_eq!(
        plan.total_bytes,
        plan.files.iter().map(|f| f.bytes).sum::<u64>()
    );
    assert!(!roots[0]
        .parent()
        .unwrap()
        .join(".rust-mirror-prune.lock")
        .exists());
    for file in &plan.files {
        assert_eq!(fs::metadata(&file.path).unwrap().len(), file.bytes);
    }
    assert!(apply(&plan, false).is_err());
    assert_eq!(inventory(&roots[1]), old);
    let report = apply(&plan, true).unwrap();
    assert_eq!(report.deleted_files, plan.files);
    assert_eq!(report.deleted_bytes, plan.total_bytes);
    assert_eq!(report.removed_generations, plan.generations);
    assert!(report.failure.is_none());
    assert!(!roots[1].exists());
    assert_eq!(inventory(&roots[0]), bytes);
    assert!(roots[1].parent().unwrap().is_dir());
    assert!(apply(&plan, true).is_err());
}
#[test]
fn rollback_and_pins_are_additive_and_missing_pins_refuse() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let mut p = policy();
    p.previous_generations = 1;
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
    p.previous_generations = 0;
    p.pinned_releases = vec!["1.89.0".into()];
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
    p.pinned_releases = vec!["1.88.0".into()];
    assert!(preview(&p, &roots, DATE).is_err());
    assert!(roots[1].exists());
}
#[test]
fn latest_stable_is_numeric_not_lexical_or_generation_order() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = vec![
        sample(t.path(), "current", "1.9.0"),
        sample(t.path(), "new", "1.100.0"),
        sample(t.path(), "old", "1.99.0"),
    ];
    let mut p = policy();
    p.latest_stable = 1;
    let plan = preview(&p, &roots, DATE).unwrap();
    assert_eq!(plan.generations, roots[2..]);
    p.latest_stable = 2;
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
}
#[test]
fn nightlies_use_calendar_window_and_dated_pins() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let current = sample(t.path(), "current", "1.90.0");
    let a = make(
        t.path(),
        "today",
        "1.91.0-nightly",
        DATE,
        "nightly-2025-09-18",
    );
    let b = make(
        t.path(),
        "yesterday",
        "1.91.0-nightly",
        "2025-09-17",
        "nightly-2025-09-17",
    );
    let c = make(
        t.path(),
        "older",
        "1.91.0-nightly",
        "2025-09-16",
        "nightly-2025-09-16",
    );
    let roots = vec![current, a, b, c];
    let mut p = policy();
    p.nightly_days = 2;
    assert_eq!(preview(&p, &roots, DATE).unwrap().generations, roots[3..]);
    p.nightly_days = 0;
    p.pinned_releases = vec!["nightly-2025-09-16".into()];
    assert_eq!(preview(&p, &roots, DATE).unwrap().generations, roots[1..3]);
    assert!(preview(&p, &roots, "2025-09-17").is_err());
}
#[test]
fn calendar_boundaries_including_leap_day() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let root = make(
        t.path(),
        "nightly",
        "1.91.0-nightly",
        "2024-02-29",
        "nightly-2024-02-29",
    );
    let current = make(t.path(), "current", "1.90.0", "2024-03-01", "1.90.0");
    let mut p = policy();
    p.nightly_days = 2;
    assert!(preview(&p, &[current.clone(), root.clone()], "2024-03-01")
        .unwrap()
        .files
        .is_empty());
    assert_eq!(
        preview(&p, &[current, root], "2024-03-02")
            .unwrap()
            .generations
            .len(),
        1
    );
    for date in [
        "2025-02-29",
        "2024-02-30",
        "2025-13-01",
        "2025-01-00",
        "no-date",
    ] {
        assert!(preview(&p, &[], date).is_err());
    }
}
#[test]
fn newest_equivalent_copy_satisfies_protection_but_different_components_survive() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0");
    let b = sample(t.path(), "b", "1.90.0");
    let mut p = policy();
    p.latest_stable = 1;
    let roots = vec![a, b];
    assert_eq!(preview(&p, &roots, DATE).unwrap().generations, roots[1..]);
    // Explicit redundant cargo is a distinct declared component choice.
    let path = roots[1].join("snapshot.toml");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("components = []", "components = [\"cargo\"]");
    fs::write(path, text).unwrap();
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
}
#[test]
fn schema_three_and_two_are_supported_without_mutating_collections() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0");
    let b = sample(t.path(), "b", "1.89.0");
    let collection = assemble(
        &[a.clone(), b.clone()],
        None,
        &t.path().join("combined"),
        BUDGET,
    )
    .unwrap();
    let before = inventory(&collection);
    let mut p = policy();
    p.pinned_releases = vec!["1.89.0".into()];
    let plan = preview(&p, &[collection.clone(), a, b], DATE).unwrap();
    assert_eq!(plan.generations.len(), 2);
    assert!(apply(&plan, true).unwrap().failure.is_none());
    assert_eq!(inventory(&collection), before);
}
#[test]
fn stale_inventory_kept_generation_and_modified_report_refuse_before_deleting() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    let mut tampered = plan.clone();
    tampered.files.clear();
    assert!(apply(&tampered, true).is_err());
    let path = roots[0].join("snapshot.toml");
    let before = inventory(&roots[0]);
    let mut changed = before.clone();
    changed.push(b'\n');
    fs::write(&path, changed).unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
    fs::write(&path, before).unwrap();
    let file = &plan.files[1].path;
    let before = fs::read(file).unwrap();
    fs::write(file, b"corrupt").unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
    fs::write(file, before).unwrap();
    fs::write(roots[1].join("unrelated"), b"preserve").unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
}
#[test]
fn duplicates_overlaps_traversal_missing_and_empty_inputs_refuse() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    for paths in [
        vec![],
        vec![roots[0].clone(), roots[0].clone()],
        vec![roots[0].clone(), roots[0].join("dist")],
        vec![t.path().join("missing")],
        vec![PathBuf::from(format!(
            "{}{}dist{}..",
            roots[0].display(),
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        ))],
    ] {
        assert!(preview(&policy(), &paths, DATE).is_err());
    }
    let file = roots[0].join("snapshot.toml");
    assert!(preview(&policy(), &[file], DATE).is_err());
}
#[test]
fn active_sync_fetch_and_prune_locks_refuse_then_allow_retry() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    for name in [".sync.lock", ".fetch.lock", ".rust-mirror-prune.lock"] {
        let path = roots[1].parent().unwrap().join(name);
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        lock.try_lock().unwrap();
        assert!(apply(&plan, true).is_err());
        assert!(roots[1].exists());
        drop(lock);
        if name == ".sync.lock" {
            fs::remove_file(path).unwrap();
        }
    }
    assert!(apply(&plan, true).unwrap().failure.is_none());
}
#[test]
fn policy_config_requires_explicit_fields_and_has_bounds() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    Config::parse(include_str!("../examples/retention.toml")).unwrap();
    let base = include_str!("../examples/mirror.toml");
    let cfg = Config::parse(base).unwrap();
    assert!(cfg.retention.is_none());
    let text=format!("{base}\n[retention]\npinned_releases = []\nlatest_stable = 2\nnightly_days = 7\nprevious_generations = 1\n");
    assert!(Config::parse(&text).unwrap().retention.is_some());
    for changed in [
        text.replace("latest_stable = 2", "latest_stable = 257"),
        text.replace("nightly_days = 7", "nightly_days = -1"),
        text.replace("previous_generations = 1", "previous_generations = 32"),
        text.replace("pinned_releases = []", "pinned_releases = [\"stable\"]"),
        text.replace("nightly_days = 7\n", ""),
        text.clone() + "unknown = 1\n",
    ] {
        assert!(Config::parse(&changed).is_err());
    }
    let mut p = policy();
    p.pinned_releases = vec!["1.90.0".into(), "1.90.0".into()];
    assert!(p.validate().is_err());
}
#[test]
fn cli_defaults_to_preview_and_reports_structured_apply() {
    let _process_gate = PROCESS_GATE.write().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let config = t.path().join("config.toml");
    fs::write(&config,format!("{}\n[retention]\npinned_releases=[]\nlatest_stable=0\nnightly_days=0\nprevious_generations=0\n",include_str!("../examples/mirror.toml"))).unwrap();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
            .arg("prune")
            .arg("--config")
            .arg(&config)
            .args(["--as-of", DATE, "--generation"])
            .arg(&roots[0])
            .arg("--generation")
            .arg(&roots[1])
            .args(extra)
            .output()
            .unwrap()
    };
    for flags in [&[][..], &["--dry-run"][..]] {
        let out = run(flags);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: toml::Value =
            toml::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
        assert!(report["total_bytes"].as_integer().unwrap() > 0);
        assert!(roots[1].exists());
    }
    assert!(!run(&["--apply", "--dry-run"]).status.success());
    let out = run(&["--apply"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!roots[1].exists());
    let report: toml::Value = toml::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
    assert!(report["deleted_bytes"].as_integer().unwrap() > 0);
}
#[cfg(unix)]
#[test]
fn linked_ancestor_payload_and_lock_refuse_without_touching_target() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    let link = t.path().join("link");
    symlink(roots[1].parent().unwrap(), &link).unwrap();
    assert!(preview(&policy(), &[roots[0].clone(), link.join("mirror")], DATE).is_err());
    let lock = roots[1].parent().unwrap().join(".rust-mirror-prune.lock");
    symlink(roots[0].join("snapshot.toml"), &lock).unwrap();
    assert!(apply(&plan, true).is_err());
    fs::remove_file(lock).unwrap();
    let file = &plan.files[1].path;
    fs::remove_file(file).unwrap();
    symlink(roots[0].join("snapshot.toml"), file).unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
}

#[test]
fn beta_is_only_protected_by_explicit_pin_or_rollback() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = vec![
        sample(t.path(), "current", "1.90.0"),
        make(t.path(), "beta", "1.91.0-beta", DATE, "beta-2025-09-18"),
    ];
    let mut p = policy();
    p.latest_stable = 256;
    p.nightly_days = 36_600;
    assert_eq!(preview(&p, &roots, DATE).unwrap().generations, roots[1..]);
    p.pinned_releases = vec!["beta-2025-09-18".into()];
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
}

#[test]
fn unexpected_empty_directory_and_legacy_schema_refuse() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    fs::create_dir(roots[1].join("unrelated-directory")).unwrap();
    assert!(preview(&policy(), &roots, DATE).is_err());
    fs::remove_dir(roots[1].join("unrelated-directory")).unwrap();
    let inv = roots[1].join("snapshot.toml");
    fs::write(
        &inv,
        fs::read_to_string(&inv)
            .unwrap()
            .replace("schema = 2", "schema = 1"),
    )
    .unwrap();
    assert!(preview(&policy(), &roots, DATE).is_err());
}

#[cfg(unix)]
#[test]
fn partial_io_failure_reports_successful_prefix_and_preserves_current() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    let dir = roots[1].join("dist");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let report = apply(&plan, true).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(report.failure.is_some());
    assert!(!report.deleted_files.is_empty());
    assert_eq!(report.deleted_files[0], plan.files[0]);
    assert!(report.removed_generations.is_empty());
    assert!(roots[0].join("snapshot.toml").exists());
    assert!(!roots[1].join("snapshot.toml").exists());
    assert!(preview(&policy(), &roots, DATE).is_err());
}

#[test]
fn real_metadata_shapes_replay_through_local_policy_without_live_network() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    #[derive(serde::Deserialize)]
    struct Corpus {
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        selector: String,
        status: String,
        date: Option<String>,
        release: Option<String>,
    }
    let corpus: Corpus = toml::from_str(include_str!(
        "../docs/evals/2026-11-11-retention-metadata.toml"
    ))
    .unwrap();
    let mut checked = 0;
    for case in corpus.cases.into_iter().filter(|c| c.status == "read") {
        let t = tempfile::tempdir().unwrap();
        let roots = vec![
            make(t.path(), "current", "1.100.0", "2025-09-22", "1.100.0"),
            make(
                t.path(),
                "historical",
                case.release.as_deref().unwrap(),
                case.date.as_deref().unwrap(),
                &case.selector,
            ),
        ];
        assert_eq!(
            preview(&policy(), &roots, "2025-09-22")
                .unwrap()
                .generations,
            roots[1..]
        );
        let mut p = policy();
        p.pinned_releases = vec![case.selector.clone()];
        assert!(preview(&p, &roots, "2025-09-22").unwrap().files.is_empty());
        p.pinned_releases.clear();
        p.latest_stable = 1;
        p.nightly_days = 3;
        let expected_kept =
            case.selector.starts_with("nightly-") && case.date.as_deref().unwrap() >= "2025-09-20";
        assert_eq!(
            preview(&p, &roots, "2025-09-22").unwrap().files.is_empty(),
            expected_kept,
            "{}",
            case.selector
        );
        checked += 1;
    }
    assert_eq!(checked, 17);
}

#[test]
fn optional_numeric_lookup_capabilities_are_not_lost_to_raw_equivalent() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = vec![
        sample(t.path(), "raw", "1.90.0"),
        sample(t.path(), "lookup", "1.90.0"),
    ];
    let mut snapshot: rust_mirror::selection::SelectionSnapshot =
        toml::from_str(std::str::from_utf8(&inventory(&roots[1])).unwrap()).unwrap();
    for suffix in ["", ".sha256", ".asc"] {
        let bytes =
            fs::read(roots[1].join(format!("dist/channel-rust-1.90.0.toml{suffix}"))).unwrap();
        let path = format!("dist/channel-rust-1.90.toml{suffix}");
        fs::write(roots[1].join(&path), &bytes).unwrap();
        snapshot.files.push(rust_mirror::SnapshotFile {
            path,
            sha256: common::hash(&bytes),
            bytes: bytes.len() as u64,
        });
    }
    let bytes = b"schema = 1\n[aliases]\n\"1.90\" = \"1.90.0\"\n";
    let path = "rustup/toolchain-aliases.toml";
    fs::write(roots[1].join(path), bytes).unwrap();
    snapshot.files.push(rust_mirror::SnapshotFile {
        path: path.into(),
        sha256: common::hash(bytes),
        bytes: bytes.len() as u64,
    });
    fs::write(
        roots[1].join("snapshot.toml"),
        toml::to_string_pretty(&snapshot).unwrap(),
    )
    .unwrap();
    let mut p = policy();
    p.pinned_releases = vec!["1.90.0".into()];
    assert!(preview(&p, &roots, DATE).unwrap().files.is_empty());
}

#[test]
fn malformed_lock_and_root_count_limit_refuse() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    let lock = roots[0].parent().unwrap().join(".sync.lock");
    fs::write(&lock, b"not an empty lock").unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
    assert!(preview(&policy(), &vec![roots[0].clone(); 33], DATE).is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_root_refuses_before_any_apply() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    use std::os::unix::ffi::OsStringExt;
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let target = roots[1]
        .parent()
        .unwrap()
        .join(std::ffi::OsString::from_vec(vec![b'm', 0xff]));
    fs::rename(&roots[1], &target).unwrap();
    assert!(preview(&policy(), &[roots[0].clone(), target.clone()], DATE).is_err());
    assert!(target.join("snapshot.toml").exists());
}

#[cfg(windows)]
#[test]
fn windows_open_file_refusal_reports_partial_deletion() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    use std::os::windows::fs::OpenOptionsExt;
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    // FILE_SHARE_READ allows preflight/hash reads, while refusing unlink.
    let file = &plan.files[1].path;
    let handle = fs::File::options()
        .read(true)
        .share_mode(1)
        .open(file)
        .unwrap();
    let report = apply(&plan, true).unwrap();
    drop(handle);
    assert_eq!(report.failure.as_ref().unwrap().path, *file);
    assert_eq!(report.deleted_files, plan.files[..1]);
    assert!(report.removed_generations.is_empty());
    assert!(roots[0].join("snapshot.toml").is_file());
    assert!(!roots[1].join("snapshot.toml").exists());
    assert!(file.is_file());
}

fn paused(store: &Path, name: &str, previous: Option<&str>) -> PathBuf {
    let run = store.join(name);
    fs::create_dir(&run).unwrap();
    fs::create_dir(run.join(".sync")).unwrap();
    fs::write(run.join(".sync.lock"), b"").unwrap();
    let mut state = format!(
        "schema = 1\nconfig_sha256 = \"{}\"\nplans = [\"{}\"]\n",
        "0".repeat(64),
        "1".repeat(64)
    );
    if let Some(previous) = previous {
        state += &format!("previous_sha256 = \"{previous}\"\n");
    }
    fs::write(run.join(".sync/state.toml"), state).unwrap();
    run.canonicalize().unwrap()
}
#[test]
fn discovery_lists_generations_completed_sync_and_paused_runs_without_writes() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let store = t.path();
    let old = sample(store, "old", "1.89.0");
    let cfg = Config::parse(include_str!("../examples/retention.toml")).unwrap();
    // Same synthetic release and bootstrap as Fixture; retain the actual sync layout.
    let mut cfg = cfg;
    cfg.rustup_version = "1.28.2".into();
    let complete =
        rust_mirror::selection::sync::run(&cfg, None, &store.join("sync"), false, &Fixture::new())
            .unwrap();
    let interrupted = paused(store, "paused", Some(&common::hash(&inventory(&old))));
    let catalog = discover(store).unwrap();
    assert_eq!(catalog.generations.len(), 2);
    assert_eq!(
        catalog
            .runs
            .iter()
            .filter(|r| r.state == RunState::Completed)
            .count(),
        2
    );
    assert_eq!(
        catalog
            .runs
            .iter()
            .find(|r| r.path == interrupted)
            .unwrap()
            .state,
        RunState::InProgress
    );
    assert!(catalog
        .generations
        .iter()
        .any(|g| g.path == complete.mirror.canonicalize().unwrap()));
    assert!(!store.join("old/.rust-mirror-prune.lock").exists());
    let text = toml::to_string_pretty(&catalog).unwrap();
    assert!(text.contains("in_progress"));
}
#[test]
fn directory_preview_protects_next_previous_paused_dependencies_pins_and_newest() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    use std::time::{Duration, UNIX_EPOCH};
    let t = tempfile::tempdir().unwrap();
    let store = t.path();
    let roots: Vec<_> = (0..4)
        .map(|i| sample(store, &format!("run-{i}"), &format!("1.{}.0", 86 + i)))
        .collect();
    for (i, root) in roots.iter().enumerate() {
        fs::File::options()
            .write(true)
            .open(root.join("snapshot.toml"))
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i as u64)),
            )
            .unwrap();
    }
    let waiting = paused(store, "waiting", Some(&common::hash(&inventory(&roots[1]))));
    let mut p = policy();
    p.keep_newest = 1;
    let plan = preview_directory(&p, store, &roots[0], DATE).unwrap();
    assert_eq!(plan.generations, vec![roots[2].clone()]);
    assert_eq!(plan.folders[0].bytes, plan.total_bytes);
    assert_eq!(plan.folders[0].path, roots[2]);
    p.pinned_generations = vec![roots[2].clone()];
    assert!(preview_directory(&p, store, &roots[0], DATE)
        .unwrap()
        .files
        .is_empty());
    let state = fs::read(waiting.join(".sync/state.toml")).unwrap();
    let report = apply(&plan, true).unwrap();
    assert!(report.failure.is_none());
    assert!(roots[0].exists() && roots[1].exists() && roots[3].exists());
    assert!(!roots[2].exists());
    assert_eq!(fs::read(waiting.join(".sync/state.toml")).unwrap(), state);
    assert!(!waiting.join(".rust-mirror-prune.lock").exists());
}
#[test]
fn directory_apply_refuses_private_content_same_size_metadata_and_membership_changes() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    let private = roots[1].parent().unwrap().join(".download/state.toml");
    let bytes = fs::read(&private).unwrap();
    let original_time = fs::metadata(&private).unwrap().modified().unwrap();
    let mut changed = bytes.clone();
    changed[0] = if changed[0] == b'x' { b'y' } else { b'x' };
    fs::write(&private, &changed).unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").exists());
    fs::write(&private, bytes).unwrap();
    // Set a distinct timestamp explicitly; do not depend on filesystem clock granularity.
    fs::File::options()
        .write(true)
        .open(&private)
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(original_time + std::time::Duration::from_secs(1)),
        )
        .unwrap();
    assert!(apply(&plan, true).is_err());
    let plan = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    fs::create_dir(t.path().join("new-unmanaged-folder")).unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").exists());
}
#[test]
fn directory_refuses_locked_unknown_and_missing_previous_runs() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let waiting = paused(t.path(), "waiting", Some(&"2".repeat(64)));
    assert!(preview_directory(&policy(), t.path(), &roots[0], DATE).is_err());
    fs::write(waiting.join(".sync/state.toml"), b"malformed").unwrap();
    let catalog = discover(t.path()).unwrap();
    assert!(catalog.runs.iter().any(|r| r.state == RunState::Unknown));
    assert!(!catalog.issues.is_empty());
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(waiting.join(".sync.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(discover(t.path())
        .unwrap()
        .runs
        .iter()
        .any(|r| r.state == RunState::Locked));
    assert!(preview_directory(&policy(), t.path(), &roots[0], DATE).is_err());
    drop(lock);
    assert!(roots[1].exists());
}
#[test]
fn paused_run_with_completed_marker_is_protected_and_explicit_preview_refuses() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let parent = roots[1].parent().unwrap();
    fs::remove_file(parent.join(".fetch.lock")).unwrap();
    // Convert the completed fixture output into a sync layout with an uncommitted report.
    fs::remove_dir_all(parent.join(".download")).unwrap();
    fs::create_dir(parent.join(".sync")).unwrap();
    fs::write(parent.join(".sync.lock"), b"").unwrap();
    fs::write(
        parent.join(".sync/state.toml"),
        format!(
            "schema=1\nconfig_sha256=\"{}\"\nplans=[\"{}\"]\n",
            "0".repeat(64),
            "1".repeat(64)
        ),
    )
    .unwrap();
    assert!(preview(&policy(), &roots, DATE).is_err());
    let plan = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    assert!(plan.files.is_empty());
    assert!(apply(&plan, true).unwrap().failure.is_none());
    assert!(!parent.join(".rust-mirror-prune.lock").exists());
}
#[test]
fn cli_discovery_and_directory_preview_are_serializable() {
    let _process_gate = PROCESS_GATE.write().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let out = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .args(["generations", "--directory"])
        .arg(t.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let catalog: toml::Value = toml::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
    assert_eq!(catalog["generations"].as_array().unwrap().len(), 2);
    let cfg = t.path().join("config.toml");
    fs::write(&cfg,format!("{}\n[retention]\npinned_releases=[]\nlatest_stable=0\nnightly_days=0\nprevious_generations=0\nkeep_newest=1\npinned_generations=[]\n",include_str!("../examples/mirror.toml"))).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .arg("prune")
        .arg("--config")
        .arg(cfg)
        .args(["--as-of", DATE, "--directory"])
        .arg(t.path())
        .arg("--next-previous")
        .arg(&roots[0])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let plan: toml::Value = toml::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
    assert!(plan.get("catalog").is_some());
    assert!(roots[1].exists());
}

#[test]
fn directory_apply_refuses_new_locks_and_changed_paused_state_before_deleting() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let waiting = paused(t.path(), "waiting", None);
    let plan = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(waiting.join(".sync.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(apply(&plan, true).is_err());
    drop(lock);
    assert!(roots[1].join("snapshot.toml").is_file());
    let state = waiting.join(".sync/state.toml");
    let raw = fs::read_to_string(&state).unwrap();
    fs::write(&state, raw + "\n").unwrap();
    assert!(apply(&plan, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
}
#[test]
fn direct_snapshot_named_mirror_does_not_expand_discovery_to_unmanaged_trees() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let direct = t.path().join("mirror");
    fs::rename(&roots[1], &direct).unwrap();
    let unknown = t.path().join("unmanaged");
    fs::create_dir(&unknown).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&roots[0], unknown.join("do-not-follow")).unwrap();
    let plan = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    assert!(plan
        .catalog
        .as_ref()
        .unwrap()
        .generations
        .iter()
        .any(|g| g.path == direct.canonicalize().unwrap()));
    assert!(apply(&plan, true).unwrap().failure.is_none());
    assert!(unknown.is_dir());
}

#[test]
fn serde_roundtrip_plans_are_revalidated_and_require_explicit_authority() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    let plan = preview(&policy(), &roots, DATE).unwrap();
    let serialized = toml::to_string(&plan).unwrap();
    let loaded: PrunePlan = toml::from_str(&serialized).unwrap();
    assert_eq!(loaded, plan);
    assert!(apply(&loaded, false).is_err());
    let mut modified = loaded.clone();
    modified.schema = 2;
    assert!(apply(&modified, true).is_err());
    let mut modified = loaded.clone();
    modified.files.clear();
    assert!(apply(&modified, true).is_err());
    assert!(roots[1].join("snapshot.toml").is_file());
    let directory = preview_directory(&policy(), t.path(), &roots[0], DATE).unwrap();
    let roundtrip: PrunePlan = toml::from_str(&toml::to_string(&directory).unwrap()).unwrap();
    assert_eq!(directory, roundtrip);
    assert!(apply(&roundtrip, true).unwrap().failure.is_none());
}

#[test]
fn discovery_refuses_a_workspace_as_the_parent_directory() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let roots = roots(t.path());
    assert!(discover(roots[0].parent().unwrap()).is_err());
    assert!(roots[0].join("snapshot.toml").is_file());
    assert!(discover(&roots[0].parent().unwrap().join(".download")).is_err());
}
#[test]
fn explicit_pruning_refuses_private_working_snapshots() {
    let _process_gate = PROCESS_GATE.read().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    let current = sample(t.path(), "current", "1.90.0");
    for reserved in [".sync", ".download"] {
        let dir = t.path().join(reserved);
        fs::create_dir(&dir).unwrap();
        let private = sample(&dir, "cached", "1.89.0");
        assert!(preview(&policy(), &[current.clone(), private.clone()], DATE).is_err());
        assert!(private.join("snapshot.toml").is_file());
    }
}
