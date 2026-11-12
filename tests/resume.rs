mod common;
use common::{hash, Fixture, HOST, ORIGIN};
use rust_mirror::{selection::*, Result, Source, Transfer};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
fn config() -> Config {
    Config {
        schema: 1,
        retention: None,
        network: None,
        rustup_version: "1.28.2".into(),
        max_download_bytes: 10 * 1024 * 1024,
        selections: vec![Selection {
            name: "test".into(),
            toolchain: "1.90.0".into(),
            hosts: vec![HOST.into()],
            profile: "minimal".into(),
            components: vec![],
            targets: vec![],
        }],
    }
}
fn fixture() -> Fixture {
    let mut f = Fixture::new();
    let mut manifest: toml::Value = toml::from_str(&Fixture::manifest()).unwrap();
    for (_, pkg) in manifest["pkg"].as_table_mut().unwrap() {
        for (_, target) in pkg["target"].as_table_mut().unwrap() {
            for (url, sha) in [("url", "hash"), ("xz_url", "xz_hash")] {
                if let Some(url) = target
                    .get(url)
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
                {
                    let bytes = format!("unique wire bytes for {url}").into_bytes();
                    target[sha] = toml::Value::String(hash(&bytes));
                    f.files.insert(url, bytes);
                }
            }
        }
    }
    f.set_manifest(toml::to_string(&manifest).unwrap());
    f
}
struct Interrupt {
    inner: Fixture,
    fail_on: usize,
    restart: bool,
    requests: Mutex<Vec<(String, u64)>>,
}
impl Source for Interrupt {
    fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
        self.inner.fetch(url, out, limit)
    }
    fn fetch_from(
        &self,
        url: &str,
        out: &mut dyn Write,
        offset: u64,
        limit: u64,
    ) -> Result<Transfer> {
        let mut requests = self.requests.lock().unwrap();
        requests.push((url.into(), offset));
        if offset > 0 && self.restart {
            return Ok(Transfer::Restart);
        }
        let data = self.inner.files.get(url).ok_or("unknown fixture")?;
        if offset as usize >= data.len() {
            return Ok(Transfer::Restart);
        }
        let bytes = &data[offset as usize..];
        if bytes.len() as u64 > limit {
            return Err("fixture budget".into());
        }
        if requests.len() == self.fail_on {
            out.write_all(&bytes[..bytes.len() / 2])?;
            return Err("simulated connection interruption".into());
        }
        out.write_all(bytes)?;
        Ok(Transfer::Complete)
    }
}
fn source(fail: usize, restart: bool) -> Interrupt {
    Interrupt {
        inner: fixture(),
        fail_on: fail,
        restart,
        requests: Mutex::new(vec![]),
    }
}
fn plan(root: &Path) -> (PathBuf, Plan) {
    let p = resolve(&config(), "test", &fixture()).unwrap();
    let plan = p.plan.clone();
    let path = root.join("plan");
    p.save(&path).unwrap();
    (path, plan)
}
#[test]
fn resumes_partial_and_reuses_verified_files_then_verifies_completed_tree_offline() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    let interrupted = source(2, false);
    assert!(fetch(&p, &out, &interrupted).is_err());
    assert!(!out.join("mirror").exists());
    let next = &plan.artifacts[1];
    let partial = out
        .join(".download/files")
        .join(format!("{}.part", next.sha256));
    let offset = std::fs::metadata(partial).unwrap().len();
    assert!(offset > 0);
    let resumed = source(0, false);
    let mirror = fetch_resuming(&p, &out, &resumed).unwrap();
    let requests = resumed.requests.lock().unwrap();
    assert_eq!(requests[0], (format!("{ORIGIN}{}", next.path), offset));
    assert!(!requests
        .iter()
        .any(|(url, _)| url == &format!("{ORIGIN}{}", plan.artifacts[0].path)));
    drop(requests);
    assert!(std::fs::read_dir(out.join(".download/files"))
        .unwrap()
        .next()
        .is_none());
    let offline = source(1, false);
    assert_eq!(fetch_resuming(&p, &out, &offline).unwrap(), mirror);
    assert!(offline.requests.lock().unwrap().is_empty());
    let changed = mirror.join(&plan.artifacts[0].path);
    std::fs::write(&changed, b"tampered").unwrap();
    assert!(fetch_resuming(&p, &out, &offline).is_err());
    assert_eq!(std::fs::read(changed).unwrap(), b"tampered");
}
#[test]
fn ignored_ranges_restart_only_interrupted_file() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let resumed = source(0, true);
    fetch_resuming(&p, &out, &resumed).unwrap();
    let r = resumed.requests.lock().unwrap();
    assert_eq!(r[0].0, r[1].0);
    assert!(r[0].1 > 0);
    assert_eq!(r[1].1, 0);
    assert!(!r
        .iter()
        .any(|(url, _)| url.ends_with(&plan.artifacts[0].path)));
}
#[test]
fn corrupt_partial_is_discarded_after_final_hash_and_next_resume_repairs_it() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let part = out
        .join(".download/files")
        .join(format!("{}.part", plan.artifacts[1].sha256));
    let n = std::fs::metadata(&part).unwrap().len();
    std::fs::write(&part, vec![b'x'; n as usize]).unwrap();
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_err());
    assert!(!part.exists());
    assert!(!out.join("mirror").exists());
    fetch_resuming(&p, &out, &source(0, false)).unwrap();
}
#[test]
fn changed_plan_and_unrelated_files_refuse_without_writes() {
    let temp = tempfile::tempdir().unwrap();
    let (p, _) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let mut c = config();
    c.max_download_bytes += 1;
    let other = temp.path().join("other-plan");
    resolve(&c, "test", &fixture())
        .unwrap()
        .save(&other)
        .unwrap();
    let s = source(0, false);
    assert!(fetch_resuming(&other, &out, &s).is_err());
    assert!(s.requests.lock().unwrap().is_empty());
    std::fs::write(out.join("keep"), b"mine").unwrap();
    assert!(fetch_resuming(&p, &out, &s).is_err());
    assert_eq!(std::fs::read(out.join("keep")).unwrap(), b"mine");
}
#[test]
fn competing_process_lock_and_missing_state_refuse() {
    let temp = tempfile::tempdir().unwrap();
    let (p, _) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .open(out.join(".fetch.lock"))
        .unwrap();
    held.try_lock().unwrap();
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_err());
    drop(held);
    let state = out.join(".download/state.toml");
    std::fs::write(&state, b"schema=1").unwrap();
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_err());
}
#[cfg(unix)]
#[test]
fn symlinks_devices_and_unix_backslash_paths_never_escape_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let files = out.join(".download/files");
    let saved = temp.path().join("outside");
    std::fs::write(&saved, b"safe").unwrap();
    let data = files.join(format!("{}.data", plan.artifacts[0].sha256));
    std::fs::remove_file(&data).unwrap();
    std::os::unix::fs::symlink(&saved, &data).unwrap();
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_err());
    assert_eq!(std::fs::read(saved).unwrap(), b"safe");
    std::fs::remove_file(data).unwrap();
    let weird = files.join("..\\escaped");
    std::fs::write(&weird, b"keep").unwrap();
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_err());
    assert_eq!(std::fs::read(weird).unwrap(), b"keep");
}

