//! Collision observations retain exact identities; precedence never authorizes checksum merging.
use super::*;

/// Stable category of a bounded sparse-index warning.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IndexWarningKind {
    /// Historical records have the same version precedence after removing build metadata.
    BuildMetadataCollision,
}
/// One original identity in a collision observation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct CollisionVersion {
    /// Full upstream version, including build metadata.
    pub version: String,
    /// Original archive SHA-256; never coalesced with another record.
    pub checksum: String,
    /// Upstream yank state at observation.
    pub yanked: bool,
}
/// A tolerated historical collision, carried in the frozen preview for review.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct IndexWarning {
    /// Machine-readable warning category.
    pub kind: IndexWarningKind,
    /// Canonical package name.
    pub name: String,
    /// Semver identity with build metadata removed.
    pub normalized_version: String,
    /// Distinct original records, bounded to 32 members per group.
    pub versions: Vec<CollisionVersion>,
}
fn normalized(version: &str) -> Result<String> {
    if version.len() > 1024 {
        return Err(budget("index version exceeds collision-context bound"));
    }
    let mut value = Version::parse(version).map_err(|_| invalid("invalid index version"))?;
    value.build = semver::BuildMetadata::EMPTY;
    Ok(value.to_string())
}
pub(super) fn inspect<'a>(
    name: &str,
    records: impl Iterator<Item = &'a Record>,
) -> Result<Vec<IndexWarning>> {
    let mut groups: BTreeMap<String, Vec<CollisionVersion>> = BTreeMap::new();
    let mut exact = BTreeSet::new();
    for r in records {
        events::check()?;
        if r.name != name || crate::valid_hash(&r.cksum).is_err() {
            return Err(invalid(format!("inconsistent index identity for {name}")));
        }
        if !exact.insert(&r.vers) {
            return Err(invalid(format!(
                "duplicate exact index version {name}@{}",
                r.vers
            )));
        }
        let group = groups.entry(normalized(&r.vers)?).or_default();
        if group.len() >= 32 {
            return Err(budget("index collision group exceeds bound"));
        }
        group.push(CollisionVersion {
            version: r.vers.clone(),
            checksum: r.cksum.clone(),
            yanked: r.yanked,
        });
    }
    let warnings: Vec<_> = groups
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(normalized_version, mut versions)| {
            versions.sort();
            IndexWarning {
                kind: IndexWarningKind::BuildMetadataCollision,
                name: name.into(),
                normalized_version,
                versions,
            }
        })
        .collect();
    if warnings.len() > 1024 {
        return Err(budget("index collision warnings exceed bound"));
    }
    Ok(warnings)
}
pub(super) fn check(
    warnings: &[IndexWarning],
    selected: impl IntoIterator<Item = (String, String)>,
) -> Result<()> {
    if warnings.len() > 1024 {
        return Err(budget("index collision warnings exceed bound"));
    }
    let mut normalized_ids = BTreeMap::new();
    for (name, version) in selected {
        normalized_ids
            .entry((name, normalized(&version)?))
            .or_insert(version);
    }
    for w in warnings {
        index_path(&w.name)?;
        if w.versions.len() < 2
            || w.versions.len() > 32
            || w.versions.iter().any(|v| {
                normalized(&v.version).ok().as_ref() != Some(&w.normalized_version)
                    || crate::valid_hash(&v.checksum).is_err()
            })
        {
            return Err(invalid("invalid collision warning context"));
        }
        if let Some(selected_version) =
            normalized_ids.get(&(w.name.clone(), w.normalized_version.clone()))
        {
            let first = &w.versions[0];
            if let Some(other) = w
                .versions
                .iter()
                .find(|v| !v.checksum.eq_ignore_ascii_case(&first.checksum))
            {
                let mut error = Error::new(
                    ErrorKind::Invalid,
                    format!(
                        "index version collision for {}: {} ({}) and {} ({}); selected {}",
                        w.name,
                        first.version,
                        first.checksum,
                        other.version,
                        other.checksum,
                        selected_version
                    ),
                );
                let mut report = toml::Value::try_from(w)?.as_table().cloned().unwrap();
                report.insert(
                    "kind".into(),
                    toml::Value::String("index_version_collision".into()),
                );
                report.insert(
                    "selected_version".into(),
                    toml::Value::String(selected_version.clone()),
                );
                error.report = Some(toml::Value::Table(report));
                return Err(error.into());
            }
        }
    }
    Ok(())
}
pub(super) fn from_bytes(name: &str, bytes: &[u8]) -> Result<Vec<IndexWarning>> {
    let records: Vec<Record> = bytes
        .split_inclusive(|b| *b == b'\n')
        .map(serde_json::from_slice)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| invalid(format!("invalid index JSON for {name}")))?;
    inspect(name, records.iter())
}
