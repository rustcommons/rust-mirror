mod common;
use common::{Fixture, HOST};
use rust_mirror::{
    selection::{sync::run, Config, Selection},
    Result, Source, Transfer,
};
use std::{io::Write, path::Path, sync::Mutex};
fn config() -> Config {
    Config {
        schema: 1,
        retention: None,
        network: None,
        rustup_version: "1.28.2".into(),
        max_download_bytes: 10 * 1024 * 1024,
        selections: vec!["project-a", "project-b"]
            .into_iter()
            .map(|name| Selection {
                name: name.into(),
                toolchain: "1.90.0".into(),
                hosts: vec![HOST.into()],
                profile: "minimal".into(),
                components: vec![],
                targets: vec![],
            })
            .collect(),
    }
}
struct TestSource {
    fixture: Fixture,
    fail_on: usize,
    metadata_allowed: bool,
    payloads: Mutex<Vec<(String, u64)>>,
    metadata: Mutex<usize>,
}
impl Source for TestSource {
    fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
        *self.metadata.lock().unwrap() += 1;
        if !self.metadata_allowed {
            return Err("live metadata forbidden during resume".into());
        }
        self.fixture.fetch(url, out, limit)
    }
    fn fetch_from(
        &self,
        url: &str,
        out: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        let mut calls = self.payloads.lock().unwrap();
        calls.push((url.into(), offset));
        let data = self.fixture.files.get(url).ok_or("unknown payload")?;
        let body = &data[offset as usize..];
        if body.len() as u64 > limit {
            return Err("payload exceeds budget".into());
        }
        if calls.len() == self.fail_on {
            out.write_all(&body[..body.len() / 2])?;
            return Err("simulated interrupted transfer".into());
        }
        out.write_all(body)?;
        Ok(Transfer::Complete)
    }
}
fn source(fail: usize, metadata_allowed: bool) -> TestSource {
    TestSource {
        fixture: Fixture::new(),
        fail_on: fail,
        metadata_allowed,
        payloads: Mutex::new(vec![]),
        metadata: Mutex::new(0),
    }
}
#[test]
fn complete_sync_reuses_earlier_selection_then_noop_revalidates_previous() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let first = run(&cfg, None, &t.path().join("first"), false, &source(0, true)).unwrap();
    assert_eq!(first.report.status, "created");
    assert_eq!(first.report.selections, 2);
    assert!(first.report.upstream_payload_bytes > 0);
    assert!(first.report.local_payload_bytes > 0);
    assert_eq!(first.report.metadata_requests, 8);
    let none = t.path().join("unchanged");
    let next = source(1, true);
    let result = run(&cfg, Some(&first.mirror), &none, false, &next).unwrap();
    assert_eq!(result.report.status, "unchanged");
    assert_eq!(result.mirror, first.mirror);
    assert_eq!(result.report.upstream_payload_bytes, 0);
    assert_eq!(result.report.upstream_attempts, 0);
    assert!(!none.exists());
    assert!(next.payloads.lock().unwrap().is_empty());
}
#[test]
fn interrupted_sync_resumes_saved_cohort_without_live_metadata() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    let cfg = config();
    run(&cfg, None, &out, false, &source(2, true))
        .expect_err("setup sync should fail at the simulated interrupted transfer");
    assert!(!out.join("mirror").exists());
    let resumed = source(0, false);
    let result = run(&cfg, None, &out, true, &resumed).unwrap();
    assert!(result.report.resumed);
    assert_eq!(result.report.metadata_requests, 0);
    assert_eq!(*resumed.metadata.lock().unwrap(), 0);
    assert!(resumed.payloads.lock().unwrap()[0].1 > 0);
    let checked = run(&cfg, None, &out, true, &source(1, false)).unwrap();
    assert_eq!(checked.report.status, "validated");
    assert_eq!(checked.report.upstream_attempts, 0);
}
#[test]
fn changed_config_previous_or_plan_refuse_resume_before_payloads() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    let cfg = config();
    run(&cfg, None, &out, false, &source(2, true))
        .expect_err("setup sync should fail at the simulated interrupted transfer");
    let blocked = source(1, false);
    let mut changed = config();
    changed.max_download_bytes += 1;
    run(&changed, None, &out, true, &blocked)
        .expect_err("resume should refuse a changed byte budget");
    let previous = run(
        &cfg,
        None,
        &t.path().join("previous"),
        false,
        &source(0, true),
    )
    .unwrap();
    run(&cfg, Some(&previous.mirror), &out, true, &blocked)
        .expect_err("resume should refuse a different previous generation");
    std::fs::write(out.join(".sync/plans/0/manifest.asc"), b"changed").unwrap();
    run(&cfg, None, &out, true, &blocked).expect_err("resume should refuse a modified saved plan");
    assert!(blocked.payloads.lock().unwrap().is_empty());
}
#[test]
fn budgets_hold_across_selected_working_copies_and_previous_retention() {
    let t = tempfile::tempdir().unwrap();
    let mut single = config();
    single.selections.truncate(1);
    let baseline = run(
        &single,
        None,
        &t.path().join("one"),
        false,
        &source(0, true),
    )
    .unwrap();
    let mut small = config();
    small.max_download_bytes = baseline.report.recorded_bytes + baseline.report.recorded_bytes / 2;
    let out = t.path().join("limited");
    run(&small, None, &out, false, &source(0, true))
        .expect_err("sync should refuse working copies exceeding the byte budget");
    assert!(!out.join("mirror").exists());
    small.max_download_bytes = 1;
    let none = t.path().join("previous-too-big");
    run(
        &small,
        Some(&baseline.mirror),
        &none,
        false,
        &source(0, true),
    )
    .expect_err("sync should refuse a previous generation exceeding the byte budget");
    assert!(!none.exists());
}
#[test]
fn unknown_entries_locks_and_nested_outputs_refuse_without_touching_user_files() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    run(&cfg, None, &out, false, &source(2, true))
        .expect_err("setup sync should fail at the simulated interrupted transfer");
    std::fs::write(out.join("keep"), b"mine").unwrap();
    run(&cfg, None, &out, true, &source(0, false))
        .expect_err("resume should refuse an unknown output entry");
    assert_eq!(std::fs::read(out.join("keep")).unwrap(), b"mine");
    std::fs::remove_file(out.join("keep")).unwrap();
    let f = std::fs::File::options()
        .read(true)
        .write(true)
        .open(out.join(".sync.lock"))
        .unwrap();
    f.try_lock().unwrap();
    // Deterministically model an in-flight fork retaining the same Linux flock.
    #[cfg(target_os = "linux")]
    let inherited_description = f.try_clone().unwrap();
    let blocked = run(&cfg, None, &out, true, &source(0, false)).unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(blocked.as_ref()),
        rust_mirror::ErrorKind::Locked
    );
    // Closing our descriptor alone need not release a fork-inherited description.
    f.unlock().unwrap();
    drop(f);
    let completed = run(&cfg, None, &out, true, &source(0, false)).unwrap();
    #[cfg(target_os = "linux")]
    drop(inherited_description);
    let nested = completed.mirror.join("new-run");
    run(
        &cfg,
        Some(&completed.mirror),
        &nested,
        false,
        &source(0, true),
    )
    .expect_err("sync should refuse an output nested in the previous mirror");
    assert!(!nested.exists());
}
#[test]
fn corrupt_previous_and_completed_generations_cannot_certify_noop_or_be_overwritten() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    let done = run(&cfg, None, &out, false, &source(0, true)).unwrap();
    let file = done.mirror.join("dist/2025-09-18/cargo.tar.gz");
    std::fs::write(&file, b"damage").unwrap();
    let blocked = source(1, false);
    run(
        &cfg,
        Some(&done.mirror),
        &t.path().join("next"),
        false,
        &blocked,
    )
    .expect_err("sync should refuse a corrupt previous generation");
    assert_eq!(*blocked.metadata.lock().unwrap(), 0);
    run(&cfg, None, &out, true, &blocked)
        .expect_err("resume should refuse a corrupt completed generation");
    assert_eq!(std::fs::read(file).unwrap(), b"damage");
}
fn stable_fixture(version: &str, date: &str) -> Fixture {
    let mut f = Fixture::new();
    f.set_manifest(
        Fixture::manifest()
            .replace("1.90.0", version)
            .replace("2025-09-18", date),
    );
    f.files = f
        .files
        .into_iter()
        .map(|(k, v)| {
            (
                k.replace("channel-rust-1.90.0", "channel-rust-stable")
                    .replace("2025-09-18", date),
                v,
            )
        })
        .collect();
    f
}
#[test]
fn channel_refresh_retains_old_release_and_uses_verified_identical_content() {
    let t = tempfile::tempdir().unwrap();
    let mut cfg = config();
    cfg.selections.truncate(1);
    cfg.selections[0].toolchain = "stable".into();
    let first = run(
        &cfg,
        None,
        &t.path().join("first"),
        false,
        &stable_fixture("1.90.0", "2025-09-18"),
    )
    .unwrap();
    let next = run(
        &cfg,
        Some(&first.mirror),
        &t.path().join("next"),
        false,
        &stable_fixture("1.91.0", "2025-10-30"),
    )
    .unwrap();
    assert_eq!(next.report.status, "created");
    assert_eq!(next.report.upstream_payload_bytes, 0);
    assert_eq!(next.report.frozen[0].release, "1.91.0");
    assert!(next
        .mirror
        .join("dist/2025-09-18/channel-rust-stable.toml")
        .exists());
    assert!(next
        .mirror
        .join("dist/2025-10-30/channel-rust-stable.toml")
        .exists());
}
#[test]
fn incomplete_assembly_is_rebuilt_but_a_foreign_staging_file_is_preserved() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    let done = run(&cfg, None, &out, false, &source(0, true)).unwrap();
    std::fs::rename(&done.mirror, out.join(".sync/tree")).unwrap();
    std::fs::write(out.join(".sync/tree/foreign"), b"keep").unwrap();
    run(&cfg, None, &out, true, &source(0, false))
        .expect_err("resume should preserve and refuse a foreign staging file");
    assert_eq!(
        std::fs::read(out.join(".sync/tree/foreign")).unwrap(),
        b"keep"
    );
    std::fs::remove_file(out.join(".sync/tree/foreign")).unwrap();
    run(&cfg, None, &out, true, &source(0, false)).expect(
        "resume should rebuild the incomplete assembly after the foreign staging file is removed",
    );
}
#[cfg(unix)]
#[test]
fn workspace_links_and_fifos_refuse_before_opening() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    run(&cfg, None, &out, false, &source(2, true))
        .expect_err("setup sync should fail at the simulated interrupted transfer");
    let state = out.join(".sync/state.toml");
    let saved = t.path().join("saved");
    std::fs::rename(&state, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &state).unwrap();
    run(&cfg, None, &out, true, &source(0, false))
        .expect_err("resume should refuse a symlinked state file");
    std::fs::remove_file(&state).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(&state)
        .status()
        .unwrap()
        .success());
    run(&cfg, None, &out, true, &source(0, false))
        .expect_err("resume should refuse a FIFO state file");
}
#[test]
fn demo_tiny_sync() {
    let Some(root) = std::env::var_os("RUST_MIRROR_DEMO_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    let cfg = config();
    let first = run(&cfg, None, &root.join("first"), false, &source(0, true)).unwrap();
    println!(
        "FIRST RUN\n{}",
        toml::to_string_pretty(&first.report).unwrap()
    );
    run(
        &cfg,
        None,
        &root.join("interrupted"),
        false,
        &source(2, true),
    )
    .expect_err("demo sync should fail at the simulated interrupted transfer");
    println!("INTERRUPTED: no completed mirror exposed");
    let refused = run(
        &cfg,
        None,
        &root.join("interrupted"),
        false,
        &source(0, false),
    )
    .unwrap_err();
    println!("WITHOUT --resume/--force: ERROR: {refused}");
    let resumed = run(
        &cfg,
        None,
        &root.join("interrupted"),
        true,
        &source(0, false),
    )
    .unwrap();
    println!(
        "RESUMED\n{}",
        toml::to_string_pretty(&resumed.report).unwrap()
    );
    let next = run(
        &cfg,
        Some(&first.mirror),
        &root.join("unchanged"),
        false,
        &source(1, true),
    )
    .unwrap();
    println!(
        "UNCHANGED\n{}",
        toml::to_string_pretty(&next.report).unwrap()
    );
    let forced = rust_mirror::selection::sync::run_forced(
        &cfg,
        None,
        &root.join("interrupted"),
        &source(0, true),
    )
    .unwrap();
    println!(
        "FORCED FRESH RUN (old progress and mirror discarded, no backup)\n{}",
        toml::to_string_pretty(&forced.report).unwrap()
    );
}

#[test]
fn incoherent_channel_resolutions_refuse_before_payloads_or_output_creation() {
    struct Moving {
        sources: [Fixture; 2],
        active: Mutex<usize>,
        resolutions: Mutex<usize>,
        payload: Mutex<usize>,
    }
    impl Source for Moving {
        fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
            if url.ends_with(".toml.sha256") {
                let mut n = self.resolutions.lock().unwrap();
                *self.active.lock().unwrap() = (*n).min(1);
                *n += 1;
            }
            self.sources[*self.active.lock().unwrap()].fetch(url, out, limit)
        }
        fn fetch_from(
            &self,
            _url: &str,
            _out: &mut dyn Write,
            _offset: u64,
            _limit: u64,
        ) -> Result<Transfer> {
            *self.payload.lock().unwrap() += 1;
            Err("payload must not be requested".into())
        }
    }
    let s = Moving {
        sources: [
            stable_fixture("1.90.0", "2025-09-18"),
            stable_fixture("1.91.0", "2025-10-30"),
        ],
        active: Mutex::new(0),
        resolutions: Mutex::new(0),
        payload: Mutex::new(0),
    };
    let t = tempfile::tempdir().unwrap();
    let mut cfg = config();
    for selection in &mut cfg.selections {
        selection.toolchain = "stable".into();
    }
    let out = t.path().join("run");
    run(&cfg, None, &out, false, &s)
        .expect_err("sync should refuse incoherent channel resolutions before fetching payloads");
    assert_eq!(*s.payload.lock().unwrap(), 0);
    assert!(!out.exists());
}
#[test]
fn reports_are_replaced_after_validation_and_do_not_certify_output() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    run(&cfg, None, &out, false, &source(0, true)).unwrap();
    let report = out.join(".sync/report.toml");
    std::fs::write(&report, b"incomplete previous report").unwrap();
    std::fs::write(out.join(".sync/report.tmp"), b"interrupted replacement").unwrap();
    let done = run(&cfg, None, &out, true, &source(0, false)).unwrap();
    assert_eq!(done.report.status, "validated");
    let body = std::fs::read_to_string(report).unwrap();
    assert!(body.contains("validated"));
    assert!(!out.join(".sync/report.tmp").exists());
}

