//! Binary-only signal bridge. Handlers perform one lock-free atomic store; all
//! allocation, token notification and cleanup happen on an ordinary Rust thread.
use std::sync::atomic::{AtomicBool, Ordering};
static REQUESTED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn request(_: std::ffi::c_int) {
    REQUESTED.store(true, Ordering::SeqCst);
}
#[cfg(unix)]
fn register() -> std::io::Result<()> {
    unsafe extern "C" {
        fn signal(signal: std::ffi::c_int, handler: usize) -> usize;
    }
    // SAFETY: SIGINT/SIGTERM and the signal ABI are shared by the supported Linux/macOS
    // hosts. The static handler lives for the process lifetime and only stores an atomic.
    for number in [2, 15] {
        if unsafe { signal(number, request as usize) } == usize::MAX {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
#[cfg(windows)]
unsafe extern "system" fn request(event: u32) -> i32 {
    if [0, 1, 2, 5, 6].contains(&event) {
        REQUESTED.store(true, Ordering::SeqCst);
        1
    } else {
        0
    }
}
#[cfg(windows)]
fn register() -> std::io::Result<()> {
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }
    // SAFETY: exact Windows console-handler ABI; the handler is static and does no I/O.
    if unsafe { SetConsoleCtrlHandler(Some(request), 1) } == 0 {
        let error = std::io::Error::last_os_error();
        // Headless GUI children may have no console. Their opt-in stdin channel still works.
        if error.raw_os_error() != Some(6) {
            return Err(error);
        }
    }
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn register() -> std::io::Result<()> {
    Ok(())
}
pub fn install(cancel: rust_mirror::events::CancelToken) -> std::io::Result<()> {
    register()?;
    std::thread::spawn(move || {
        while !cancel.is_cancelled() {
            if REQUESTED.load(Ordering::SeqCst) {
                cancel.cancel();
                break;
            }
            std::thread::park_timeout(std::time::Duration::from_millis(10));
        }
    });
    Ok(())
}
