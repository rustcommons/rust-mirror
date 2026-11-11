mod common;
use common::{Fixture, HOST};
use rust_mirror::{
    events::*,
    selection::{self, Config},
    ErrorKind,
};
use std::{
    io::{BufRead, Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
static SERIAL: Mutex<()> = Mutex::new(());
#[derive(Default)]
struct Seen(Mutex<Vec<Event>>);
impl Observer for Seen {
    fn observe(&self, event: &Event) -> std::io::Result<()> {
        self.0.lock().unwrap().push(event.clone());
        Ok(())
    }
}
struct CancelOn {
    seen: Arc<Seen>,
    token: CancelToken,
    hash: bool,
}
impl Observer for CancelOn {
    fn observe(&self, event: &Event) -> std::io::Result<()> {
        self.seen.observe(event)?;
        if matches!(event.kind, EventKind::Hashing { count: 0, .. }) && self.hash
            || matches!(event.kind,EventKind::Artifact{ref action,..} if action=="delete")
                && !self.hash
        {
            self.token.cancel();
        }
        Ok(())
    }
}
fn config() -> Config {
    Config::parse(&format!("schema=1\nrustup_version=\"1.28.2\"\n[[selections]]\nname=\"test\"\ntoolchain=\"1.90.0\"\nhosts=[\"{HOST}\"]\nprofile=\"minimal\"\n")).unwrap()
}
fn snapshot(t: &std::path::Path) -> std::path::PathBuf {
    rust_mirror::prepare(
        &rust_mirror::Options {
            release: "1.90.0".into(),
            rustup_version: "1.28.2".into(),
            host: HOST.into(),
            output: t.join("legacy"),
        },
        &Fixture::new(),
    )
    .unwrap()
}
#[test]
fn cancelled_fetch_keeps_frozen_state_and_can_resume() {
    let _s = SERIAL.lock().unwrap();
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let f = Fixture::new();
    let plan = t.path().join("plan");
    selection::resolve(&cfg, "test", &f)
        .unwrap()
        .save(&plan)
        .unwrap();
    struct StopSource {
        f: Fixture,
        token: CancelToken,
    }
    impl rust_mirror::Source for StopSource {
        fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> rust_mirror::Result<()> {
            self.token.cancel();
            self.f.fetch(url, out, limit)
        }
    }
    let token = CancelToken::new();
    let seen = Arc::new(Seen::default());
    let control = Control::new(seen.clone(), token.clone());
    let out = t.path().join("out");
    let e = selection::fetch_with_control(
        &plan,
        &out,
        &StopSource {
            f: Fixture::new(),
            token,
        },
        &control,
    )
    .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Cancelled);
    assert!(e.is::<rust_mirror::Error>());
    assert!(out.join(".download/state.toml").exists());
    assert!(!out.join("mirror").exists());
    let complete =
        selection::fetch_resuming_with_control(&plan, &out, &f, &Control::default()).unwrap();
    assert!(complete.join("snapshot.toml").exists());
    assert!(seen
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e.kind, EventKind::Artifact { .. })));
}
#[test]
fn heartbeat_continues_while_operation_is_blocked_and_tracks_stalled_progress() {
    let _s = SERIAL.lock().unwrap();
    let seen = Arc::new(Seen::default());
    let c = Control::new(seen.clone(), CancelToken::new());
    c.run("blocked-read", || {
        std::thread::park_timeout(Duration::from_millis(1250));
        Ok(())
    })
    .unwrap();
    let list = seen.0.lock().unwrap();
    let heart = list
        .iter()
        .find(|e| matches!(e.kind, EventKind::Heartbeat { .. }))
        .expect("independent heartbeat");
    assert!(
        matches!(&heart.kind,EventKind::Heartbeat{stage,operation,..} if stage=="blocked-read"&&operation=="blocked-read")
    );
    assert!(list.windows(2).all(|w| w[0].seq < w[1].seq));
}
#[test]
fn hashing_cancel_preserves_source_and_prune_cancel_has_partial_report() {
    let _s = SERIAL.lock().unwrap();
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let f = Fixture::new();
    let plan = t.path().join("plan");
    selection::resolve(&cfg, "test", &f)
        .unwrap()
        .save(&plan)
        .unwrap();
    let one = selection::fetch(&plan, &t.path().join("one"), &f).unwrap();
    let two = selection::fetch(&plan, &t.path().join("two"), &f).unwrap();
    let policy = selection::retention::Policy {
        pinned_releases: vec![],
        latest_stable: 0,
        nightly_days: 0,
        previous_generations: 0,
        keep_newest: 0,
        pinned_generations: vec![],
    };
    let token = CancelToken::new();
    let obs = Arc::new(CancelOn {
        seen: Arc::new(Seen::default()),
        token: token.clone(),
        hash: true,
    });
    let c = Control::new(obs, token);
    let e = selection::retention::preview_with_control(
        &policy,
        &[one.clone(), two.clone()],
        "2025-09-18",
        &c,
    )
    .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Cancelled);
    assert!(one.join("snapshot.toml").exists());
    assert!(two.join("snapshot.toml").exists());
    let preview =
        selection::retention::preview(&policy, &[one, two.clone()], "2025-09-18").unwrap();
    let token = CancelToken::new();
    let c = Control::new(
        Arc::new(CancelOn {
            seen: Arc::new(Seen::default()),
            token: token.clone(),
            hash: false,
        }),
        token,
    );
    let e = selection::retention::apply_with_control(&preview, true, &c).unwrap_err();
    let error = e.downcast_ref::<rust_mirror::Error>().unwrap();
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(error.report.is_some());
    assert!(two.join("snapshot.toml").exists());
}
#[test]
fn preflight_is_head_only_and_typed_remote_exists_refuses_put() {
    let _s = SERIAL.lock().unwrap();
    let t = tempfile::tempdir().unwrap();
    let root = snapshot(t.path());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(sock.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        first
    });
    let options = rust_mirror::publish::Options {
        base_url: format!("http://{addr}/mirror"),
        ca_file: None,
        credentials: None,
        allow_loopback_http: true,
    };
    let e = rust_mirror::publish::preflight_with_control(&root, &options, &Control::default())
        .err()
        .unwrap();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::RemoteExists);
    assert!(server.join().unwrap().starts_with("HEAD "));
}
#[test]
fn cli_blocked_preflight_heartbeats_and_stdin_cancellation() {
    let _s = SERIAL.lock().unwrap();
    for mode in ["record", "eof", "signal"] {
        if mode == "signal" && cfg!(windows) {
            continue;
        }
        let t = tempfile::tempdir().unwrap();
        let root = snapshot(t.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
            let mut buf = [0];
            let mut header = Vec::new();
            while sock.read(&mut buf).unwrap_or(0) > 0 {
                header.push(buf[0]);
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let _ = release_rx.recv_timeout(Duration::from_secs(8));
            let _ = sock.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            header
        });
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rust-mirror"));
        command
            .args([
                "--events",
                "jsonl",
                "--cancel-stdin",
                "publish",
                "--preflight",
                "--source",
            ])
            .arg(&root)
            .args([
                "--base-url",
                &format!("http://{addr}/mirror"),
                "--allow-loopback-http",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let child = command.spawn().unwrap();
        struct Cleanup(std::process::Child);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Cleanup(child);
        let stdout = child.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                if tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut lines = Vec::new();
        loop {
            let line = rx.recv_timeout(Duration::from_secs(8)).unwrap();
            let heartbeat = line.contains("\"type\":\"heartbeat\"");
            lines.push(line);
            if heartbeat {
                break;
            }
        }
        match mode {
            "record" => {
                child
                    .0
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(b"{ \"type\" : \"cancel\" }\n")
                    .unwrap();
            }
            "eof" => {
                drop(child.0.stdin.take());
            }
            _ => {
                #[cfg(unix)]
                assert!(std::process::Command::new("kill")
                    .args(["-TERM", &child.0.id().to_string()])
                    .status()
                    .unwrap()
                    .success());
            }
        }
        loop {
            let line = rx.recv_timeout(Duration::from_secs(8)).unwrap();
            let cancelled = line.contains("\"cancel_requested\":true");
            lines.push(line);
            if cancelled {
                break;
            }
        }
        release_tx.send(()).unwrap();
        let status = child.0.wait().unwrap();
        assert_eq!(status.code(), Some(130));
        reader.join().unwrap();
        lines.extend(rx.try_iter());
        assert!(lines.first().unwrap().contains("\"type\":\"hello\""));
        let last = lines.last().unwrap();
        assert!(last.contains("\"type\":\"result\""));
        assert!(last.contains("\"kind\":\"cancelled\""), "{last}");
        assert_eq!(
            lines
                .iter()
                .filter(|s| s.contains("\"type\":\"result\""))
                .count(),
            1
        );
        assert!(String::from_utf8(server.join().unwrap())
            .unwrap()
            .starts_with("HEAD "));
    }
}
