#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::{
    crates::lockfile::{parse_with_control, LockInput},
    events::Control,
};
fuzz_target!(|data: &[u8]| {
    if let Some(text) = rust_mirror_fuzz::text(data) {
        let input = LockInput {
            source_name: "fuzz".into(),
            file_name: "Cargo.lock".into(),
            text: text.into(),
        };
        let _ = parse_with_control(&input, &Control::default());
    }
});
