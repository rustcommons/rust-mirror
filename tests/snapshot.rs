use rust_mirror::{prepare, Options, Result, Snapshot, Source};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Write};
const ORIGIN: &str = "https://static.rust-lang.org/";
const HOST: &str = "x86_64-unknown-linux-gnu";
fn hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
struct Fixture {
    files: BTreeMap<String, Vec<u8>>,
}
impl Source for Fixture {
    fn fetch(&self, url: &str, out: &mut dyn Write, limit: u64) -> Result<()> {
        let bytes = self
            .files
            .get(url)
            .ok_or(format!("unexpected request: {url}"))?;
        if bytes.len() as u64 > limit {
            return Err("fixture exceeds budget".into());
        }
        out.write_all(bytes)?;
        Ok(())
    }
}
impl Fixture {
    fn new() -> Self {
        let mut f = Self {
            files: BTreeMap::new(),
        };
        f.set_manifest(Self::manifest());
        for pkg in ["cargo", "rustc", "rust-std"] {
            f.files.insert(
                format!("{ORIGIN}dist/2025-09-18/{pkg}.tar.xz"),
                b"test artifact".to_vec(),
            );
        }
        let bootstrap = format!("{ORIGIN}rustup/archive/1.28.2/{HOST}/rustup-init");
        f.files.insert(
            format!("{bootstrap}.sha256"),
            hash(b"bootstrap").into_bytes(),
        );
        f.files.insert(bootstrap, b"bootstrap".to_vec());
        f.files.insert(
            format!("{ORIGIN}dist/channel-rust-1.90.0.toml.asc"),
            b"preserved unverified signature".to_vec(),
        );
        f
    }
    fn manifest() -> String {
        let mut m = format!(
            r#"manifest-version = "2"
date = "2025-09-18"
[profiles]
minimal = ["rustc", "cargo", "rust-std", "rust-mingw"]
[pkg.rust]
version = "1.90.0 (example)"
[pkg.rust.target.{HOST}]
available = true
components = [{{pkg="rustc",target="{HOST}"}},{{pkg="cargo",target="{HOST}"}},{{pkg="rust-std",target="{HOST}"}}]
"#
        );
        for pkg in ["cargo", "rustc", "rust-std"] {
            m.push_str(&format!(
                r#"
[pkg.{pkg}]
version = "1.90.0"
[pkg.{pkg}.target.{HOST}]
available = true
url = "{ORIGIN}dist/2025-09-18/{pkg}.tar.gz"
hash = "{hash}"
xz_url = "{ORIGIN}dist/2025-09-18/{pkg}.tar.xz"
xz_hash = "{hash}"
"#,
                hash = hash(b"test artifact")
            ));
        }
        m
    }
    fn set_manifest(&mut self, m: String) {
        self.files.insert(
            format!("{ORIGIN}dist/channel-rust-1.90.0.toml.sha256"),
            format!("{}  channel-rust-1.90.0.toml\n", hash(m.as_bytes())).into_bytes(),
        );
        self.files.insert(
            format!("{ORIGIN}dist/channel-rust-1.90.0.toml"),
            m.into_bytes(),
        );
    }
}
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
