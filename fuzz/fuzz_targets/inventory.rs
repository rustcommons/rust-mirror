#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::{
    crates::mirror::Inventory,
    selection::{collection::CollectionSnapshot, SelectionSnapshot},
    Snapshot,
};
fuzz_target!(|data: &[u8]| {
    if let Some(text) = rust_mirror_fuzz::text(data) {
        let _ = toml::from_str::<Snapshot>(text);
        let _ = toml::from_str::<SelectionSnapshot>(text);
        let _ = toml::from_str::<CollectionSnapshot>(text);
        // The actual crate inventory reader also validates paths, schema and frozen identity.
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("snapshot.toml"), data).unwrap();
        let _ = Inventory::load(root.path());
    }
});
