#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_mirror::events::{Event, JsonLines, Observer};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fuzz_target!(|data: &[u8]| {
    if let Some(text) = rust_mirror_fuzz::text(data) {
        for line in text.lines().take(32) {
            if let Ok(event) = serde_json::from_str::<Event>(line) {
                let sink = Sink(Arc::new(Mutex::new(Vec::new())));
                JsonLines::new(sink.clone()).observe(&event).unwrap();
                let bytes = sink.0.lock().unwrap();
                assert_eq!(bytes.iter().filter(|&&c| c == b'\n').count(), 1);
                let _: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            }
        }
    }
});
