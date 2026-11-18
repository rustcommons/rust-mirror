#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::crates::catalog::Catalog;
fuzz_target!(|data: &[u8]| {
    if let Some(text) = rust_mirror_fuzz::text(data) {
        if let Ok(c) = Catalog::parse(text) {
            let _ = c.to_toml();
        }
        let _ = Catalog::from_dependencies(text);
        if let Ok(c) = serde_json::from_str::<Catalog>(text) {
            let _ = c.validate();
        }
    }
});
