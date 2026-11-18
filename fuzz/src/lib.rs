//! Bounded, offline adapters; all parsing is delegated to the shipped library.
pub const MAX_INPUT: usize = 64 * 1024;
pub fn text(data: &[u8]) -> Option<&str> {
    if data.len() > MAX_INPUT {
        return None;
    }
    std::str::from_utf8(data).ok()
}