#[test]
fn corrupted_verified_cache_is_rehashed_and_repaired() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let ready = out
        .join(".download/files")
        .join(format!("{}.data", plan.artifacts[0].sha256));
    std::fs::write(ready, b"damaged").unwrap();
    let s = source(0, false);
    fetch_resuming(&p, &out, &s).unwrap();
    assert_eq!(
        s.requests.lock().unwrap()[0].0,
        format!("{ORIGIN}{}", plan.artifacts[0].path)
    );
}
#[test]
fn already_complete_partial_is_verified_without_requesting_its_tail() {
    let temp = tempfile::tempdir().unwrap();
    let (p, plan) = plan(temp.path());
    let out = temp.path().join("output");
    assert!(fetch(&p, &out, &source(2, false)).is_err());
    let a = &plan.artifacts[1];
    let url = format!("{ORIGIN}{}", a.path);
    let part = out
        .join(".download/files")
        .join(format!("{}.part", a.sha256));
    std::fs::write(part, &fixture().files[&url]).unwrap();
    let s = source(0, false);
    fetch_resuming(&p, &out, &s).unwrap();
    assert!(!s.requests.lock().unwrap().iter().any(|(u, _)| u == &url));
}
#[test]
fn crash_child() {
    let Some(root) = std::env::var_os("RUST_MIRROR_TEST_CRASH_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    struct Blocking {
        fixture: Fixture,
        marker: PathBuf,
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
            std::fs::write(&self.marker, b"partial-written")?;
            loop {
                std::thread::park();
            }
        }
    }
    let _ = fetch(
        &root.join("plan"),
        &root.join("output"),
        &Blocking {
            fixture: fixture(),
            marker: root.join("ready"),
        },
    );
    panic!("crash probe should have been killed");
}
#[test]
fn killed_process_releases_lock_and_preserves_a_resumable_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let (p, _) = plan(temp.path());
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child", "--nocapture"])
        .env("RUST_MIRROR_TEST_CRASH_ROOT", temp.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !temp.path().join("ready").exists() && std::time::Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let ready = temp.path().join("ready").exists();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(ready, "child did not reach interrupted transfer");
    let out = temp.path().join("output");
    assert!(!out.join("mirror").exists());
    assert!(fetch_resuming(&p, &out, &source(0, false)).is_ok());
}
