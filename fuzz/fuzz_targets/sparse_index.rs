#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::{
    crates::{self, Bounds, IndexSource, Root},
    Result,
};
struct Input<'a>(&'a [u8]);
impl IndexSource for Input<'_> {
    fn index(&mut self, name: &str, limit: usize) -> Result<Vec<u8>> {
        if name != "fixture" || self.0.len() > limit {
            return Err("outside fuzz fixture".into());
        }
        Ok(self.0.to_vec())
    }
}
fuzz_target!(|data: &[u8]| {
    if data.len() > rust_mirror_fuzz::MAX_INPUT {
        return;
    }
    let bounds = Bounds {
        crates: 8,
        index_bytes: rust_mirror_fuzz::MAX_INPUT,
        total_metadata_bytes: 128 * 1024,
        versions: 16,
        work: 256,
        attempts: 32,
    };
    let roots = [Root {
        name: "fixture".into(),
        version: "1.0.0".into(),
        features: vec![],
        default_features: true,
    }];
    let _ = crates::plan(&mut Input(data), &roots, bounds);
});
