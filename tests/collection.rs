mod common;
use common::{Fixture, HOST, ORIGIN};
use rust_mirror::selection::{collection::*, *};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
const BUDGET: u64 = 10 * 1024 * 1024;
fn make(
    root: &Path,
    name: &str,
    version: &str,
    date: &str,
    selector: &str,
    salt: &str,
    rustup: &str,
) -> PathBuf {
    let mut f = Fixture::new();
    let raw = Fixture::manifest()
        .replace("1.90.0", version)
        .replace("2025-09-18", date)
        + salt;
    f.set_manifest(raw);
    let channel = if selector.starts_with("stable") {
        "stable"
    } else if selector.starts_with("nightly") {
        "nightly"
    } else if selector.starts_with("beta") {
        "beta"
    } else {
        selector
    };
    let metadata = if selector == channel {
        format!("{ORIGIN}dist/channel-rust-{channel}.toml")
    } else {
        format!("{ORIGIN}dist/{date}/channel-rust-{channel}.toml")
    };
    f.files = f
        .files
        .into_iter()
        .map(|(p, bytes)| {
            let old = format!("{ORIGIN}dist/channel-rust-1.90.0.toml");
            let path = if let Some(suffix) = p.strip_prefix(&old) {
                format!("{metadata}{suffix}")
            } else {
                p.replace("1.90.0", version)
                    .replace("2025-09-18", date)
                    .replace("1.28.2", rustup)
            };
            (path, bytes)
        })
        .collect::<BTreeMap<_, _>>();
    let c = Config {
        schema: 1,
        retention: None,
        network: None,
        rustup_version: rustup.into(),
        max_download_bytes: BUDGET,
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
    resolve(&c, name, &f).unwrap().save(&plan).unwrap();
    fetch(&plan, &root.join(format!("{name}-output")), &f).unwrap()
}
fn sample(root: &Path, name: &str, version: &str, date: &str, selector: &str) -> PathBuf {
    make(root, name, version, date, selector, "", "1.28.2")
}
fn inventory(root: &Path) -> CollectionSnapshot {
    toml::from_str(&std::fs::read_to_string(root.join("snapshot.toml")).unwrap()).unwrap()
}
#[test]
fn combines_versions_deduplicates_inputs_and_retains_previous_generations() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "old", "1.89.0", "2025-08-07", "1.89.0");
    let b = sample(t.path(), "new", "1.90.0", "2025-09-18", "1.90.0");
    let one = assemble(
        &[a.clone(), b.clone(), a.clone()],
        None,
        &t.path().join("one"),
        BUDGET,
    )
    .unwrap();
    let s = inventory(&one);
    assert_eq!(s.toolchains.len(), 2);
    assert_eq!(s.inputs.len(), 2);
    let c = sample(t.path(), "another", "1.88.0", "2025-06-26", "1.88.0");
    let two = assemble(&[c], Some(&one), &t.path().join("two"), BUDGET).unwrap();
    assert_eq!(inventory(&two).toolchains.len(), 3);
    assert!(two
        .join("dist/2025-08-07/channel-rust-1.89.0.toml")
        .exists());
    assert!(two
        .join("dist/2025-09-18/channel-rust-1.90.0.toml")
        .exists());
    assert!(one.join("snapshot.toml").exists());
}
#[test]
fn explicit_new_stable_alias_replaces_previous_root_but_keeps_old_dated_metadata() {
    let t = tempfile::tempdir().unwrap();
    let old = sample(t.path(), "old-stable", "1.89.0", "2025-08-07", "stable");
    let new = sample(t.path(), "new-stable", "1.90.0", "2025-09-18", "stable");
    let first = assemble(&[old], None, &t.path().join("first"), BUDGET).unwrap();
    let before = std::fs::read(first.join("dist/channel-rust-stable.toml")).unwrap();
    let next = assemble(
        std::slice::from_ref(&new),
        Some(&first),
        &t.path().join("next"),
        BUDGET,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(next.join("dist/channel-rust-stable.toml")).unwrap(),
        std::fs::read(new.join("dist/channel-rust-stable.toml")).unwrap()
    );
    assert_eq!(
        std::fs::read(first.join("dist/channel-rust-stable.toml")).unwrap(),
        before
    );
    assert!(next
        .join("dist/2025-08-07/channel-rust-stable.toml")
        .exists());
    assert_eq!(inventory(&next).toolchains.len(), 2);
}
#[test]
fn conflicting_incoming_channels_and_immutable_retained_paths_refuse_before_output() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.89.0", "2025-08-07", "stable");
    let b = sample(t.path(), "b", "1.90.0", "2025-09-18", "stable");
    let out = t.path().join("out");
    assert!(assemble(&[a, b], None, &out, BUDGET).is_err());
    assert!(!out.exists());
    let a = sample(t.path(), "c", "1.90.0", "2025-09-18", "1.90.0");
    let b = make(
        t.path(),
        "d",
        "1.90.0",
        "2025-09-18",
        "1.90.0",
        "\n# different valid upstream metadata\n",
        "1.28.2",
    );
    assert!(assemble(&[b], Some(&a), &out, BUDGET).is_err());
    assert!(!out.exists());
}
#[test]
fn budget_existing_output_and_bootstrap_disagreement_refuse() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0", "2025-09-18", "1.90.0");
    let out = t.path().join("out");
    assert!(assemble(std::slice::from_ref(&a), None, &out, 1).is_err());
    assert!(!out.exists());
    let b = make(
        t.path(),
        "b",
        "1.89.0",
        "2025-08-07",
        "1.89.0",
        "",
        "1.29.0",
    );
    assert!(assemble(&[a.clone(), b], None, &out, BUDGET).is_err());
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("keep"), b"mine").unwrap();
    assert!(assemble(&[a], None, &out, BUDGET).is_err());
    assert_eq!(std::fs::read(out.join("keep")).unwrap(), b"mine");
}
#[test]
fn input_damage_unknown_files_and_missing_selected_payloads_refuse() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0", "2025-09-18", "1.90.0");
    std::fs::write(a.join("extra"), b"unrelated").unwrap();
    assert!(assemble(
        std::slice::from_ref(&a),
        None,
        &t.path().join("out"),
        BUDGET
    )
    .is_err());
    std::fs::remove_file(a.join("extra")).unwrap();
    let mut s: SelectionSnapshot =
        toml::from_str(&std::fs::read_to_string(a.join("snapshot.toml")).unwrap()).unwrap();
    let index = s
        .files
        .iter()
        .position(|f| f.path.ends_with("cargo.tar.xz"))
        .unwrap();
    let removed = s.files.remove(index);
    std::fs::remove_file(a.join(removed.path)).unwrap();
    std::fs::write(a.join("snapshot.toml"), toml::to_string(&s).unwrap()).unwrap();
    assert!(assemble(&[a], None, &t.path().join("out"), BUDGET).is_err());
}
#[test]
fn beta_nightly_and_dated_selections_preserve_selector_identity() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "beta", "1.91.0-beta.1", "2025-09-19", "beta");
    let b = sample(
        t.path(),
        "nightly",
        "1.92.0-nightly",
        "2025-09-20",
        "nightly-2025-09-20",
    );
    let out = assemble(&[a, b], None, &t.path().join("out"), BUDGET).unwrap();
    let s = inventory(&out);
    assert!(s
        .toolchains
        .iter()
        .any(|r| r.pinned_toolchain == "beta-2025-09-19"));
    assert!(s
        .toolchains
        .iter()
        .any(|r| r.pinned_toolchain == "nightly-2025-09-20"));
}
#[cfg(unix)]
#[test]
fn input_symlink_refuses_without_following_a_publishable_tree() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0", "2025-09-18", "1.90.0");
    let link = t.path().join("link");
    std::os::unix::fs::symlink(a, &link).unwrap();
    assert!(assemble(&[link], None, &t.path().join("out"), BUDGET).is_err());
}

#[test]
fn output_cannot_be_nested_in_a_source_or_previous_snapshot() {
    let t = tempfile::tempdir().unwrap();
    let a = sample(t.path(), "a", "1.90.0", "2025-09-18", "1.90.0");
    let b = sample(t.path(), "b", "1.89.0", "2025-08-07", "1.89.0");
    let nested = a.join("new-generation");
    assert!(assemble(std::slice::from_ref(&a), None, &nested, BUDGET).is_err());
    assert!(!nested.exists());
    assert!(assemble(&[b], Some(&a), &nested, BUDGET).is_err());
    assert!(!nested.exists());
}