#[test]
fn resume_does_not_initialize_an_empty_unrelated_directory() {
    let t = tempfile::tempdir().unwrap();
    let empty = t.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    run(&config(), None, &empty, true, &source(0, false))
        .expect_err("resume should refuse an unrelated empty directory");
    assert!(std::fs::read_dir(empty).unwrap().next().is_none());
}
#[test]
fn sync_crash_child() {
    let Some(root) = std::env::var_os("RUST_MIRROR_SYNC_CHILD_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    struct Blocking {
        fixture: Fixture,
        ready: std::path::PathBuf,
    }
    impl Source for Blocking {
        fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
            self.fixture.fetch(url, out, limit)
        }
        fn fetch_from(
            &self,
            url: &str,
            out: &mut dyn Write,
            _offset: u64,
            _limit: u64,
        ) -> Result<Transfer> {
            let data = &self.fixture.files[url];
            out.write_all(&data[..data.len() / 2])?;
            std::fs::write(&self.ready, b"partial")?;
            loop {
                std::thread::park();
            }
        }
    }
    let _ = run(
        &config(),
        None,
        &root.join("run"),
        false,
        &Blocking {
            fixture: Fixture::new(),
            ready: root.join("ready"),
        },
    );
    panic!("expected parent termination");
}
#[test]
fn killed_sync_releases_both_locks_and_resumes_without_metadata_refresh() {
    let t = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "sync_crash_child", "--nocapture"])
        .env("RUST_MIRROR_SYNC_CHILD_ROOT", t.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !t.path().join("ready").exists() && std::time::Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let ready = t.path().join("ready").exists();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(ready, "child did not reach payload phase");
    let out = t.path().join("run");
    assert!(!out.join("mirror").exists());
    let result = run(&config(), None, &out, true, &source(0, false)).unwrap();
    assert!(result.report.resumed);
    assert_eq!(result.report.metadata_requests, 0);
}

