//! Scoped ownership of workspace and catalog file locks.
use crate::Result;
use std::fs::File;
#[cfg(all(test, target_os = "linux"))]
use std::io;

// Explicit unlock ends ownership even when another thread's fork temporarily
// retains this open file description. Close-on-exec alone does not cover that gap.
pub(crate) struct WorkspaceLock(File);
impl WorkspaceLock {
    pub(crate) fn acquire(file: File, message: &str) -> Result<Self> {
        file.try_lock()
            .map_err(|_| crate::Error::new(crate::ErrorKind::Locked, message))?;
        Ok(Self::owned(file))
    }

    pub(crate) fn acquire_shared(file: File, message: &str) -> Result<Self> {
        file.try_lock_shared()
            .map_err(|_| crate::Error::new(crate::ErrorKind::Locked, message))?;
        Ok(Self::owned(file))
    }

    fn owned(file: File) -> Self {
        let guard = Self(file);
        #[cfg(all(test, target_os = "linux"))]
        test_support::capture(&guard.0);
        guard
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn inherited_description(&self) -> io::Result<File> {
        self.0.try_clone()
    }
}
impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static INHERITED: RefCell<Option<Vec<File>>> = const { RefCell::new(None) };
    }

    fn reset() {
        INHERITED.with(|slot| *slot.borrow_mut() = None);
    }

    pub(super) fn capture(file: &File) {
        INHERITED.with(|slot| {
            if let Some(files) = slot.borrow_mut().as_mut() {
                files.push(file.try_clone().expect("duplicate acquired test lock"));
            }
        });
    }

    /// Keep copies of this thread's acquired descriptions beyond operation return.
    pub(crate) fn inherit_during<T>(operation: impl FnOnce() -> T) -> (T, Vec<File>) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                reset();
            }
        }
        INHERITED.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "nested lock inheritance capture");
            *slot = Some(Vec::new());
        });
        let _reset = Reset;
        let result = operation();
        let files = INHERITED.with(|slot| slot.borrow_mut().take().unwrap());
        (result, files)
    }

    pub(crate) struct CrateFixture;
    impl crate::crates::source::CrateSource for CrateFixture {
        fn index(&self, name: &str, _limit: usize) -> Result<Vec<u8>> {
            assert_eq!(name, "x");
            let mut row = serde_json::to_vec(&serde_json::json!({
                "name":"x", "vers":"1.0.0", "deps":[], "features":{},
                "cksum":crate::sha256(b"payload"), "yanked":false
            }))?;
            row.push(b'\n');
            Ok(row)
        }
        fn head(&self, _name: &str, _version: &str) -> Result<Option<u64>> {
            Ok(Some(7))
        }
        fn archive(
            &self,
            _name: &str,
            _version: &str,
            out: &mut dyn std::io::Write,
            offset: u64,
            limit: u64,
        ) -> Result<crate::Transfer> {
            let bytes = &b"payload"[offset as usize..];
            assert!(bytes.len() as u64 <= limit);
            out.write_all(bytes)?;
            Ok(crate::Transfer::Complete)
        }
    }
    pub(crate) fn crate_fixture() -> (crate::crates::catalog::Catalog, CrateFixture) {
        let catalog = crate::crates::catalog::Catalog::parse(
            "[crates]\nx='=1.0.0'\n[crate-policy]\nhosts=['x86_64-unknown-linux-gnu']\ntargets=['x86_64-unknown-linux-gnu']\n[crate-output]\nbase-url='https://mirror.example/generation-001'\n",
        ).unwrap();
        (catalog, CrateFixture)
    }
}
