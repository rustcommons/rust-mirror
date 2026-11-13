//! Research driver: exact-root JSON, private metadata cache, JSON plan. No Cargo subprocess.
use rust_mirror::crates::{self, Bounds, IndexSource, Root, SparseIndex};
use std::{fs, path::PathBuf};
struct Cache {
    path: PathBuf,
    upstream: Option<SparseIndex>,
}
impl IndexSource for Cache {
    fn index(&mut self, name: &str, limit: usize) -> rust_mirror::Result<Vec<u8>> {
        let file = self.path.join(crates::index_path(name)?);
        if file.exists() {
            if fs::symlink_metadata(&file)?.file_type().is_symlink()
                || fs::metadata(&file)?.len() > limit as u64
            {
                return Err("invalid research cache".into());
            }
            return Ok(fs::read(file)?);
        }
        let bytes = self
            .upstream
            .as_mut()
            .ok_or("offline research cache miss")?
            .index(name, limit)?;
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(file, &bytes)?;
        Ok(bytes)
    }
}
fn main() -> rust_mirror::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(args.len() == 3 || args.len() == 4 && args[3] == "--offline") {
        return Err("usage: crates_phase0 ROOTS.json CACHE_DIR NEW_PLAN.json".into());
    }
    let roots: Vec<Root> = serde_json::from_slice(&fs::read(&args[0])?)?;
    let mut source = Cache {
        path: PathBuf::from(&args[1]),
        upstream: if args.len() == 4 {
            None
        } else {
            Some(SparseIndex::new(Default::default())?)
        },
    };
    let plan = crates::plan(&mut source, &roots, Bounds::default())?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    file.write_all(&serde_json::to_vec_pretty(&plan)?)?;
    eprintln!(
        "{} versions; {} index files; {} metadata bytes; {} attempts; zero archive API",
        plan.versions.len(),
        plan.index_requests,
        plan.metadata_bytes,
        plan.attempts
    );
    Ok(())
}