#[test]
fn force_discards_interrupted_and_completed_runs_and_resolves_fresh() {
    use rust_mirror::selection::sync::run_forced;
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    let cfg = config();
    run(&cfg, None, &out, false, &source(2, true))
        .expect_err("setup sync should fail at the simulated interrupted transfer");
    let forbidden = source(0, false);
    let err = run(&cfg, None, &out, false, &forbidden)
        .unwrap_err()
        .to_string();
    assert!(err.contains("--resume") && err.contains("--force"));
    assert_eq!(*forbidden.metadata.lock().unwrap(), 0);
    std::fs::write(out.join(".sync/obsolete"), b"discard me").unwrap();
    let result = run_forced(&cfg, None, &out, &source(0, true)).unwrap();
    assert_eq!(result.report.status, "created");
    assert!(!result.report.resumed);
    assert_eq!(result.report.metadata_requests, 8);
    assert!(!out.join(".sync/obsolete").exists());
    std::fs::write(out.join("mirror/obsolete"), b"discard me too").unwrap();
    run_forced(&cfg, None, &out, &source(0, true)).unwrap();
    assert!(!out.join("mirror/obsolete").exists());
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 1, "no backup");
}
#[test]
fn force_refuses_active_unmanaged_or_overlapping_runs_without_changes() {
    use rust_mirror::selection::sync::run_forced;
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    let cfg = config();
    let done = run(&cfg, None, &out, false, &source(0, true)).unwrap();
    let inventory = std::fs::read(done.mirror.join("snapshot.toml")).unwrap();
    let lock = std::fs::File::options()
        .read(true)
        .write(true)
        .open(out.join(".sync.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    run_forced(&cfg, None, &out, &source(0, false))
        .expect_err("force should refuse an actively locked run");
    lock.unlock().unwrap();
    drop(lock);
    run_forced(&cfg, Some(&done.mirror), &out, &source(0, false))
        .expect_err("force should refuse reusing its own mirror as the previous generation");
    run_forced(
        &cfg,
        Some(&done.mirror),
        &done.mirror.join("nested"),
        &source(0, false),
    )
    .expect_err("force should refuse an output nested in the previous mirror");
    std::fs::write(out.join("mine"), b"keep").unwrap();
    run_forced(&cfg, None, &out, &source(0, false))
        .expect_err("force should preserve and refuse an unmanaged output entry");
    assert_eq!(std::fs::read(out.join("mine")).unwrap(), b"keep");
    assert_eq!(
        std::fs::read(done.mirror.join("snapshot.toml")).unwrap(),
        inventory
    );
    let empty = t.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    run_forced(&cfg, None, &empty, &source(0, false))
        .expect_err("force should refuse an unrelated empty directory");
    assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
}
#[test]
fn force_failure_leaves_no_old_mirror_and_can_restart_lock_only_shell() {
    use rust_mirror::selection::sync::run_forced;
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    let cfg = config();
    run_forced(&cfg, None, &out, &source(0, true)).unwrap();
    run_forced(&cfg, None, &out, &source(0, false))
        .expect_err("force should fail when fresh metadata is unavailable");
    assert!(!out.join("mirror").exists());
    assert!(!out.join(".sync").exists());
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    run_forced(&cfg, None, &out, &source(0, true)).unwrap();
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn force_rebuilds_with_external_previous_reuse_without_mutating_previous() {
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let previous = run(&cfg, None, &t.path().join("prior"), false, &source(0, true)).unwrap();
    let out = t.path().join("run");
    run(&cfg, None, &out, false, &source(0, true)).unwrap();
    let before = std::fs::read(previous.mirror.join("snapshot.toml")).unwrap();
    let result = rust_mirror::selection::sync::run_forced(
        &cfg,
        Some(&previous.mirror),
        &out,
        &source(1, true),
    )
    .unwrap();
    assert_eq!(result.report.status, "created");
    assert_eq!(result.report.upstream_attempts, 0);
    assert_eq!(
        std::fs::read(previous.mirror.join("snapshot.toml")).unwrap(),
        before
    );
}
#[cfg(unix)]
#[test]
fn force_refuses_outer_and_inner_links_or_special_files_before_deleting() {
    use rust_mirror::selection::sync::run_forced;
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    run(&config(), None, &out, false, &source(0, true)).unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&out, &link).unwrap();
    run_forced(&config(), None, &link, &source(0, false))
        .expect_err("force should refuse a symlinked outer run directory");
    let protected = t.path().join("protected");
    std::fs::create_dir(&protected).unwrap();
    std::fs::write(protected.join("mine"), b"safe").unwrap();
    std::os::unix::fs::symlink(&protected, out.join(".sync/escape")).unwrap();
    run_forced(&config(), None, &out, &source(0, false))
        .expect_err("force should refuse an inner symlink before deleting either tree");
    assert!(out.join("mirror/snapshot.toml").exists());
    assert_eq!(std::fs::read(protected.join("mine")).unwrap(), b"safe");
    std::fs::remove_file(out.join(".sync/escape")).unwrap();
    let fifo = out.join(".sync/fifo");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    run_forced(&config(), None, &out, &source(0, false))
        .expect_err("force should refuse an inner FIFO before deleting the mirror");
    assert!(out.join("mirror/snapshot.toml").exists());
}

#[cfg(windows)]
#[test]
fn force_refuses_windows_junctions_before_deleting_either_tree() {
    use rust_mirror::selection::sync::run_forced;
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    run(&config(), None, &out, false, &source(0, true)).unwrap();
    let protected = t.path().join("protected");
    std::fs::create_dir(&protected).unwrap();
    std::fs::write(protected.join("mine"), b"safe").unwrap();
    let outer = t.path().join("linked-run");
    let created = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&outer)
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    run_forced(&config(), None, &outer, &source(0, false))
        .expect_err("force should refuse a junction as the outer run directory");
    std::fs::remove_dir(&outer).unwrap();
    let linked = out.join(".sync").join("junction");
    let created = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&linked)
        .arg(&protected)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    run_forced(&config(), None, &out, &source(0, false))
        .expect_err("force should refuse an inner junction before deleting either tree");
    assert!(out.join("mirror/snapshot.toml").exists());
    assert_eq!(std::fs::read(protected.join("mine")).unwrap(), b"safe");
    // Remove this test's junction itself, never its target.
    std::fs::remove_dir(&linked).unwrap();
}

