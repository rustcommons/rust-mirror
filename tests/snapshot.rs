mod common;
use common::{hash, Fixture, HOST, ORIGIN};
use rust_mirror::{prepare, Options, Snapshot};
fn options(dir: &std::path::Path) -> Options {
    Options {
        release: "1.90.0".into(),
        host: HOST.into(),
        rustup_version: "1.28.2".into(),
        output: dir.join("snapshot"),
    }
}
#[test]
fn complete_snapshot_preserves_metadata_and_hashes_every_file() {
    let temp = tempfile::tempdir().unwrap();
    let f = Fixture::new();
    let o = options(temp.path());
    let root = prepare(&o, &f).unwrap();
    assert_eq!(
        std::fs::read(root.join("dist/channel-rust-1.90.0.toml")).unwrap(),
        f.files[&format!("{ORIGIN}dist/channel-rust-1.90.0.toml")]
    );
    let snapshot: Snapshot =
        toml::from_str(&std::fs::read_to_string(root.join("snapshot.toml")).unwrap()).unwrap();
    assert_eq!(snapshot.files.len(), 8);
    for file in snapshot.files {
        let data = std::fs::read(root.join(file.path)).unwrap();
        assert_eq!(hash(&data), file.sha256);
        assert_eq!(data.len() as u64, file.bytes);
    }
}
#[test]
fn refuses_existing_output_without_touching_it() {
    let temp = tempfile::tempdir().unwrap();
    let o = options(temp.path());
    std::fs::create_dir(&o.output).unwrap();
    std::fs::write(o.output.join("keep"), b"old").unwrap();
    assert!(prepare(&o, &Fixture::new()).is_err());
    assert_eq!(std::fs::read(o.output.join("keep")).unwrap(), b"old");
}
#[test]
fn corrupt_artifact_never_publishes_completed_tree() {
    let temp = tempfile::tempdir().unwrap();
    let o = options(temp.path());
    let mut f = Fixture::new();
    f.files.insert(
        format!("{ORIGIN}dist/2025-09-18/rustc.tar.xz"),
        b"corrupt".to_vec(),
    );
    assert!(prepare(&o, &f).is_err());
    assert!(!o.output.join("mirror").exists());
}
#[test]
fn corrupt_manifest_is_refused_before_output_creation() {
    let temp = tempfile::tempdir().unwrap();
    let o = options(temp.path());
    let mut f = Fixture::new();
    f.files.insert(
        format!("{ORIGIN}dist/channel-rust-1.90.0.toml"),
        b"corrupt".to_vec(),
    );
    assert!(prepare(&o, &f).is_err());
    assert!(!o.output.exists());
}
#[test]
fn bootstrap_hash_is_enforced() {
    let temp = tempfile::tempdir().unwrap();
    let o = options(temp.path());
    let mut f = Fixture::new();
    f.files.insert(
        format!("{ORIGIN}rustup/archive/1.28.2/{HOST}/rustup-init"),
        b"bad".to_vec(),
    );
    assert!(prepare(&o, &f).is_err());
    assert!(!o.output.join("mirror").exists());
}
#[test]
fn wildcard_component_target_is_supported() {
    let temp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new();
    f.set_manifest(Fixture::manifest().replace(
        &format!("[pkg.rust-std.target.{HOST}]"),
        "[pkg.rust-std.target.'*']",
    ));
    assert!(prepare(&options(temp.path()), &f).is_ok());
}
#[test]
fn unsupported_manifest_shapes_and_selection_refuse() {
    for change in [
        Fixture::manifest().replace("manifest-version = \"2\"", "manifest-version = \"99\""),
        Fixture::manifest().replace("1.90.0 (example)", "1.91.0 (example)"),
        Fixture::manifest().replace("2025-09-18", "../../escape"),
        Fixture::manifest().replace("available = true", "available = false"),
        Fixture::manifest().replace("pkg=\"cargo\"", "pkg=\"absent\""),
        Fixture::manifest().replace("xz_hash =", "unused_hash ="),
        Fixture::manifest().replace(
            "https://static.rust-lang.org/dist/",
            "https://static.rust-lang.org.evil.invalid/dist/",
        ),
        Fixture::manifest().replace("cargo.tar.xz", "../cargo.tar.xz"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let o = options(temp.path());
        let mut f = Fixture::new();
        f.set_manifest(change);
        assert!(prepare(&o, &f).is_err());
        assert!(!o.output.join("mirror").exists());
    }
}
#[test]
fn validates_options_before_fetching() {
    let temp = tempfile::tempdir().unwrap();
    let mut o = options(temp.path());
    o.release = "../../escape".into();
    assert!(prepare(&o, &Fixture::new()).is_err());
    o.release = "1.90.0".into();
    o.host = "x86_64-pc-windows-msvc".into();
    assert!(prepare(&o, &Fixture::new()).is_err());
    assert!(!o.output.exists());
}

#[test]
fn minimal_profile_does_not_select_foreign_target_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new();
    let manifest = Fixture::manifest().replace(
        "components = [",
        "extensions = [{pkg=\"rust-std\",target=\"aarch64-unknown-linux-gnu\"}]\ncomponents = [",
    );
    f.set_manifest(manifest);
    assert!(prepare(&options(temp.path()), &f).is_ok());
}
