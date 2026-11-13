use super::*;
use crate::crates::lockfile::{LockSource, SourceKind};
/// Typed origin of protection/inclusion, independent of display prose.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SourceReasonKind {
    /// Manual catalog alias.
    Manual,
    /// Copied Cargo.lock source.
    Lock,
    /// Explicit generation policy pin.
    Pinned,
}
/// Stable source badge shared by preview/explain and retention keep reasons.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct SourceReason {
    /// Source category.
    pub kind: SourceReasonKind,
    /// Manual alias, lock source name or pinned generation path.
    pub name: String,
    /// Original lockfile SHA-256 for lock sources, otherwise null.
    pub digest: Option<String>,
}
pub(super) fn manual_sources(reasons: &[Reason], roots: &[SelectedRoot]) -> Vec<SourceReason> {
    reasons
        .iter()
        .filter_map(|r| roots.get(r.root))
        .map(|r| SourceReason {
            kind: SourceReasonKind::Manual,
            name: r.alias.clone(),
            digest: None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
pub(super) fn lock_sources(catalog: &Catalog, row: &VersionRow) -> Vec<SourceReason> {
    catalog
        .crate_locks
        .iter()
        .filter(|(_, s)| {
            s.pins
                .iter()
                .any(|p| p.name == row.name && p.version == row.version)
        })
        .map(|(name, s)| SourceReason {
            kind: SourceReasonKind::Lock,
            name: name.clone(),
            digest: Some(s.sha256.clone()),
        })
        .collect()
}
pub(super) fn native(
    source: &mut dyn IndexSource,
    profiles: &[Root],
    bounds: Bounds,
) -> Result<super::super::Plan> {
    if profiles.is_empty() {
        Ok(super::super::Plan {
            schema: 0,
            versions: vec![],
            witnesses: vec![],
            metadata_bytes: 0,
            index_requests: 0,
            archive_requests: 0,
            attempts: 0,
        })
    } else {
        super::super::plan(source, profiles, bounds)
    }
}
fn refusal(
    context: (&str, &str),
    name: &str,
    version: &str,
    parent: Option<&str>,
    requirement: Option<&str>,
    expected: Option<&str>,
    actual: Option<&str>,
) -> Box<dyn std::error::Error + Send + Sync> {
    let (kind, source) = context;
    let mut e = Error::new(
        if kind == "lock_checksum_mismatch" {
            ErrorKind::Integrity
        } else {
            ErrorKind::Invalid
        },
        "imported lock pins failed index consistency; inspect structured source context",
    );
    let mut report = toml::map::Map::new();
    for (key, value) in [
        ("kind", Some(kind)),
        ("source_name", Some(source)),
        ("name", Some(name)),
        ("version", Some(version)),
        ("parent", parent),
        ("requirement", requirement),
        ("expected_checksum", expected),
        ("actual_checksum", actual),
    ] {
        if let Some(value) = value {
            report.insert(key.into(), toml::Value::String(value.into()));
        }
    }
    e.report = Some(toml::Value::Table(report));
    e.into()
}
pub(super) fn add(
    catalog: &Catalog,
    cache: &mut Cache<'_>,
    rows: &mut Vec<VersionRow>,
    estimate: bool,
) -> Result<()> {
    for (source_name, source) in &catalog.crate_locks {
        for pin in &source.pins {
            events::check()?;
            let bytes = cache.index(&pin.name, 32 * 1024 * 1024)?;
            let parsed = records(&bytes).map_err(|_| {
                refusal(
                    ("lock_unsupported_index", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                )
            })?;
            let (ordinal, r, line) = parsed
                .into_iter()
                .find(|(_, r, _)| r.name == pin.name && r.vers == pin.version)
                .ok_or_else(|| {
                    refusal(
                        ("lock_missing_version", source_name),
                        &pin.name,
                        &pin.version,
                        None,
                        None,
                        None,
                        None,
                    )
                })?;
            if crate::valid_hash(&r.cksum).is_err() {
                return Err(refusal(
                    ("lock_unsupported_index", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                ));
            }
            if !r.cksum.eq_ignore_ascii_case(&pin.checksum) {
                return Err(refusal(
                    ("lock_checksum_mismatch", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    Some(&pin.checksum),
                    Some(&r.cksum),
                ));
            }
            if r.v == 0 || r.v > 2 {
                return Err(refusal(
                    ("lock_unsupported_index", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                ));
            }
            if let Some(row) = rows
                .iter_mut()
                .find(|r| r.name == pin.name && r.version == pin.version)
            {
                if row.index_line.as_bytes() != line {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "manual and locked index observations disagree",
                    )
                    .into());
                }
            } else {
                let expected_bytes = if estimate {
                    events::stage("measure")?;
                    let path = archive_path(&pin.name, &pin.version)?;
                    events::artifact(&path, "head", 0, None)?;
                    let size = cache.source.head(&pin.name, &pin.version)?;
                    events::complete(&path, "head", size, false)?;
                    size
                } else {
                    None
                };
                rows.push(VersionRow {
                    name: pin.name.clone(),
                    version: pin.version.clone(),
                    cksum: r.cksum,
                    yanked: r.yanked,
                    expected_bytes,
                    lock_only: true,
                    root_aliases: vec![],
                    primary_reason: None,
                    reasons: vec![],
                    retained_from_previous: false,
                    index_line: String::from_utf8(line)?,
                    source_ordinal: ordinal,
                    sources: vec![],
                    locked_yanked: false,
                });
            }
        }
    }
    for row in rows.iter_mut() {
        row.sources.extend(lock_sources(catalog, row));
        row.sources.sort();
        row.sources.dedup();
        row.locked_yanked =
            row.yanked && row.sources.iter().any(|s| s.kind == SourceReasonKind::Lock);
    }
    rows.sort_by_key(VersionRow::identity);
    validate(catalog, rows)
}
/// Validate one source against exact served records, with no expansion/default-feature inference.
pub(crate) fn covers(source_name: &str, source: &LockSource, rows: &[VersionRow]) -> Result<()> {
    let mut links = BTreeSet::new();
    for pin in &source.pins {
        events::check()?;
        let row = rows
            .iter()
            .find(|r| r.name == pin.name && r.version == pin.version)
            .ok_or_else(|| {
                refusal(
                    ("lock_missing_version", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                )
            })?;
        if !row.cksum.eq_ignore_ascii_case(&pin.checksum) {
            return Err(refusal(
                ("lock_checksum_mismatch", source_name),
                &pin.name,
                &pin.version,
                None,
                None,
                Some(&pin.checksum),
                Some(&row.cksum),
            ));
        }
        let record: Record = serde_json::from_str(row.index_line.trim_end()).map_err(|_| {
            refusal(
                ("lock_unsupported_index", source_name),
                &pin.name,
                &pin.version,
                None,
                None,
                None,
                None,
            )
        })?;
        if record.v == 0 || record.v > 2 {
            return Err(refusal(
                ("lock_unsupported_index", source_name),
                &pin.name,
                &pin.version,
                None,
                None,
                None,
                None,
            ));
        }
        if record
            .links
            .as_ref()
            .is_some_and(|link| !links.insert(link.clone()))
        {
            return Err(refusal(
                ("lock_inconsistent", source_name),
                &pin.name,
                &pin.version,
                None,
                Some("links conflict within one source"),
                None,
                None,
            ));
        }
        let deps: Vec<_> = record
            .deps
            .iter()
            .filter(|d| d.kind.as_deref() != Some("dev"))
            .collect();
        for dep in &deps {
            if index_path(dep.package()).is_err()
                || !matches!(dep.kind.as_deref(), None | Some("normal" | "build"))
            {
                return Err(refusal(
                    ("lock_unsupported_index", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                ));
            }

            if dep.registry.as_deref().is_some_and(|s| {
                s != "https://github.com/rust-lang/crates.io-index"
                    && s != "sparse+https://index.crates.io/"
            }) {
                return Err(refusal(
                    ("lock_inconsistent", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    Some("alternate index dependency"),
                    None,
                    None,
                ));
            }
            let req = VersionReq::parse(&dep.req).map_err(|_| {
                refusal(
                    ("lock_unsupported_index", source_name),
                    &pin.name,
                    &pin.version,
                    None,
                    None,
                    None,
                    None,
                )
            })?;
            if !dep.optional
                && !pin.dependencies.iter().any(|edge| {
                    edge.source_kind == SourceKind::CratesIo
                        && edge.name == dep.package()
                        && Version::parse(&edge.version).is_ok_and(|v| req.matches(&v))
                })
            {
                return Err(refusal(
                    ("lock_inconsistent", source_name),
                    dep.package(),
                    "",
                    Some(&format!("{}@{}", pin.name, pin.version)),
                    Some(&dep.req),
                    None,
                    None,
                ));
            }
        }
        for edge in pin
            .dependencies
            .iter()
            .filter(|e| e.source_kind == SourceKind::CratesIo)
        {
            if !deps.iter().any(|d| {
                d.package() == edge.name
                    && VersionReq::parse(&d.req).is_ok_and(|req| {
                        Version::parse(&edge.version).is_ok_and(|v| req.matches(&v))
                    })
            }) {
                return Err(refusal(
                    ("lock_inconsistent", source_name),
                    &edge.name,
                    &edge.version,
                    Some(&format!("{}@{}", pin.name, pin.version)),
                    None,
                    None,
                    None,
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn validate(catalog: &Catalog, rows: &[VersionRow]) -> Result<()> {
    for (name, source) in &catalog.crate_locks {
        covers(name, source, rows)?;
    }
    Ok(())
}
pub(super) fn source_changes(before: Option<&Catalog>, after: &Catalog) -> Vec<String> {
    let empty = BTreeMap::new();
    let old = before.map(|c| &c.crate_locks).unwrap_or(&empty);
    old.keys()
        .chain(after.crate_locks.keys())
        .filter(|n| old.get(*n) != after.crate_locks.get(*n))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