#[test]
fn prune_lock_allows_toolchain_sync_resume_force_and_selected_fetch_resume() {
    use rust_mirror::selection::{retention, sync::run_forced};
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let out = t.path().join("run");
    let result = run(&cfg, None, &out, false, &source(0, true)).unwrap();
    let policy = retention::Policy {
        pinned_releases: vec![],
        latest_stable: 0,
        nightly_days: 0,
        previous_generations: 0,
        keep_newest: 0,
        pinned_generations: vec![],
    };
    let preview = retention::preview(&policy, &[result.mirror], "2026-11-22").unwrap();
    assert!(retention::apply(&preview, true).unwrap().failure.is_none());
    assert!(out.join(".rust-mirror-prune.lock").is_file());
    run(&cfg, None, &out, true, &source(0, false)).unwrap();
    run_forced(&cfg, None, &out, &source(0, true)).unwrap();

    let plan = rust_mirror::selection::resolve(&cfg, "project-a", &Fixture::new()).unwrap();
    let plan_root = t.path().join("selected-plan");
    plan.save(&plan_root).unwrap();
    let selected = t.path().join("selected");
    let mirror = rust_mirror::selection::fetch(&plan_root, &selected, &Fixture::new()).unwrap();
    let preview = retention::preview(&policy, &[mirror], "2026-11-22").unwrap();
    retention::apply(&preview, true).unwrap();
    rust_mirror::selection::fetch_resuming(&plan_root, &selected, &Fixture::new()).unwrap();
}
