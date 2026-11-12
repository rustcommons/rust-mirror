mod common;
use rust_mirror::{
    events::{CancelToken, Control, Event, EventKind, Observer},
    network::*,
    selection, ErrorKind, Source,
};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
static SERIAL: Mutex<()> = Mutex::new(());
use ureq::rustls::{
    self,
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
};
#[derive(Default)]
struct State {
    requests: Vec<(String, String)>,
    head: Option<String>,
    denied: bool,
    legacy_auth_header: bool,
    expected_token: Option<String>,
    delay_millis: u64,
    remote: BTreeMap<String, Vec<u8>>,
}
struct Proxy {
    dir: tempfile::TempDir,
    addr: std::net::SocketAddr,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
fn openssl() -> PathBuf {
    if cfg!(windows) {
        let p = PathBuf::from(
            std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:/Program Files".into()),
        )
        .join("Git/usr/bin/openssl.exe");
        if p.is_file() {
            return p;
        }
    }
    "openssl".into()
}
impl Proxy {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("extensions"),"subjectAltName=DNS:static.rust-lang.org,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
        for args in [
            vec![
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=Corporate Fixture Root",
                "-addext",
                "basicConstraints=critical,CA:TRUE",
                "-keyout",
                "ca.key",
                "-out",
                "ca.pem",
            ],
            vec![
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=static.rust-lang.org",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
            ],
            vec![
                "x509",
                "-req",
                "-in",
                "server.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-out",
                "server.pem",
                "-days",
                "1",
                "-extfile",
                "extensions",
            ],
        ] {
            let output = Command::new(openssl())
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "fixture certificate generation failed"
            );
        }
        let leaf = CertificateDer::from_pem_file(dir.path().join("server.pem")).unwrap();
        let ca = CertificateDer::from_pem_file(dir.path().join("ca.pem")).unwrap();
        let key = PrivateKeyDer::from_pem_file(dir.path().join("server.key")).unwrap();
        let config = Arc::new(
            rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![leaf, ca], key)
            .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(State::default()));
        let stopped = stop.clone();
        let shared = state.clone();
        let handle = thread::spawn(move || {
            let mut threads = vec![];
            while !stopped.load(Ordering::SeqCst) {
                let (socket, _) = listener.accept().unwrap();
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let config = config.clone();
                let state = shared.clone();
                threads.push(thread::spawn(move || {
                    let _ = serve(socket, config, state);
                }));
            }
            for t in threads {
                t.join().unwrap();
            }
        });
        Self {
            dir,
            addr,
            state,
            stop,
            thread: Some(handle),
        }
    }
    fn options(&self) -> NetworkOptions {
        NetworkOptions {
            proxy: ProxyMode::Manual(format!("http://{}", self.addr)),
            proxy_credentials: Some(ProxyCredentials {
                username: "proxy-user".into(),
                password: "proxy-secret".into(),
            }),
            extra_ca_file: Some(self.dir.path().join("ca.pem")),
            connect_timeout: Duration::from_secs(2),
            read_timeout: Duration::from_secs(2),
            ..Default::default()
        }
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
fn serve(
    mut socket: TcpStream,
    config: Arc<rustls::ServerConfig>,
    state: Arc<Mutex<State>>,
) -> std::io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut header = vec![];
    while !header.ends_with(b"\r\n\r\n") {
        let mut b = [0];
        if socket.read(&mut b)? == 0 {
            return Ok(());
        }
        header.push(b[0]);
        if header.len() > 16384 {
            return Ok(());
        }
    }
    let text = String::from_utf8_lossy(&header);
    let first = text.lines().next().unwrap_or("");
    if !first.starts_with("CONNECT ") {
        return Ok(());
    }
    let expected = state
        .lock()
        .unwrap()
        .expected_token
        .clone()
        .unwrap_or_else(|| "cHJveHktdXNlcjpwcm94eS1zZWNyZXQ=".into());
    let authorized = text
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("Proxy-Authorization"))
        .any(|(_, value)| {
            value.trim().split_once(' ').is_some_and(|(scheme, token)| {
                scheme.eq_ignore_ascii_case("Basic") && token == expected
            })
        });
    let (deny, delay) = {
        let mut s = state.lock().unwrap();
        s.requests.push((
            "CONNECT".into(),
            first.split_whitespace().nth(1).unwrap_or("").into(),
        ));
        (s.denied, s.delay_millis)
    };
    if delay > 0 {
        thread::sleep(Duration::from_millis(delay));
    }
    if deny || !authorized {
        if state.lock().unwrap().legacy_auth_header {
            socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"Corp\xff\"\r\nContent-Length: 0\r\n\r\n")?;
            return Ok(());
        }
        socket.write_all(
            b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n",
        )?;
        return Ok(());
    }
    socket.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
    let connection = rustls::ServerConnection::new(config).unwrap();
    let mut tls = rustls::StreamOwned::new(connection, socket);
    let mut reader = std::io::BufReader::new(&mut tls);
    let mut first = String::new();
    if reader.read_line(&mut first)? == 0 {
        return Ok(());
    }
    let words = first.split_whitespace().collect::<Vec<_>>();
    if words.len() < 2 {
        return Ok(());
    }
    let method = words[0].to_owned();
    let path = if words[1].starts_with("https://") {
        ureq::get(words[1]).request_url().unwrap().path().to_owned()
    } else {
        words[1].to_owned()
    };
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap();
            }
        }
    }
    assert!(length < 1024 * 1024);
    let mut upload = vec![0; length];
    reader.read_exact(&mut upload)?;
    drop(reader);
    let mut s = state.lock().unwrap();
    s.requests.push((method.clone(), path.clone()));
    let mut fixture = common::Fixture::new().files;
    fixture.insert(
        "https://static.rust-lang.org/dist/channel-rust-stable.toml.sha256".into(),
        format!("{}  channel-rust-stable.toml\n", "a".repeat(64)).into_bytes(),
    );
    fixture.insert("https://static.rust-lang.org/manifests.txt".into(),b"static.rust-lang.org/dist/2025-09-18/channel-rust-1.90.0.toml\nstatic.rust-lang.org/dist/2025-09-19/channel-rust-beta.toml\nstatic.rust-lang.org/dist/2025-09-20/channel-rust-nightly.toml\n".to_vec());
    let (status, data) = if path.starts_with("/publish/") {
        match method.as_str() {
            "HEAD" => (
                if s.remote.contains_key(&path) {
                    200
                } else {
                    404
                },
                vec![],
            ),
            "PUT" => {
                s.remote.insert(path.clone(), upload);
                (201, vec![])
            }
            "GET" => s
                .remote
                .get(&path)
                .map(|d| (200, d.clone()))
                .unwrap_or((404, vec![])),
            _ => (405, vec![]),
        }
    } else if path == "/dist/redirect" {
        (302, vec![])
    } else {
        fixture
            .get(&format!("https://static.rust-lang.org{path}"))
            .map(|v| (200, v.clone()))
            .unwrap_or((404, vec![]))
    };
    let headers = if method == "HEAD" {
        s.head
            .clone()
            .unwrap_or_else(|| format!("Content-Length: {}\r\n", data.len()))
    } else {
        format!("Content-Length: {}\r\n", data.len())
    };
    drop(s);
    write!(tls,"HTTP/1.1 {status} Status\r\n{headers}Connection: close\r\nLocation: https://example.com/forbidden\r\n\r\n")?;
    if method != "HEAD" {
        tls.write_all(&data)?;
    }
    tls.flush()?;
    Ok(())
}
#[derive(Default)]
struct Seen(Mutex<Vec<Event>>);
impl Observer for Seen {
    fn observe(&self, e: &Event) -> std::io::Result<()> {
        self.0.lock().unwrap().push(e.clone());
        Ok(())
    }
}
fn config() -> selection::Config {
    selection::Config::parse(&format!("schema=1\nrustup_version=\"1.28.2\"\n[[selections]]\nname=\"test\"\ntoolchain=\"1.90.0\"\nhosts=[\"{}\"]\nprofile=\"minimal\"",common::HOST)).unwrap()
}
#[test]
fn inspect_connect_auth_trust_chain_head_and_refusals() {
    let _serial = SERIAL.lock().unwrap();
    let proxy = Proxy::new();
    let options = proxy.options();
    let ca = inspect_ca_file(options.extra_ca_file.as_ref().unwrap()).unwrap();
    assert!(ca[0].subject.contains("Corporate Fixture Root"));
    assert_eq!(ca[0].sha256.len(), 64);
    let control = Control::default();
    let report = check_network_with_control(&options, &control).unwrap();
    assert!(
        report.ok,
        "{report:?}; requests {:?}",
        proxy.state.lock().unwrap().requests
    );
    assert_eq!(report.steps.len(), 5);
    let mut untrusted = options.clone();
    untrusted.extra_ca_file = None;
    let report = check_network_with_control(&untrusted, &control).unwrap();
    assert!(!report.ok);
    let failure = report
        .steps
        .iter()
        .find(|s| s.error_kind == Some(ErrorKind::Tls))
        .unwrap();
    assert!(failure
        .certificates
        .iter()
        .any(|c| c.issuer.contains("Corporate Fixture Root")));
    assert!(failure.certificates.len() >= 2);
    let mut denied = options.clone();
    denied.proxy_credentials.as_mut().unwrap().password = "wrong-password".into();
    let report = check_network_with_control(&denied, &control).unwrap();
    assert!(report
        .steps
        .iter()
        .any(|s| s.error_kind == Some(ErrorKind::Auth)));
    let source = rust_mirror::HttpsSource::with_network(&denied).unwrap();
    let e = source
        .content_length("https://static.rust-lang.org/dist/channel-rust-1.90.0.toml")
        .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Auth);
    assert!(!format!("{e:?}").contains("wrong-password"));
    assert!(!format!("{options:?}").contains("proxy-secret"));
    {
        let mut s = proxy.state.lock().unwrap();
        s.denied = true;
        s.legacy_auth_header = true;
    }
    let legacy = check_network_with_control(&options, &control).unwrap();
    assert!(legacy
        .steps
        .iter()
        .any(|s| s.error_kind == Some(ErrorKind::Auth)));
    {
        let mut s = proxy.state.lock().unwrap();
        s.denied = false;
        s.legacy_auth_header = false;
    }
    let source = rust_mirror::HttpsSource::with_network(&options).unwrap();
    let url = "https://static.rust-lang.org/dist/channel-rust-1.90.0.toml";
    assert!(source.content_length(url).unwrap().unwrap() > 100);
    proxy.state.lock().unwrap().head = Some("X-No-Length: unknown\r\n".into());
    assert_eq!(source.content_length(url).unwrap(), None);
    for header in [
        "Content-Length: 12\r\nContent-Length: 13\r\n",
        "Content-Length: -1\r\n",
        "Content-Length: 18446744073709551616\r\n",
    ] {
        proxy.state.lock().unwrap().head = Some(header.into());
        assert!(source.content_length(url).is_err());
    }
    proxy.state.lock().unwrap().head = None;
    assert!(source
        .content_length("https://example.com/dist/file")
        .is_err());
    assert!(source
        .fetch(
            "https://static.rust-lang.org/dist/redirect",
            &mut vec![],
            100
        )
        .is_err());
    for url in [
        "http://user:secret@127.0.0.1:12",
        "socks5://localhost:1080",
        "https://localhost:8080",
        "http://localhost:0",
        "http://localhost/path",
    ] {
        let bad = NetworkOptions {
            proxy: ProxyMode::Manual(url.into()),
            ..Default::default()
        };
        assert!(rust_mirror::HttpsSource::with_network(&bad).is_err());
    }
    assert!(is_excluded("static.rust-lang.org", 443, &[".rust-lang.org:443".into()]).unwrap());
    assert!(!is_excluded("evilrust-lang.org", 443, &["rust-lang.org".into()]).unwrap());
    assert!(!is_excluded("static.rust-lang.org", 444, &["rust-lang.org:443".into()]).unwrap());
    assert!(is_excluded("x", 443, &["*".into()]).unwrap());
    assert!(is_excluded("x", 443, &["10.0.0.0/8".into()]).is_err());
    let mut excluded = options.clone();
    excluded.no_proxy = vec!["rust-lang.org".into()]; // Constructor validates the route, no network needed.
    assert!(rust_mirror::HttpsSource::with_network(&excluded).is_ok());
}
#[test]
fn timeouts_cancel_and_no_secret_in_cli_results() {
    let _serial = SERIAL.lock().unwrap();
    let proxy = Proxy::new();
    proxy.state.lock().unwrap().delay_millis = 600;
    let mut options = proxy.options();
    options.read_timeout = Duration::from_millis(100);
    let start = std::time::Instant::now();
    let report = check_network_with_control(&options, &Control::default()).unwrap();
    assert!(!report.ok);
    assert!(start.elapsed() < Duration::from_secs(2));
    proxy.state.lock().unwrap().delay_millis = 1200;
    let seen = Arc::new(Seen::default());
    let token = CancelToken::new();
    let stop = token.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stop.cancel();
    });
    let blocked = check_network_with_control(&proxy.options(), &Control::new(seen.clone(), token))
        .unwrap_err();
    canceller.join().unwrap();
    assert_eq!(
        rust_mirror::error_kind(blocked.as_ref()),
        ErrorKind::Cancelled
    );
    assert!(seen.0.lock().unwrap().iter().any(|e| matches!(
        e.kind,
        EventKind::Heartbeat {
            cancel_requested: true,
            ..
        }
    )));
    let token = CancelToken::new();
    token.cancel();
    let e = check_network_with_control(&options, &Control::new(Arc::new(Seen::default()), token))
        .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Cancelled);
    let output = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .args([
            "--events",
            "jsonl",
            "--proxy",
            "http://user:top-secret@localhost:9",
            "check-network",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("top-secret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("top-secret"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"command\":\"check-network\""));
}
#[test]
fn estimates_sync_progress_cached_offline_and_explicit_publish() {
    let _serial = SERIAL.lock().unwrap();
    let proxy = Proxy::new();
    let source = rust_mirror::HttpsSource::with_network(&proxy.options()).unwrap();
    let t = tempfile::tempdir().unwrap();
    let cfg = config();
    let resolved = selection::resolve(&cfg, "test", &source).unwrap();
    let sizes = selection::measure_sizes(&resolved, &source, &Control::default()).unwrap();
    assert!(sizes.complete);
    assert!(sizes.estimated);
    assert_eq!(sizes.unknown_artifacts, 0);
    let total = resolved
        .plan
        .artifacts
        .iter()
        .map(|a| {
            common::Fixture::new().files[&format!("{}{}", common::ORIGIN, a.path)].len() as u64
        })
        .sum::<u64>();
    assert_eq!(sizes.known_bytes, total);
    let seen = Arc::new(Seen::default());
    let control = Control::new(seen.clone(), CancelToken::new());
    let out = t.path().join("run");
    let run = selection::sync::run_with_options(
        &cfg,
        None,
        &out,
        &selection::sync::RunOptions {
            estimate_sizes: true,
            ..Default::default()
        },
        &source,
        &control,
    )
    .unwrap();
    let events = seen.0.lock().unwrap();
    let last = events
        .iter()
        .rev()
        .find_map(|e| {
            if let EventKind::TransferSummary {
                completed_bytes,
                transferred_bytes,
                expected_bytes,
                ..
            } = e.kind
            {
                Some((completed_bytes, transferred_bytes, expected_bytes))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(last.0, total);
    assert_eq!(last.1, run.report.upstream_payload_bytes);
    assert!(last.1 < total);
    assert_eq!(last.2, Some(total));
    drop(events);
    let publisher = rust_mirror::publish::Options {
        base_url: "https://127.0.0.1:1234/publish".into(),
        credentials: None,
        ca_file: None,
        allow_loopback_http: false,
    };
    let before = proxy.state.lock().unwrap().requests.len();
    let preflight = rust_mirror::publish::preflight_with_network(
        &run.mirror,
        &publisher,
        &proxy.options(),
        &control,
    )
    .unwrap();
    assert!(preflight.files > 0);
    assert!(proxy.state.lock().unwrap().requests[before..]
        .iter()
        .all(|(m, _)| m == "CONNECT" || m == "HEAD"));
    rust_mirror::publish::publish_with_network(&run.mirror, &publisher, &proxy.options(), &control)
        .unwrap();
    let cache = t.path().join("cache");
    let catalog = rust_mirror::releases::load(
        &source,
        &cache,
        rust_mirror::releases::CacheMode::Refresh,
        &control,
    )
    .unwrap();
    assert!(!catalog.cached);
    assert_eq!(catalog.releases.len(), 3);
    let requests = proxy.state.lock().unwrap().requests.len();
    let offline = rust_mirror::releases::load(
        &source,
        &cache,
        rust_mirror::releases::CacheMode::Offline,
        &control,
    )
    .unwrap();
    assert!(offline.cached);
    assert_eq!(offline.releases, catalog.releases);
    assert_eq!(proxy.state.lock().unwrap().requests.len(), requests);
    fs::write(cache.join("releases.toml"), "corrupt").unwrap();
    assert!(rust_mirror::releases::load(
        &source,
        &cache,
        rust_mirror::releases::CacheMode::Offline,
        &control
    )
    .is_err());
}

#[test]
fn environment_detection_is_explicit_sanitized_and_has_deterministic_precedence() {
    let _serial = SERIAL.lock().unwrap();
    if let Ok(mode) = std::env::var("ROM_NETWORK_ENV_CASE") {
        if mode == "valid" {
            let env = detect_environment().unwrap();
            assert_eq!(env.proxy_url.as_deref(), Some("http://first.example:8080"));
            assert!(env.credentials_present);
            assert_eq!(env.no_proxy, vec![".rust-lang.org:443"]);
            assert!(!format!("{env:?}").contains("never-show-password"));
            assert!(!toml::to_string(&env)
                .unwrap()
                .contains("never-show-password"));
        } else {
            assert!(detect_environment().is_err());
        }
        // The malformed environment is never inspected by a direct source constructor.
        assert!(rust_mirror::HttpsSource::with_network(&NetworkOptions::default()).is_ok());
        return;
    }
    for mode in ["valid", "malformed"] {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "environment_detection_is_explicit_sanitized_and_has_deterministic_precedence",
        ]);
        for name in [
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
        ] {
            cmd.env_remove(name);
        }
        cmd.env("ROM_NETWORK_ENV_CASE", mode)
            .env(
                "HTTPS_PROXY",
                if mode == "valid" {
                    "http://user:never-show-password@first.example:8080"
                } else {
                    "socks5://bad.example:1080"
                },
            )
            .env("https_proxy", "http://second.example:8081")
            .env("ALL_PROXY", "http://third.example:8082")
            .env("NO_PROXY", ".rust-lang.org:443")
            .env("no_proxy", "wrong.example");
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn unknown_sizes_overflow_budget_and_network_settings_do_not_change_resume_recipe() {
    let _serial = SERIAL.lock().unwrap();
    struct Head {
        inner: common::Fixture,
        value: Option<u64>,
    }
    impl Source for Head {
        fn fetch(&self, u: &str, w: &mut dyn Write, l: u64) -> rust_mirror::Result<()> {
            self.inner.fetch(u, w, l)
        }
        fn content_length(&self, _: &str) -> rust_mirror::Result<Option<u64>> {
            Ok(self.value)
        }
    }
    let cfg = config();
    let resolved = selection::resolve(&cfg, "test", &common::Fixture::new()).unwrap();
    let unknown = selection::measure_sizes(
        &resolved,
        &Head {
            inner: common::Fixture::new(),
            value: None,
        },
        &Control::default(),
    )
    .unwrap();
    assert!(!unknown.complete);
    assert_eq!(unknown.known_bytes, 0);
    assert_eq!(unknown.unknown_artifacts, resolved.plan.artifacts.len());
    assert!(selection::measure_sizes(
        &resolved,
        &Head {
            inner: common::Fixture::new(),
            value: Some(u64::MAX)
        },
        &Control::default()
    )
    .is_err());
    let large = selection::measure_sizes(
        &resolved,
        &Head {
            inner: common::Fixture::new(),
            value: Some(cfg.max_download_bytes),
        },
        &Control::default(),
    )
    .unwrap();
    assert!(large.exceeds_budget);
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("run");
    selection::sync::run(&cfg, None, &out, false, &common::Fixture::new()).unwrap();
    let mut changed = cfg.clone();
    changed.network = Some(NetworkConfig {
        read_timeout_seconds: 30,
        ..Default::default()
    });
    let resumed =
        selection::sync::run(&changed, None, &out, true, &common::Fixture::new()).unwrap();
    assert_eq!(resumed.report.status, "validated");
    let mut invalid = cfg;
    invalid.network = Some(NetworkConfig {
        proxy_mode: "manual".into(),
        proxy_url: Some("http://user:do-not-save@proxy.example:80".into()),
        ..Default::default()
    });
    assert!(selection::write_config(&invalid, &t.path().join("config.toml")).is_err());
    assert!(!t.path().join("config.toml").exists());
    assert!(!format!("{invalid:?}").contains("do-not-save"));
}

#[test]
fn literal_exclusions_and_refresh_fallback_are_bounded() {
    let _serial = SERIAL.lock().unwrap();
    assert!(!is_excluded("evil.127.0.0.1", 443, &["127.0.0.1".into()]).unwrap());
    assert!(is_excluded("[::1]", 443, &["::1".into()]).unwrap());
    assert!(is_excluded("[::1]", 443, &["[::1]:443".into()]).unwrap());
    struct Fail;
    impl Source for Fail {
        fn fetch(&self, _: &str, _: &mut dyn Write, _: u64) -> rust_mirror::Result<()> {
            Err(rust_mirror::Error::new(ErrorKind::Network, "synthetic unreachable source").into())
        }
    }
    struct Index;
    impl Source for Index {
        fn fetch(&self, _: &str, w: &mut dyn Write, _: u64) -> rust_mirror::Result<()> {
            w.write_all(b"static.rust-lang.org/dist/2025-09-18/channel-rust-1.90.0.toml\n")?;
            Ok(())
        }
    }
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("cache");
    let c = Control::default();
    rust_mirror::releases::load(&Index, &dir, rust_mirror::releases::CacheMode::Refresh, &c)
        .unwrap();
    let fallback = rust_mirror::releases::load(
        &Fail,
        &dir,
        rust_mirror::releases::CacheMode::RefreshOrCached,
        &c,
    )
    .unwrap();
    assert!(fallback.cached);
    assert_eq!(fallback.warning.unwrap().kind, ErrorKind::Network);
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(dir.join(".releases.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let e =
        rust_mirror::releases::load(&Index, &dir, rust_mirror::releases::CacheMode::Refresh, &c)
            .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Locked);
    drop(lock);
    fs::write(dir.join("unrelated.txt"), "preserve me").unwrap();
    assert!(rust_mirror::releases::load(
        &Index,
        &dir,
        rust_mirror::releases::CacheMode::Offline,
        &c
    )
    .is_err());
    fs::remove_file(dir.join("unrelated.txt")).unwrap();
    fs::write(dir.join("releases.toml"), "corrupt").unwrap();
    assert!(rust_mirror::releases::load(
        &Index,
        &dir,
        rust_mirror::releases::CacheMode::Refresh,
        &c
    )
    .is_err());
    assert_eq!(
        fs::read_to_string(dir.join("releases.toml")).unwrap(),
        "corrupt"
    );
    let absent = t.path().join("absent");
    let e = rust_mirror::releases::load(
        &Fail,
        &absent,
        rust_mirror::releases::CacheMode::RefreshOrCached,
        &c,
    )
    .unwrap_err();
    assert_eq!(rust_mirror::error_kind(e.as_ref()), ErrorKind::Network);
}

#[test]
fn normal_transfers_send_the_same_programmatic_proxy_credentials_as_diagnostics() {
    let _serial = SERIAL.lock().unwrap();
    let proxy = Proxy::new();
    proxy.state.lock().unwrap().expected_token = Some("YWxpY2U6cHctMTIz".into());
    let mut options = proxy.options();
    options.proxy_credentials = Some(ProxyCredentials {
        username: "alice".into(),
        password: "pw-123".into(),
    });
    let source = rust_mirror::HttpsSource::with_network(&options).unwrap();
    let url = "https://static.rust-lang.org/dist/2025-09-18/cargo.tar.gz";
    let mut bytes = vec![];
    source.fetch(url, &mut bytes, 1000).unwrap();
    assert_eq!(bytes, b"test artifact");
    assert_eq!(source.content_length(url).unwrap(), Some(13));
    let mut ranged = vec![];
    assert_eq!(
        source.fetch_from(url, &mut ranged, 1, 1000).unwrap(),
        rust_mirror::Transfer::Restart
    );
    assert!(ranged.is_empty());
    selection::resolve(&config(), "test", &source).unwrap();
    let report = check_network_with_control(&options, &Control::default()).unwrap();
    assert!(report.ok, "{report:?}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_rust-mirror"))
        .args([
            "--events",
            "jsonl",
            "check-network",
            "--proxy",
            &format!("http://{}", proxy.addr),
            "--proxy-username-env",
            "ROM_ALICE_USER",
            "--proxy-password-stdin",
            "--extra-ca-file",
        ])
        .arg(proxy.dir.path().join("ca.pem"))
        .env("ROM_ALICE_USER", "alice")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"pw-123\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("pw-123"));
    let requests = &proxy.state.lock().unwrap().requests;
    assert!(requests.iter().any(|(m, _)| m == "GET"));
    assert!(requests.iter().any(|(m, _)| m == "HEAD"));
}
