//! Thin local import adapter; parsing, diagnostics, diffs and canonical writing live in the library.
use super::crates::{invalid, output, read};
use crate::{
    crates::{
        catalog::Catalog,
        lockfile::{self, ImportReport},
    },
    Error, ErrorKind, Result,
};
use std::{ffi::OsString, path::PathBuf};
pub(super) fn run(command: &str, mut args: impl Iterator<Item = OsString>) -> Result<()> {
    let mut config = None;
    let mut inputs = Vec::new();
    let mut names = Vec::new();
    let mut write = false;
    let mut bytes = 0usize;
    while let Some(key) = args.next() {
        let key = key
            .into_string()
            .map_err(|_| invalid("import option must be UTF-8"))?;
        if key == "--write" {
            if write {
                return Err(invalid("duplicate write option"));
            }
            write = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| invalid("missing import option value"))?;
        match key.as_str() {
            "--config" if config.is_none() => config = Some(PathBuf::from(value)),
            "--lock" if command == "import-lock" => {
                let value = value
                    .into_string()
                    .map_err(|_| invalid("source/path must be UTF-8"))?;
                let (name, path) = value
                    .split_once('=')
                    .ok_or_else(|| invalid("lock input must be NAME=PATH"))?;
                if inputs.len() >= 128 {
                    return Err(Error::new(ErrorKind::Budget, "too many lock inputs").into());
                }
                let input = lockfile::read_input(name, std::path::Path::new(path))?;
                bytes += input.text.len();
                if bytes > lockfile::CATALOG_LIMIT {
                    return Err(Error::new(ErrorKind::Budget, "lock batch exceeds 64 MiB").into());
                }
                inputs.push(input);
            }
            "--source" if command == "remove-lock" => {
                let name = value
                    .into_string()
                    .map_err(|_| invalid("source name must be UTF-8"))?;
                lockfile::validate_source_name(&name)?;
                if names.len() >= 128 {
                    return Err(Error::new(ErrorKind::Budget, "too many source removals").into());
                }
                names.push(name);
            }
            _ => return Err(invalid("unknown/duplicate lock-import option")),
        }
    }
    let config = config.ok_or_else(|| invalid("config is required"))?;
    let original = read(&config, lockfile::CATALOG_LIMIT as u64)?;
    let catalog = Catalog::parse(&original)?;
    let control = crate::events::current().unwrap_or_default();
    let mut report: ImportReport = if command == "import-lock" {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        lockfile::preview_import(&catalog, &inputs, time, &control)?
    } else {
        lockfile::preview_remove(&catalog, &names, &control)?
    };
    if !report.accepted {
        let integrity = report
            .diagnostics
            .iter()
            .any(|d| d.kind == lockfile::DiagnosticKind::ConflictingChecksum);
        let mut e = Error::new(
            if integrity {
                ErrorKind::Integrity
            } else {
                ErrorKind::Invalid
            },
            "lock import refused; inspect typed diagnostics",
        );
        e.report = Some(toml::Value::try_from(&report)?);
        return Err(e.into());
    }
    if write {
        crate::events::stage("lock-write")?;
        crate::events::check()?;
        if read(&config, lockfile::CATALOG_LIMIT as u64)? != original {
            return Err(Error::new(
                ErrorKind::ResumeMismatch,
                "catalog changed during import; preview again",
            )
            .into());
        }
        report
            .catalog
            .as_ref()
            .ok_or_else(|| invalid("accepted import needs a proposed catalog"))?
            .write(&config, true)?;
        report.written = true;
    }
    let human = serde_json::to_string_pretty(&report)?;
    output(&report, human)
}
