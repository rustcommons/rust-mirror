//! Official manifest-index discovery with explicit, bounded local caching.
use crate::{events, Error, ErrorKind, Result, Source};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
const INDEX: &str = "https://static.rust-lang.org/manifests.txt";
const LIMIT: u64 = 8 * 1024 * 1024;
/// Explicit freshness/network policy; offline mode makes no source calls.
#[derive(Clone, Copy, Debug)]
pub enum CacheMode {
    /// Require a successful fresh index.
    Refresh,
    /// Read only a verified local cache.
    Offline,
    /// Refresh, falling back to a valid cache with a typed warning.
    RefreshOrCached,
}
/// An indexed selector; availability does not prove host/component support.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Release {
    /// `stable`, `beta` or `nightly`.
    pub channel: String,
    /// Numeric stable version or dated channel selector.
    pub toolchain: String,
    /// Original indexed manifest date, YYYY-MM-DD.
    pub date: String,
}
/// Bounded sanitized refresh failure, not an indication of fresh cache contents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefreshWarning {
    /// Typed refresh failure.
    pub kind: ErrorKind,
    /// Sanitized diagnostic.
    pub message: String,
}
/// Release index and explicit cache provenance; hash is not an upstream signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseCatalog {
    /// Deterministic records, newest date first.
    pub releases: Vec<Release>,
    /// Official source index URL.
    pub source_url: String,
    /// True for local cache, including fallback.
    pub cached: bool,
    /// Unix UTC seconds when the original bytes were fetched.
    pub fetched_at: u64,
    /// Nonnegative age in seconds, never a freshness guarantee.
    pub age_seconds: u64,
    /// SHA-256 of original index bytes.
    pub index_sha256: String,
    /// Failed attempted refresh if cached fallback was used.
    pub warning: Option<RefreshWarning>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cache {
    schema: u32,
    fetched_at: u64,
    sha256: String,
    raw: String,
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn parse(raw: &str) -> Result<Vec<Release>> {
    if raw.len() as u64 > LIMIT || raw.lines().count() > 100000 {
        return Err(Error::new(ErrorKind::Budget, "release index exceeds budget").into());
    }
    let mut records = BTreeSet::new();
    for line in raw.lines() {
        events::check()?;
        let Some(path) = line.strip_prefix("static.rust-lang.org/dist/") else {
            continue;
        };
        let Some((date, name)) = path.split_once('/') else {
            continue;
        };
        if !crate::selection::valid_date(date) {
            continue;
        }
        let Some(name) = name
            .strip_prefix("channel-rust-")
            .and_then(|s| s.strip_suffix(".toml"))
        else {
            continue;
        };
        let (channel, toolchain) = if ["beta", "nightly"].contains(&name) {
            (name.to_owned(), format!("{name}-{date}"))
        } else if crate::version(name).is_ok() {
            ("stable".into(), name.into())
        } else {
            continue;
        };
        records.insert(Release {
            channel,
            toolchain,
            date: date.into(),
        });
    }
    if records.is_empty() {
        return Err(Error::new(
            ErrorKind::Integrity,
            "index contains no supported dated release records",
        )
        .into());
    }
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.cmp(b)));
    Ok(records)
}
fn catalog(c: &Cache, cached: bool, warning: Option<RefreshWarning>) -> Result<ReleaseCatalog> {
    if c.schema != 1
        || crate::sha256(c.raw.as_bytes()) != c.sha256
        || c.fetched_at > now()?.saturating_add(300)
    {
        return Err(Error::new(
            ErrorKind::Integrity,
            "invalid release cache identity or timestamp",
        )
        .into());
    }
    Ok(ReleaseCatalog {
        releases: parse(&c.raw)?,
        source_url: INDEX.into(),
        cached,
        fetched_at: c.fetched_at,
        age_seconds: now()?.saturating_sub(c.fetched_at),
        index_sha256: c.sha256.clone(),
        warning,
    })
}
fn cache_file(directory: &Path) -> Result<std::path::PathBuf> {
    let meta = fs::symlink_metadata(directory)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(
            Error::new(ErrorKind::Invalid, "release cache must be a real directory").into(),
        );
    }
    for e in fs::read_dir(directory)? {
        let e = e?;
        if !["releases.toml", ".releases.lock"].contains(&e.file_name().to_str().unwrap_or(""))
            || !e.file_type()?.is_file()
        {
            return Err(Error::new(
                ErrorKind::Invalid,
                "release cache contains unrelated or linked state",
            )
            .into());
        }
    }
    Ok(directory.join("releases.toml"))
}
fn read_cache(path: &Path) -> Result<Cache> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > LIMIT * 2 {
        return Err(Error::new(ErrorKind::Integrity, "invalid release cache file").into());
    }
    let mut text = String::new();
    fs::File::open(path)?
        .take(LIMIT * 2 + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > LIMIT * 2 {
        return Err(Error::new(ErrorKind::Budget, "release cache exceeds budget").into());
    }
    toml::from_str(&text)
        .map_err(|_| Error::new(ErrorKind::Integrity, "malformed release cache").into())
}
/// Refresh or reuse the original official index, with a shared observer and cancellation.
/// Cache writes are atomic under a writer lock; offline reads never create directories.
pub fn load(
    source: &impl Source,
    directory: &Path,
    mode: CacheMode,
    control: &events::Control,
) -> Result<ReleaseCatalog> {
    control.run("releases", || {
        if !directory.try_exists()? {
            if matches!(mode, CacheMode::Offline) {
                return Err(
                    Error::new(ErrorKind::Invalid, "offline release cache does not exist").into(),
                );
            }
            fs::create_dir(directory)?;
        }
        let path = cache_file(directory)?;
        if matches!(mode, CacheMode::Offline) {
            return catalog(&read_cache(&path)?, true, None);
        }
        let lockpath = directory.join(".releases.lock");
        if lockpath.try_exists()? {
            let m = fs::symlink_metadata(&lockpath)?;
            if !m.is_file() || m.file_type().is_symlink() || m.len() != 0 {
                return Err(Error::new(ErrorKind::Invalid, "invalid cache lock").into());
            }
        }
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lockpath)?;
        lock.try_lock()
            .map_err(|_| Error::new(ErrorKind::Locked, "another process owns the release cache"))?;
        if path.try_exists()? {
            catalog(&read_cache(&path)?, true, None)?;
        }
        let mut raw = vec![];
        if let Err(e) = source.fetch(INDEX, &mut raw, LIMIT) {
            let kind = crate::error_kind(e.as_ref());
            if matches!(mode, CacheMode::RefreshOrCached)
                && matches!(
                    kind,
                    ErrorKind::Network | ErrorKind::Tls | ErrorKind::Auth | ErrorKind::Io
                )
            {
                if !path.try_exists()? {
                    return Err(e);
                }
                return catalog(
                    &read_cache(&path)?,
                    true,
                    Some(RefreshWarning {
                        kind,
                        message: "refresh failed; showing cached index".into(),
                    }),
                );
            }
            return Err(e);
        }
        if raw.len() as u64 > LIMIT {
            return Err(Error::new(ErrorKind::Budget, "release index exceeds byte budget").into());
        }
        let cache = Cache {
            schema: 1,
            fetched_at: now()?,
            sha256: crate::sha256(&raw),
            raw: String::from_utf8(raw)
                .map_err(|_| Error::new(ErrorKind::Integrity, "release index is not UTF-8"))?,
        };
        let result = catalog(&cache, false, None)?;
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(toml::to_string(&cache)?.as_bytes())?;
        temp.as_file().sync_all()?;
        events::check()?;
        temp.persist(&path)?;
        Ok(result)
    })
}
