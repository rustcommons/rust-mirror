use rust_mirror::crates::{self, Bounds, IndexSource, Root};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Default)]
struct Index {
    files: BTreeMap<String, Vec<u8>>,
    calls: usize,
}
impl IndexSource for Index {
    fn index(&mut self, name: &str, _limit: usize) -> rust_mirror::Result<Vec<u8>> {
        self.calls += 1;
        self.files
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unplanned index request: {name}").into())
    }
}
fn record(name: &str, version: &str, deps: Vec<Value>, features: Value) -> Value {
    json!({"name":name,"vers":version,"deps":deps,"features":features,"cksum":"0".repeat(64),"yanked":false})
}
fn dep(name: &str, req: &str) -> Value {
    json!({"name":name,"req":req,"features":[],"default_features":true,"optional":false,"target":null,"kind":"normal","registry":null})
}
fn root(name: &str) -> Root {
    Root {
        name: name.into(),
        version: "1.0.0".into(),
        features: vec![],
        default_features: true,
    }
}
fn put(index: &mut Index, name: &str, records: Vec<Value>) {
    index.files.insert(
        name.into(),
        records
            .into_iter()
            .flat_map(|v| {
                let mut s = serde_json::to_vec(&v).unwrap();
                s.push(b'\n');
                s
            })
            .collect(),
    );
}
fn ids(plan: &crates::Plan) -> Vec<String> {
    plan.versions
        .iter()
        .map(|v| format!("{}@{}", v.name, v.version))
        .collect()
}
#[test]
fn feature_union_needs_an_older_joint_candidate() {
    let mut index = Index::default();
    let mut a = dep("x", "1");
    a["features"] = json!(["a"]);
    let mut b = dep("x", "1");
    b["features"] = json!(["b"]);
    put(
        &mut index,
        "root",
        vec![record(
            "root",
            "1.0.0",
            vec![dep("left", "1"), dep("right", "1")],
            json!({}),
        )],
    );
    put(
        &mut index,
        "left",
        vec![record("left", "1.0.0", vec![a], json!({}))],
    );
    put(
        &mut index,
        "right",
        vec![record("right", "1.0.0", vec![b], json!({}))],
    );
    put(
        &mut index,
        "x",
        vec![
            record("x", "1.3.0", vec![], json!({"a":[]})),
            record("x", "1.2.0", vec![], json!({"b":[]})),
            record("x", "1.1.0", vec![], json!({"a":[],"b":[]})),
        ],
    );
    let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
    assert!(ids(&plan).contains(&"x@1.3.0".into()));
    assert!(ids(&plan).contains(&"x@1.2.0".into()));
    assert!(plan.witnesses[0].versions.contains(&"x@1.1.0".into()));
    assert_eq!(index.calls, 4);
    assert_eq!(plan.archive_requests, 0);
}
#[test]
fn prerelease_intersection_is_not_independent_maxima() {
    let mut index = Index::default();
    put(
        &mut index,
        "root",
        vec![record(
            "root",
            "1.0.0",
            vec![dep("left", "1"), dep("right", "1")],
            json!({}),
        )],
    );
    put(
        &mut index,
        "left",
        vec![record(
            "left",
            "1.0.0",
            vec![dep("x", ">=1.0.0, <1.2.0-zzz")],
            json!({}),
        )],
    );
    put(
        &mut index,
        "right",
        vec![record(
            "right",
            "1.0.0",
            vec![dep("x", ">=1.0.0, <1.3.0-zzz")],
            json!({}),
        )],
    );
    put(
        &mut index,
        "x",
        vec![
            record("x", "1.1.0", vec![], json!({})),
            record("x", "1.2.0-alpha", vec![], json!({})),
            record("x", "1.3.0-alpha", vec![], json!({})),
        ],
    );
    let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
    assert_eq!(plan.versions.len(), 6);
    assert!(plan.witnesses[0].versions.contains(&"x@1.1.0".into()));
}
#[test]
fn dep_weak_renamed_v2_and_declared_defaults_only() {
    let mut index = Index::default();
    let mut optional = dep("alias", "1");
    optional["package"] = json!("actual");
    optional["optional"] = json!(true);
    optional["default_features"] = json!(false);
    let mut rootrecord = record(
        "root",
        "1.0.0",
        vec![optional, {
            let mut d = dep("unused", "1");
            d["optional"] = json!(true);
            d
        }],
        json!({"default":["weak"],"weak":["alias?/forward"],"other":["dep:unused"]}),
    );
    rootrecord["v"] = json!(2);
    rootrecord["features2"] = json!({"activate":["dep:alias"]});
    put(&mut index, "root", vec![rootrecord]);
    put(
        &mut index,
        "actual",
        vec![record(
            "actual",
            "1.0.0",
            vec![{
                let mut d = dep("leaf", "1");
                d["optional"] = json!(true);
                d
            }],
            json!({"forward":["dep:leaf"]}),
        )],
    );
    put(
        &mut index,
        "leaf",
        vec![record("leaf", "1.0.0", vec![], json!({}))],
    );
    let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
    assert_eq!(plan.versions.len(), 3);
    assert!(plan
        .versions
        .iter()
        .filter(|v| v.name != "root")
        .all(|v| v.reasons.iter().all(|r| r.lock_only)));
    let mut selected = root("root");
    selected.features.push("activate".into());
    let plan = crates::plan(&mut index, &[selected.clone()], Bounds::default()).unwrap();
    assert_eq!(plan.versions.len(), 3);
    selected.default_features = false;
    let plan = crates::plan(&mut index, &[selected], Bounds::default()).unwrap();
    assert_eq!(plan.versions.len(), 2);
}
#[test]
fn links_require_parent_backtracking_and_dev_is_excluded() {
    let mut index = Index::default();
    let mut dev = dep("never-fetched", "1");
    dev["kind"] = json!("dev");
    put(
        &mut index,
        "root",
        vec![record(
            "root",
            "1.0.0",
            vec![dep("wrapper", "1"), dep("sys-old", "1"), dev],
            json!({}),
        )],
    );
    put(
        &mut index,
        "wrapper",
        vec![
            record("wrapper", "1.1.0", vec![dep("sys-new", "1")], json!({})),
            record("wrapper", "1.0.0", vec![dep("sys-old", "1")], json!({})),
        ],
    );
    for name in ["sys-new", "sys-old"] {
        let mut v = record(name, "1.0.0", vec![], json!({}));
        v["links"] = json!("native");
        put(&mut index, name, vec![v]);
    }
    let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
    assert!(plan.witnesses[0].versions.contains(&"wrapper@1.0.0".into()));
    assert!(!plan.witnesses[0].versions.contains(&"sys-new@1.0.0".into()));
}
#[test]
fn exact_pin_compatible_conflict_is_refused_and_independent_roots_are_separate() {
    let mut index = Index::default();
    put(
        &mut index,
        "x",
        vec![
            record("x", "1.0.0", vec![], json!({})),
            record("x", "1.1.0", vec![], json!({})),
        ],
    );
    put(
        &mut index,
        "root",
        vec![record(
            "root",
            "1.0.0",
            vec![dep("a", "1"), dep("b", "1")],
            json!({}),
        )],
    );
    for (name, req) in [("a", "=1.0.0"), ("b", "=1.1.0")] {
        put(
            &mut index,
            name,
            vec![record(name, "1.0.0", vec![dep("x", req)], json!({}))],
        );
    }
    let error = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap_err();
    assert!(error.to_string().contains("must unify"));
    let plan = crates::plan(&mut index, &[root("a"), root("b")], Bounds::default()).unwrap();
    assert_eq!(plan.witnesses.len(), 2);
}
#[test]
fn no_yanks_alternate_registry_unknown_schema_or_unbounded_work() {
    for case in 0..4 {
        let mut index = Index::default();
        let mut r = record("root", "1.0.0", vec![], json!({}));
        let mut bounds = Bounds::default();
        match case {
            0 => r["yanked"] = json!(true),
            1 => {
                let mut d = dep("x", "1");
                d["registry"] = json!("https://evil.example/index");
                r["deps"] = json!([d]);
            }
            2 => r["v"] = json!(3),
            _ => bounds.work = 0,
        }
        put(&mut index, "root", vec![r]);
        assert!(crates::plan(&mut index, &[root("root")], bounds).is_err());
    }
}
#[test]
fn original_index_record_bytes_and_target_build_edges_are_kept() {
    let mut index = Index::default();
    let mut d = dep("build-sys", "1");
    d["kind"] = json!("build");
    d["target"] = json!("cfg(windows)");
    put(
        &mut index,
        "root",
        vec![record("root", "1.0.0", vec![d], json!({}))],
    );
    put(
        &mut index,
        "build-sys",
        vec![record("build-sys", "1.0.0", vec![], json!({}))],
    );
    let original = index.files["root"].clone();
    let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
    assert_eq!(
        plan.versions
            .iter()
            .find(|v| v.name == "root")
            .unwrap()
            .index_line,
        original
    );
    assert!(plan
        .versions
        .iter()
        .find(|v| v.name == "build-sys")
        .unwrap()
        .reasons
        .iter()
        .any(|r| r.target.as_deref() == Some("cfg(windows)")));
}
#[test]
fn control_cancellation_is_typed() {
    use std::sync::Arc;
    let cancel = rust_mirror::events::CancelToken::new();
    cancel.cancel();
    struct Observer;
    impl rust_mirror::events::Observer for Observer {
        fn observe(&self, _: &rust_mirror::events::Event) -> std::io::Result<()> {
            Ok(())
        }
    }
    let control = rust_mirror::events::Control::new(Arc::new(Observer), cancel);
    let error = crates::plan_with_control(
        &mut Index::default(),
        &[root("root")],
        Bounds::default(),
        &control,
    )
    .unwrap_err();
    assert_eq!(
        rust_mirror::error_kind(error.as_ref()),
        rust_mirror::ErrorKind::Cancelled
    );
}

