mod common;
use common::{hash, Fixture, HOST, ORIGIN};
use rust_mirror::selection::{rustup, sync, Config, Selection};
use std::{fs, path::Path};
fn config(toolchain: &str) -> Config {
    Config {
        schema: 1,
        rustup_version: "1.28.2".into(),
        max_download_bytes: 10_000_000,
        selections: vec![Selection {
            name: "test".into(),
            toolchain: toolchain.into(),
            hosts: vec![HOST.into()],
            profile: "minimal".into(),
            components: vec![],
            targets: vec![],
        }],
    }
}
fn source() -> Fixture {
    let mut f = Fixture::new();
    f.files.insert(
        format!("{ORIGIN}rustup/rustup-init.sh"),
        b"#!/bin/sh\n# original fixture\n".to_vec(),
    );
    let path = format!("{ORIGIN}rustup/archive/1.29.3/{HOST}/rustup-init");
    f.files.insert(
        format!("{path}.sha256"),
        hash(b"new bootstrap").into_bytes(),
    );
    f.files.insert(path, b"new bootstrap".to_vec());
    f
}
fn exists_no_backup(root: &Path) {
    assert_eq!(fs::read_dir(root).unwrap().count(), 1);
}
#[test]
fn complete_catalog_preserves_input_and_archives_and_validates_under_collection_reader() {
    let t = tempfile::tempdir().unwrap();
    let f = source();
    let old = sync::run(&config("1.90.0"), None, &t.path().join("old"), false, &f).unwrap();
    let bytes = fs::read(old.mirror.join("snapshot.toml")).unwrap();
    let root = t.path().join("new");
    let mirror = rustup::prepare(&old.mirror, "1.29.3", &[], &root, 10_000_000, &f).unwrap();
    assert_eq!(fs::read(old.mirror.join("snapshot.toml")).unwrap(), bytes);
    assert_eq!(
        fs::read(mirror.join(format!("rustup/archive/1.28.2/{HOST}/rustup-init"))).unwrap(),
        b"bootstrap"
    );
    assert_eq!(
        fs::read(mirror.join(format!("rustup/dist/{HOST}/rustup-init"))).unwrap(),
        b"new bootstrap"
    );
    assert!(
        fs::read_to_string(mirror.join("rustup/release-stable.toml"))
            .unwrap()
            .contains("1.29.3")
    );
    assert_eq!(
        fs::read(mirror.join("rustup/rustup-init.sh")).unwrap(),
        f.files[&format!("{ORIGIN}rustup/rustup-init.sh")]
    );
    let assembled = rust_mirror::selection::collection::assemble(
        std::slice::from_ref(&mirror),
        None,
        &t.path().join("assembled"),
        10_000_000,
    )
    .unwrap();
    assert!(assembled.join("rustup/release-stable.toml").exists());
    assert!(rustup::prepare(&old.mirror, "1.29.3", &[], &root, 10_000_000, &f).is_err());
    exists_no_backup(&root);
}
#[test]
fn bad_installer_hash_budget_nested_output_and_bad_promotion_refuse() {
    let t = tempfile::tempdir().unwrap();
    let mut f = source();
    let old = sync::run(&config("1.90.0"), None, &t.path().join("old"), false, &f).unwrap();
    assert!(rustup::prepare(
        &old.mirror,
        "1.29.3",
        &[],
        &old.mirror.join("nested"),
        10_000_000,
        &f
    )
    .is_err());
    assert!(rustup::prepare(
        &old.mirror,
        "1.29.3",
        &["stable".into()],
        &t.path().join("bad-channel"),
        10_000_000,
        &f
    )
    .is_err());
    let small = t.path().join("small");
    assert!(rustup::prepare(&old.mirror, "1.29.3", &[], &small, 1, &f).is_err());
    assert!(!small.join("mirror").exists());
    f.files.insert(
        format!("{ORIGIN}rustup/archive/1.29.3/{HOST}/rustup-init"),
        b"corrupt".to_vec(),
    );
    let bad = t.path().join("bad-hash");
    assert!(rustup::prepare(&old.mirror, "1.29.3", &[], &bad, 10_000_000, &f).is_err());
    assert!(!bad.join("mirror").exists());
}
#[test]
fn promote_only_an_existing_dated_channel_preserving_original_metadata() {
    let t = tempfile::tempdir().unwrap();
    let mut f = source();
    for suffix in ["", ".sha256", ".asc"] {
        let data = f.files[&format!("{ORIGIN}dist/channel-rust-1.90.0.toml{suffix}")].clone();
        f.files.insert(
            format!("{ORIGIN}dist/2025-09-18/channel-rust-stable.toml{suffix}"),
            data,
        );
    }
    let old = sync::run(
        &config("stable-2025-09-18"),
        None,
        &t.path().join("old"),
        false,
        &f,
    )
    .unwrap();
    let new = rustup::prepare(
        &old.mirror,
        "1.28.2",
        &["stable-2025-09-18".into()],
        &t.path().join("new"),
        10_000_000,
        &f,
    )
    .unwrap();
    for suffix in ["", ".sha256", ".asc"] {
        assert_eq!(
            fs::read(new.join(format!("dist/channel-rust-stable.toml{suffix}"))).unwrap(),
            fs::read(
                old.mirror
                    .join(format!("dist/2025-09-18/channel-rust-stable.toml{suffix}"))
            )
            .unwrap()
        );
    }
}

#[test]
fn a_self_consistent_but_wrong_catalog_cannot_pass_collection_validation() {
    let t = tempfile::tempdir().unwrap();
    let f = source();
    let old = sync::run(&config("1.90.0"), None, &t.path().join("old"), false, &f).unwrap();
    let root = rustup::prepare(
        &old.mirror,
        "1.29.3",
        &[],
        &t.path().join("new"),
        10_000_000,
        &f,
    )
    .unwrap();
    let file = root.join("rustup/release-stable.toml");
    fs::write(&file, b"schema-version='1'\nversion='1.28.2'\n").unwrap();
    let inventory = root.join("snapshot.toml");
    let mut doc: toml::Value = toml::from_str(&fs::read_to_string(&inventory).unwrap()).unwrap();
    for item in doc["files"].as_array_mut().unwrap() {
        if item["path"].as_str() == Some("rustup/release-stable.toml") {
            let data = fs::read(&file).unwrap();
            item["bytes"] = toml::Value::Integer(data.len() as i64);
            item["sha256"] = toml::Value::String(hash(&data));
        }
    }
    fs::write(&inventory, toml::to_string_pretty(&doc).unwrap()).unwrap();
    assert!(rust_mirror::selection::collection::assemble(
        &[root],
        None,
        &t.path().join("bad"),
        10_000_000
    )
    .is_err());
}
