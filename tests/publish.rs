mod common;
use common::{Fixture, HOST};
use rust_mirror::{
    prepare,
    publish::{self, Options},
};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};
#[derive(Default)]
struct State {
    files: BTreeMap<String, Vec<u8>>,
    requests: Vec<(String, String)>,
    corrupt: bool,
    redirect: bool,
}
struct Server {
    addr: std::net::SocketAddr,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let stopped = stop.clone();
        let thread = thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    continue;
                }
                let words = line.split_whitespace().collect::<Vec<_>>();
                let method = words[0].to_owned();
                let path = words[1].to_owned();
                let mut len = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((key, value)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("content-length") {
                            len = value.trim().parse::<usize>().unwrap();
                        }
                    }
                }
                assert!(len < 1024 * 1024);
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let mut s = shared.lock().unwrap();
                s.requests.push((method.clone(), path.clone()));
                let (status, data) = if s.redirect {
                    (302, vec![])
                } else {
                    match method.as_str() {
                        "HEAD" => (
                            if s.files.contains_key(&path) {
                                200
                            } else {
                                404
                            },
                            vec![],
                        ),
                        "PUT" => {
                            if s.files.contains_key(&path) {
                                (409, vec![])
                            } else {
                                s.files.insert(path.clone(), body);
                                (201, vec![])
                            }
                        }
                        "GET" => match s.files.get(&path) {
                            Some(data) => (
                                200,
                                if s.corrupt {
                                    b"corrupt".to_vec()
                                } else {
                                    data.clone()
                                },
                            ),
                            None => (404, vec![]),
                        },
                        _ => panic!("unexpected method"),
                    }
                };
                write!(stream,"HTTP/1.1 {status} Status\r\nContent-Length: {}\r\nConnection: close\r\nLocation: http://127.0.0.1:1/forbidden\r\n\r\n",data.len()).unwrap();
                stream.write_all(&data).unwrap();
            }
        });
        Self {
            addr,
            state,
            stop,
            thread: Some(thread),
        }
    }
    fn options(&self) -> Options {
        Options {
            base_url: format!("http://{}/mirror", self.addr),
            ca_file: None,
            credentials: None,
            allow_loopback_http: true,
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn snapshot(root: &std::path::Path) -> std::path::PathBuf {
    prepare(
        &rust_mirror::Options {
            release: "1.90.0".into(),
            host: HOST.into(),
            rustup_version: "1.28.2".into(),
            output: root.join("run"),
        },
        &Fixture::new(),
    )
    .unwrap()
}
#[test]
fn publish_verifies_all_bytes_checks_absence_first_and_places_inventory_last() {
    let t = tempfile::tempdir().unwrap();
    let root = snapshot(t.path());
    let server = Server::new();
    let report = publish::publish(&root, &server.options()).unwrap();
    assert_eq!(report.schema, 1);
    let state = server.state.lock().unwrap();
    let puts = state
        .requests
        .iter()
        .filter(|(m, _)| m == "PUT")
        .collect::<Vec<_>>();
    assert_eq!(puts.len(), report.files);
    assert_eq!(puts.last().unwrap().1, "/mirror/snapshot.toml");
    assert!(state.requests[..report.files]
        .iter()
        .all(|(m, _)| m == "HEAD"));
    drop(state);
    assert!(publish::publish(&root, &server.options()).is_err());
    assert_eq!(
        server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|(m, _)| m == "PUT")
            .count(),
        report.files
    );
}
#[test]
fn existing_late_marker_refuses_without_any_put_and_corrupt_get_never_completes() {
    let t = tempfile::tempdir().unwrap();
    let root = snapshot(t.path());
    let server = Server::new();
    server
        .state
        .lock()
        .unwrap()
        .files
        .insert("/mirror/snapshot.toml".into(), b"existing".to_vec());
    assert!(publish::publish(&root, &server.options()).is_err());
    assert!(server
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .all(|(m, _)| m == "HEAD"));
    let bad = Server::new();
    bad.state.lock().unwrap().corrupt = true;
    assert!(publish::publish(&root, &bad.options()).is_err());
    assert!(!bad
        .state
        .lock()
        .unwrap()
        .files
        .contains_key("/mirror/snapshot.toml"));
}
#[test]
fn invalid_local_content_and_redirects_refuse_without_upload() {
    let t = tempfile::tempdir().unwrap();
    let root = snapshot(t.path());
    let redirect = Server::new();
    redirect.state.lock().unwrap().redirect = true;
    assert!(publish::publish(&root, &redirect.options()).is_err());
    assert!(redirect
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .all(|(m, _)| m == "HEAD"));
    let empty = Server::new();
    std::fs::write(root.join("dist/unexpected"), b"mine").unwrap();
    assert!(publish::publish(&root, &empty.options()).is_err());
    assert!(empty.state.lock().unwrap().requests.is_empty());
}

#[cfg(unix)]
#[test]
fn special_ca_files_refuse_before_opening_or_network() {
    let t = tempfile::tempdir().unwrap();
    let root = snapshot(t.path());
    let server = Server::new();
    let fifo = t.path().join("ca-fifo");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let mut options = server.options();
    options.base_url = "https://127.0.0.1:1/mirror".into();
    options.ca_file = Some(fifo);
    assert!(publish::publish(&root, &options).is_err());
    assert!(server.state.lock().unwrap().requests.is_empty());
}