#[test]
fn cargo_requirement_forms_yanks_and_prereleases() {
    for (requirement, expected) in [
        ("~1.2", "1.2.9"),
        ("1.2.*", "1.2.9"),
        ("^0.1", "0.1.0"),
        (">=1.0, <1.3", "1.2.9"),
        ("*", "2.0.0"),
        ("=2.1.0-alpha", "2.1.0-alpha"),
    ] {
        let mut index = Index::default();
        put(
            &mut index,
            "root",
            vec![record(
                "root",
                "1.0.0",
                vec![dep("x", requirement)],
                json!({}),
            )],
        );
        let mut records: Vec<_> = [
            "0.1.0",
            "0.2.0",
            "1.0.0",
            "1.2.0",
            "1.2.9",
            "1.3.0",
            "2.0.0",
            "2.1.0-alpha",
        ]
        .into_iter()
        .map(|v| record("x", v, vec![], json!({})))
        .collect();
        let mut yanked = record("x", "3.0.0", vec![], json!({}));
        yanked["yanked"] = json!(true);
        records.push(yanked);
        put(&mut index, "x", records);
        let plan = crates::plan(&mut index, &[root("root")], Bounds::default()).unwrap();
        assert!(
            plan.witnesses[0]
                .versions
                .contains(&format!("x@{expected}")),
            "{requirement}"
        );
    }
}
