use rust_mirror::{Result, Source};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Write};
pub const ORIGIN: &str = "https://static.rust-lang.org/";
pub const HOST: &str = "x86_64-unknown-linux-gnu";
pub fn hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
pub struct Fixture {
    pub files: BTreeMap<String, Vec<u8>>,
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
    pub fn new() -> Self {
        let mut f = Self {
            files: BTreeMap::new(),
        };
        f.set_manifest(Self::manifest());
        for pkg in ["cargo", "rustc", "rust-std"] {
            f.files.insert(
                format!("{ORIGIN}dist/2025-09-18/{pkg}.tar.gz"),
                b"test artifact".to_vec(),
            );
        }
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
    pub fn manifest() -> String {
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
    pub fn set_manifest(&mut self, m: String) {
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
