use rust_mirror::crates::catalog::{Catalog, DiagnosticKind};
#[test]
fn cargo_values_normalize_and_round_trip_without_gui_toml_logic() {
    let text = r#"
[crates]
serde="1"
TOKIO={package="tokio",version="1.40",features=["full"],default-features=false,optional=true}
[crates.rand_old]
package="rand"
version="0.8"
[crate-rules.serde]
latest=2
pins=["1.0.228"]
"#;
    let catalog = Catalog::parse(text).unwrap();
    assert_eq!(catalog.crates["serde"].version, "1");
    assert!(!catalog.crates["TOKIO"].default_features);
    assert!(catalog.crates["TOKIO"].optional);
    let json = serde_json::to_string(&catalog).unwrap();
    let loaded: Catalog = serde_json::from_str(&json).unwrap();
    let canonical = loaded.to_toml().unwrap();
    let reparsed = Catalog::parse(&canonical).unwrap();
    assert_eq!(
        serde_json::to_value(catalog).unwrap(),
        serde_json::to_value(reparsed).unwrap()
    );
}
#[test]
fn paste_collects_all_field_errors_and_preserves_accepted_entries() {
    let report = Catalog::from_dependencies(
        r#"[dependencies]
good="1"
one={git="https://example.invalid/repo",version="1"}
two={path="../never-read",version="1"}
three={workspace=true}
four={registry="private",version="1"}
five={version="1",surprise=true}
"#,
    );
    assert_eq!(report.accepted, vec!["good"]);
    assert_eq!(report.rejected.len(), 5);
    for kind in [
        DiagnosticKind::Git,
        DiagnosticKind::Path,
        DiagnosticKind::Workspace,
        DiagnosticKind::AlternateRegistry,
        DiagnosticKind::UnknownField,
    ] {
        assert!(report.rejected.iter().any(|d| d.kind == kind));
    }
    assert_eq!(report.catalog.unwrap().crates.len(), 1);
}
#[test]
fn whole_manifest_syntax_errors_and_wrong_types_are_typed() {
    let whole = Catalog::from_dependencies("[package]\nname='x'\n[dependencies]\nserde='1'\n");
    assert_eq!(whole.rejected[0].kind, DiagnosticKind::WholeManifest);
    assert!(whole.catalog.is_none());
    let syntax = Catalog::from_dependencies("[dependencies]\nserde = [\n");
    assert_eq!(syntax.rejected[0].kind, DiagnosticKind::Syntax);
    assert!(syntax.rejected[0].line.is_some());
    let types = Catalog::from_dependencies("[dependencies]\nserde=123\n");
    assert_eq!(types.rejected[0].kind, DiagnosticKind::InvalidValue);
}
#[test]
fn rules_unknown_keys_secrets_and_limits_are_refused() {
    for text in [
        "[crates]\na='1'\n[crate-rules.unknown]\nlatest=1",
        "[crates]\na='nonsense'",
        "[crates]\na={version='1',password='hidden'}",
        "[crates]\na='1'\n[crate-network]\nconcurrency=0",
        "[crates]\na='1'\n[crate-policy]\nfeatures='all'",
        "[crates]\na='1'\n[crate-rules.a]\npins=['not-exact']",
        "[crates]\na='1'\n[unknown]\nx=true",
    ] {
        assert!(Catalog::parse(text).is_err(), "{text}");
    }
    let err = Catalog::parse("[crates]\na={git='hidden',version='1'}").unwrap_err();
    let e = err.downcast_ref::<rust_mirror::Error>().unwrap();
    assert!(e.report.as_ref().unwrap().get("diagnostics").is_some());
    assert!(!err.to_string().contains("hidden"));
}

#[test]
fn portable_index_paths_preserve_package_case_and_refuse_device_names() {
    use rust_mirror::crates::index_path;
    assert_eq!(index_path("Inflector").unwrap(), "in/fl/inflector");
    for name in ["NUL", "con", "com1", "lpt9", "..", "x/y", "123"] {
        assert!(index_path(name).is_err(), "{name}");
    }
}
#[test]
fn concurrent_event_additions_have_typed_kinds_and_explicit_nulls() {
    use rust_mirror::events::{Event, EventKind, JsonLines, Observer, RetryKind};
    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };
    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);
    impl Write for Buffer {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let buffer = Buffer(Arc::new(Mutex::new(vec![])));
    let sink = JsonLines::new(buffer.clone());
    for (seq, kind) in [
        EventKind::ArtifactComplete {
            path: "cargo/crates/x/x-1.0.0.crate".into(),
            action: "head".into(),
            bytes: None,
            verified: false,
        },
        EventKind::CrateSummary {
            files_completed: 0,
            files_total: None,
            transferred_bytes: 0,
            reused_bytes: 0,
        },
        EventKind::Retry {
            path: "x".into(),
            attempt: 2,
            wait_seconds: 0,
            kind: RetryKind::RateLimited,
            reason: "server requested reduced request rate".into(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        sink.observe(&Event {
            seq: seq as u64,
            elapsed_ms: 0,
            kind,
        })
        .unwrap();
    }
    let data = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    let rows: Vec<serde_json::Value> = data
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows[0]["bytes"], serde_json::Value::Null);
    assert_eq!(rows[1]["files_total"], serde_json::Value::Null);
    assert_eq!(rows[2]["kind"], "rate_limited");
}

#[test]
fn canonical_save_is_explicit_and_preserves_unrelated_or_concurrent_work() {
    use std::fs;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.toml");
    let catalog = Catalog::parse("[crates]\nserde='1'\n").unwrap();
    catalog.write(&path, false).unwrap();
    assert!(catalog.write(&path, false).is_err());
    catalog.write(&path, true).unwrap();
    assert_eq!(
        Catalog::parse(&fs::read_to_string(&path).unwrap())
            .unwrap()
            .crates["serde"]
            .version,
        "1"
    );
    fs::write(&path, "user work\n").unwrap();
    assert!(catalog.write(&path, true).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "user work\n");
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(directory.path().join("catalog.toml.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let error = catalog.write(&path, true).unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(error.as_ref()),
        rust_mirror::ErrorKind::Locked
    );
}
