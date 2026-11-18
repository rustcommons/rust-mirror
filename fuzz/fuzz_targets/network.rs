#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::network::{is_excluded, NetworkConfig};
fuzz_target!(|data: &[u8]| {
    if let Some(text) = rust_mirror_fuzz::text(data) {
        if let Ok(c) = toml::from_str::<NetworkConfig>(text) {
            let _ = c.validate();
        }
        if let Ok(c) = serde_json::from_str::<NetworkConfig>(text) {
            let _ = c.validate();
        }
        let _ = is_excluded("static.rust-lang.org", 443, &[text.into()]);
    }
});
