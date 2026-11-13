use super::*;
use toml::Spanned;
#[derive(Deserialize)]
struct Raw {
    version: Option<Spanned<u32>>,
    #[serde(default)]
    package: Vec<Spanned<Package>>,
    root: Option<Spanned<Package>>,
    #[serde(default)]
    metadata: BTreeMap<String, Spanned<String>>,
    patch: Option<Patch>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}
#[derive(Deserialize)]
struct Patch {
    #[serde(default)]
    unused: Vec<Spanned<Package>>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}
#[derive(Deserialize)]
struct Package {
    name: Option<Spanned<String>>,
    version: Option<Spanned<String>>,
    source: Option<Spanned<String>>,
    checksum: Option<Spanned<String>>,
    #[serde(default)]
    dependencies: Vec<Spanned<String>>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}
struct Entry {
    identity: LockedIdentity,
    source: Option<String>,
    checksum: Option<String>,
    deps: Vec<(String, usize)>,
    line: usize,
    column: usize,
}
fn reference(text: &str) -> Option<(String, Option<String>, Option<String>)> {
    let (head, source) = if let Some((head, tail)) = text.split_once(" (") {
        (head, Some(tail.strip_suffix(')')?.to_owned()))
    } else {
        (text, None)
    };
    let mut words = head.split_whitespace();
    let name = words.next()?.to_owned();
    let version = words.next().map(str::to_owned);
    if words.next().is_some() {
        return None;
    }
    Some((name, version, source))
}
pub(super) fn parse(input: &LockInput) -> Result<ParseReport> {
    let mut report = ParseReport::new(input);
    events::stage("lock-parse")?;
    if input.text.len() > INPUT_LIMIT
        || !valid_name(&input.source_name)
        || !valid_file(&input.file_name)
    {
        report.issue(
            input,
            None,
            None,
            None,
            DiagnosticKind::Budget,
            "input/name/basename exceeds supported bounds or is unsafe",
        );
        return Ok(report);
    }
    events::operation(
        &format!("lock-source:{}", input.source_name),
        &input.file_name,
    );
    let raw: Raw = match toml::from_str(&input.text) {
        Ok(raw) => raw,
        Err(e) => {
            let e: toml::de::Error = e;
            report.issue(
                input,
                None,
                None,
                e.span().map(|s| s.start),
                DiagnosticKind::Syntax,
                "invalid lock TOML or field type",
            );
            return Ok(report);
        }
    };
    let mut version = raw.version.as_ref().map(|v| *v.get_ref()).unwrap_or(1);
    if raw
        .version
        .as_ref()
        .is_some_and(|v| ![3, 4].contains(v.get_ref()))
    {
        report.issue(
            input,
            None,
            Some("version"),
            raw.version.as_ref().map(|v| v.span().start),
            DiagnosticKind::UnsupportedVersion,
            "only standard unversioned v1/v2 and versioned v3/v4 locks are accepted",
        );
    }
    for key in raw.extra.keys() {
        report.issue(
            input,
            None,
            Some(key),
            None,
            DiagnosticKind::UnknownField,
            "unsupported lock document field",
        );
    }
    let mut packages = raw.package;
    if let Some(root) = raw.root {
        packages.insert(0, root);
    }
    if packages
        .len()
        .saturating_add(raw.patch.as_ref().map(|p| p.unused.len()).unwrap_or(0))
        > 10000
    {
        report.issue(
            input,
            None,
            None,
            None,
            DiagnosticKind::Budget,
            "lock package count exceeds 10000",
        );
        return Ok(report);
    }
    let mut entries = Vec::new();
    let mut unique = BTreeSet::new();
    let mut edge_count = 0usize;
    for pkg in packages {
        events::check()?;
        let offset = pkg.span().start;
        let pkg = pkg.into_inner();
        let name = pkg.name.as_ref().map(|n| n.get_ref().as_str());
        for key in pkg.extra.keys() {
            report.issue(
                input,
                name,
                Some(key),
                Some(offset),
                DiagnosticKind::UnknownField,
                "unsupported lock package field",
            );
        }
        let (Some(n), Some(v)) = (pkg.name.as_ref(), pkg.version.as_ref()) else {
            report.issue(
                input,
                name,
                None,
                Some(offset),
                DiagnosticKind::InvalidPackage,
                "package name and exact version are required",
            );
            continue;
        };
        let source = pkg.source.as_ref().map(|s| s.get_ref().clone());
        let kind = match source.as_deref() {
            None => SourceKind::Local,
            Some(
                "registry+https://github.com/rust-lang/crates.io-index"
                | "sparse+https://index.crates.io/",
            ) => SourceKind::CratesIo,
            Some(s) if s.starts_with("git+") => SourceKind::Git,
            _ => {
                report.issue(
                    input,
                    Some(n.get_ref()),
                    Some("source"),
                    pkg.source.as_ref().map(|s| s.span().start),
                    DiagnosticKind::AlternateRegistry,
                    "registry/source is not an accepted crates.io identity",
                );
                if !report.truncated {
                    if let Some(d) = report.diagnostics.last_mut() {
                        d.registry_origin = source.as_deref().and_then(origin);
                    }
                }
                continue;
            }
        };
        if !valid_name(n.get_ref())
            || semver::Version::parse(v.get_ref()).is_err()
            || (kind == SourceKind::CratesIo && index_path(n.get_ref()).is_err())
        {
            report.issue(
                input,
                Some(n.get_ref()),
                None,
                Some(offset),
                DiagnosticKind::InvalidPackage,
                "invalid package name or exact version",
            );
            continue;
        }
        let identity = LockedIdentity {
            name: n.get_ref().clone(),
            version: v.get_ref().clone(),
            source_kind: kind.clone(),
        };
        if !unique.insert(identity.clone()) {
            report.issue(
                input,
                Some(n.get_ref()),
                None,
                Some(offset),
                DiagnosticKind::DuplicatePackage,
                "duplicate or indistinguishable package identity",
            );
        }
        let mut checksum = pkg
            .checksum
            .as_ref()
            .map(|c| c.get_ref().to_ascii_lowercase());
        if checksum.is_some() && raw.version.is_none() {
            version = 2;
        }
        if kind == SourceKind::CratesIo {
            let legacy: Vec<_> = raw
                .metadata
                .iter()
                .filter_map(|(key, value)| {
                    let (name, vers, src) = reference(key.strip_prefix("checksum ")?)?;
                    if name == identity.name
                        && vers.as_deref() == Some(&identity.version)
                        && src.as_deref() == source.as_deref()
                    {
                        Some(value)
                    } else {
                        None
                    }
                })
                .collect();
            if let Some(old) = legacy.first() {
                let value = old.get_ref().to_ascii_lowercase();
                if checksum.as_ref().is_some_and(|c| *c != value) || legacy.len() > 1 {
                    report.issue(
                        input,
                        Some(&identity.name),
                        Some("checksum"),
                        Some(old.span().start),
                        DiagnosticKind::ConflictingChecksum,
                        "recorded checksums conflict",
                    );
                } else if checksum.is_some() {
                    report.issue(
                        input,
                        Some(&identity.name),
                        Some("checksum"),
                        Some(old.span().start),
                        DiagnosticKind::ConflictingChecksum,
                        "mixed inline and legacy checksum encodings are refused",
                    );
                } else {
                    checksum = Some(value);
                }
            }
            match checksum.as_deref() {
                None | Some("<none>") => report.issue(
                    input,
                    Some(&identity.name),
                    Some("checksum"),
                    Some(offset),
                    DiagnosticKind::MissingChecksum,
                    "registry package has no recorded checksum; reimport a lock with checksums",
                ),
                Some(c) if crate::valid_hash(c).is_err() => report.issue(
                    input,
                    Some(&identity.name),
                    Some("checksum"),
                    pkg.checksum
                        .as_ref()
                        .map(|s| s.span().start)
                        .or(Some(offset)),
                    DiagnosticKind::InvalidChecksum,
                    "checksum must be SHA-256 hex",
                ),
                _ => {}
            }
        } else {
            let (line, column) = report.location(input, offset);
            report.skipped.push(SkippedEntry {
                name: identity.name.clone(),
                version: identity.version.clone(),
                kind: if kind == SourceKind::Git {
                    SkipKind::Git
                } else {
                    SkipKind::Local
                },
                line: Some(line),
                column: Some(column),
            });
        }
        edge_count += pkg.dependencies.len();
        if edge_count > 40000 {
            report.issue(
                input,
                None,
                None,
                Some(offset),
                DiagnosticKind::Budget,
                "lock edge count exceeds 40000",
            );
            return Ok(report);
        }
        let (line, column) = report.location(input, offset);
        entries.push(Entry {
            line,
            column,
            identity,
            source,
            checksum,
            deps: pkg
                .dependencies
                .into_iter()
                .map(|v| {
                    let offset = v.span().start;
                    (v.into_inner(), offset)
                })
                .collect(),
        });
    }
    if let Some(patch) = raw.patch {
        for key in patch.extra.keys() {
            report.issue(
                input,
                None,
                Some(key),
                None,
                DiagnosticKind::UnknownField,
                "unsupported patch field",
            );
        }
        for pkg in patch.unused {
            let (line, col) = report.location(input, pkg.span().start);
            let p = pkg.into_inner();
            report.skipped.push(SkippedEntry {
                name: p.name.map(Spanned::into_inner).unwrap_or_default(),
                version: p.version.map(Spanned::into_inner).unwrap_or_default(),
                kind: SkipKind::UnusedPatch,
                line: Some(line),
                column: Some(col),
            });
        }
    }
    let mut pins = vec![];
    let mut external_packages = vec![];
    for entry in &entries {
        events::check()?;
        let mut dependencies = BTreeSet::new();
        for (dep, offset) in &entry.deps {
            events::check()?;
            let parsed = reference(dep);
            let mut candidates: Vec<_> = entries
                .iter()
                .filter(|e| {
                    parsed.as_ref().is_some_and(|(name, version, source)| {
                        e.identity.name == *name
                            && version.as_ref().is_none_or(|v| *v == e.identity.version)
                            && source.as_ref().is_none_or(|s| Some(s) == e.source.as_ref())
                    })
                })
                .collect();
            if parsed.as_ref().is_some_and(|(_, _, s)| s.is_none()) {
                let locals: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|e| e.identity.source_kind == SourceKind::Local)
                    .collect();
                if locals.len() == 1 {
                    candidates = locals;
                }
            }
            if candidates.len() != 1 {
                report.issue(
                    input,
                    Some(&entry.identity.name),
                    Some("dependencies"),
                    Some(*offset),
                    if candidates.is_empty() {
                        DiagnosticKind::MissingDependency
                    } else {
                        DiagnosticKind::AmbiguousDependency
                    },
                    "dependency identity is missing or ambiguous",
                );
                continue;
            }
            if parsed.as_ref().is_some_and(|(_, v, s)| {
                v.is_none()
                    || (s.is_none() && candidates[0].identity.source_kind != SourceKind::Local)
            }) && raw.version.is_none()
            {
                version = 2;
            }
            if !dependencies.insert(candidates[0].identity.clone()) {
                report.issue(
                    input,
                    Some(&entry.identity.name),
                    Some("dependencies"),
                    Some(*offset),
                    DiagnosticKind::DuplicatePackage,
                    "duplicate resolved edge",
                );
            }
        }
        if entry.identity.source_kind == SourceKind::CratesIo {
            pins.push(LockPin {
                line: Some(entry.line),
                column: Some(entry.column),
                name: entry.identity.name.clone(),
                version: entry.identity.version.clone(),
                checksum: entry.checksum.clone().unwrap_or_default(),
                dependencies: dependencies.into_iter().collect(),
            });
        } else {
            external_packages.push(ExternalPackage {
                identity: entry.identity.clone(),
                dependencies: dependencies.into_iter().collect(),
            });
        }
    }
    pins.sort();
    external_packages.sort_by(|a, b| a.identity.cmp(&b.identity));
    report.accepted = report.total_diagnostics == 0;
    if report.accepted {
        report.source = Some(LockSource {
            file_name: input.file_name.clone(),
            sha256: crate::sha256(input.text.as_bytes()),
            imported_at: 0,
            lock_version: version,
            pins,
            external_packages,
        });
    }
    Ok(report)
}

fn origin(source: &str) -> Option<String> {
    let source = source
        .strip_prefix("registry+")
        .or_else(|| source.strip_prefix("sparse+"))?;
    let (scheme, rest) = source.split_once("://")?;
    if !matches!(scheme, "https" | "http") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    if authority.is_empty()
        || authority.len() > 512
        || !authority
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.:[]".contains(&b))
    {
        return None;
    }
    Some(format!("{scheme}://{authority}"))
}
