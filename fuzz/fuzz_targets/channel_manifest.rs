#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::{selection, Result, Source};
use sha2::{Digest, Sha256};
use std::io::Write;
struct Input<'a>(&'a [u8]);
impl Source for Input<'_> {
    fn fetch(&self, url: &str, output: &mut dyn Write, limit: u64) -> Result<()> {
        let value = match url {
            "https://static.rust-lang.org/dist/channel-rust-1.90.0.toml" => self.0.to_vec(),
            "https://static.rust-lang.org/dist/channel-rust-1.90.0.toml.sha256" => {
                hex::encode(Sha256::digest(self.0)).into_bytes()
            }
            "https://static.rust-lang.org/dist/channel-rust-1.90.0.toml.asc" => {
                b"fixture signature".to_vec()
            }
            _ => return Err("outside fuzz fixture".into()),
        };
        if value.len() as u64 > limit {
            return Err("fixture exceeds limit".into());
        }
        output.write_all(&value)?;
        Ok(())
    }
}
fuzz_target!(|data: &[u8]| {
    if data.len() > rust_mirror_fuzz::MAX_INPUT {
        return;
    }
    let _ = selection::catalog("1.90.0", Some("x86_64-unknown-linux-gnu"), &Input(data));
});
