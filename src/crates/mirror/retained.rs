//! Flat historical closure proofs prevent recursive growth and silent version removal.
use super::*;

/// One independently checked selection observation, with no recursive predecessor.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cohort {
    /// Nonsecret catalog selecting this cohort, including its historical output URL.
    pub catalog: Catalog,
    /// UTC Unix seconds of this observation.
    pub observed_at: u64,
    /// Exact independently resolved roots.
    pub roots: Vec<SelectedRoot>,
    /// Original version records and cohort-local reason indexes.
    pub versions: Vec<VersionRow>,
    /// Native consistency witnesses.
    pub witnesses: Vec<Witness>,
    /// Source metadata observations.
    pub observations: Vec<Observation>,
    /// Original bounded warnings.
    pub warnings: Vec<String>,
}
/// Upstream yank observation for an already verified retained archive; never rewrites its original line.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionObservation {
    /// Previously mirrored version identity.
    pub identity: Identity,
    /// Current original upstream line, including its delimiter.
    pub index_line: String,
    /// Verified archive length from the predecessor inventory.
    pub bytes: u64,
    /// Yank state in the predecessor's most recent observation.
    pub previously_yanked: bool,
    /// SHA-256 of the complete current sparse index observation.
    pub index_sha256: String,
    /// Length of that complete sparse metadata file.
    pub index_bytes: u64,
}
impl Cohort {
    fn capture(plan: &FrozenPlan) -> Self {
        Self {
            catalog: plan.catalog.clone(),
            observed_at: plan.observed_at,
            roots: plan.roots.clone(),
            versions: plan.versions.clone(),
            witnesses: plan.witnesses.clone(),
            observations: plan.observations.clone(),
            warnings: plan.warnings.clone(),
        }
    }
    fn first(&self) -> Result<FrozenPlan> {
        let mut plan = FrozenPlan {
            schema: 1,
            plan_id: String::new(),
            catalog: self.catalog.clone(),
            observed_at: self.observed_at,
            roots: self.roots.clone(),
            versions: self.versions.clone(),
            witnesses: self.witnesses.clone(),
            observations: self.observations.clone(),
            counts: Counts::default(),
            size_estimate: SizeEstimate::default(),
            budgets: BudgetFlags::default(),
            changes: Changes::default(),
            previous_inventory_sha256: None,
            warnings: self.warnings.clone(),
            cohorts: Vec::new(),
            retention_observations: Vec::new(),
        };
        plan.changes.added = plan.versions.iter().map(VersionRow::identity).collect();
        let (counts, sizes, budgets) = summaries(&plan)?;
        plan.counts = counts;
        plan.size_estimate = sizes;
        plan.budgets = budgets;
        plan.plan_id = plan.computed_id()?;
        Ok(plan)
    }
}
fn same_immutable(old: &str, current: &str) -> Result<bool> {
    let mut old: serde_json::Value = serde_json::from_str(old)?;
    let mut current: serde_json::Value = serde_json::from_str(current)?;
    for value in [&mut old, &mut current] {
        value
            .as_object_mut()
            .ok_or_else(|| invalid("index record must be an object"))?
            .remove("yanked");
    }
    Ok(old == current)
}
fn merge(
    cohorts: Vec<Cohort>,
    observations: Vec<RetentionObservation>,
    previous_hash: String,
) -> Result<FrozenPlan> {
    crate::valid_hash(&previous_hash)?;
    if !(2..=128).contains(&cohorts.len()) {
        return Err(budget("retained cohort bound exceeded"));
    }
    let last = cohorts.last().unwrap();
    let mut value = last.first()?;
    value.roots.clear();
    value.versions.clear();
    value.witnesses.clear();
    let mut rows: BTreeMap<Identity, VersionRow> = BTreeMap::new();
    let mut prior = BTreeSet::new();
    let active: BTreeSet<_> = last.versions.iter().map(VersionRow::identity).collect();
    let mut metadata = BTreeMap::new();
    for (position, cohort) in cohorts.iter().enumerate() {
        events::check()?;
        let offset = value.roots.len();
        value.roots.extend(cohort.roots.clone());
        if value.roots.len() > 4096 {
            return Err(budget("historical root bound exceeded"));
        }
        value.witnesses.extend(cohort.witnesses.clone());
        for observation in &cohort.observations {
            metadata.insert(observation.name.clone(), observation.clone());
        }
        for row in &cohort.versions {
            let id = row.identity();
            if position + 1 < cohorts.len() {
                prior.insert(id.clone());
            }
            let mut row = row.clone();
            for reason in &mut row.reasons {
                reason.root += offset;
            }
            if let Some(old) = rows.get_mut(&id) {
                if old.source_ordinal != row.source_ordinal
                    || !same_immutable(&old.index_line, &row.index_line)?
                {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "retained crate metadata changed beyond its yank observation",
                    )
                    .into());
                }
                old.reasons.extend(row.reasons);
                old.root_aliases.extend(row.root_aliases);
            } else {
                rows.insert(id, row);
            }
        }
    }
    let mut observed = BTreeMap::new();
    for observation in &observations {
        if !prior.contains(&observation.identity)
            || observed
                .insert(observation.identity.clone(), observation)
                .is_some()
        {
            return Err(invalid(
                "retention observation is duplicate or not previously mirrored",
            ));
        }
        crate::valid_hash(&observation.index_sha256)?;
        if observation.index_bytes == 0 || observation.index_bytes > 32 * 1024 * 1024 {
            return Err(budget("retained metadata observation exceeds bound"));
        }
        let row = &rows[&observation.identity];
        let parsed = records(observation.index_line.as_bytes())?;
        if parsed.len() != 1
            || !observation.index_line.ends_with('\n')
            || !same_immutable(&row.index_line, &observation.index_line)?
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "retained upstream record conflicts with its immutable original",
            )
            .into());
        }
        if !observation.previously_yanked && parsed[0].1.yanked {
            value
                .changes
                .newly_yanked_retained
                .push(observation.identity.clone());
        }
    }
    if observed.len() != prior.len() {
        return Err(invalid("retained observation set is incomplete"));
    }
    value.changes.added.clear();
    for (id, mut row) in rows {
        row.reasons.sort();
        row.reasons.dedup();
        row.root_aliases.sort();
        row.root_aliases.dedup();
        row.primary_reason = row.reasons.first().cloned();
        row.lock_only = row.reasons.iter().all(|r| r.lock_only);
        row.retained_from_previous = prior.contains(&id);
        if row.retained_from_previous {
            row.expected_bytes = Some(observed[&id].bytes);
            value.changes.retained.push(id.clone());
            if !active.contains(&id) {
                value.changes.no_longer_selected_retained.push(id.clone());
            }
        } else {
            value.changes.added.push(id.clone());
        }
        value.versions.push(row);
    }
    let mut current_metadata = BTreeMap::new();
    for observation in &observations {
        let captured = Observation {
            name: observation.identity.name.clone(),
            sha256: observation.index_sha256.clone(),
            bytes: observation.index_bytes,
        };
        if let Some(old) = current_metadata.insert(captured.name.clone(), captured.clone()) {
            if old.sha256 != captured.sha256 || old.bytes != captured.bytes {
                return Err(invalid(
                    "retained records disagree about their index observation",
                ));
            }
        }
    }
    metadata.extend(current_metadata);
    value.observations = metadata.into_values().collect();
    if value.versions.len() > value.catalog.crate_policy.max_versions
        || value.observations.iter().map(|o| o.bytes).sum::<u64>()
            > value.catalog.crate_policy.max_metadata_bytes
    {
        return Err(budget(
            "retained closure exceeds current version/metadata bounds",
        ));
    }
    value.previous_inventory_sha256 = Some(previous_hash);
    value.cohorts = cohorts;
    value.retention_observations = observations;
    let (counts, sizes, budgets) = summaries(&value)?;
    value.counts = counts;
    value.size_estimate = sizes;
    value.budgets = budgets;
    value.plan_id = value.computed_id()?;
    Ok(value)
}
pub(super) fn validate(value: &FrozenPlan, replay: bool) -> Result<()> {
    if value.schema != 1 || value.catalog.network.is_some() || value.cohorts.len() > 128 {
        return Err(invalid("unsupported retained plan"));
    }
    for cohort in &value.cohorts {
        let first = cohort.first()?;
        if replay {
            first.validate()?;
        } else {
            first.inspect_metadata()?;
        }
    }
    let expected = merge(
        value.cohorts.clone(),
        value.retention_observations.clone(),
        value
            .previous_inventory_sha256
            .clone()
            .ok_or_else(|| invalid("retained plan requires its predecessor hash"))?,
    )?;
    if serde_json::to_value(expected)? != serde_json::to_value(value)? {
        return Err(Error::new(
            ErrorKind::Integrity,
            "retained preview differs from its historical proofs",
        )
        .into());
    }
    Ok(())
}
pub(super) fn plan(
    catalog: &Catalog,
    source: &dyn CrateSource,
    previous: &Path,
    estimate: bool,
) -> Result<FrozenPlan> {
    let inventory = Inventory::load(previous)?;
    let raw = fs::read(previous.join("snapshot.toml"))?;
    if raw.len() as u64 > PLAN_LIMIT {
        return Err(budget("previous inventory grew beyond bound"));
    }
    // Both current selection and retained observations share one bounded metadata view.
    struct Cached<'a> {
        source: &'a dyn CrateSource,
        files: std::sync::Mutex<BTreeMap<String, Vec<u8>>>,
        limit: u64,
    }
    impl CrateSource for Cached<'_> {
        fn index(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
            let mut files = self.files.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(bytes) = files.get(name) {
                return Ok(bytes.clone());
            }
            let bytes = self.source.index(name, limit)?;
            if bytes.len() > limit
                || files
                    .values()
                    .map(|b| b.len() as u64)
                    .sum::<u64>()
                    .saturating_add(bytes.len() as u64)
                    > self.limit
            {
                return Err(budget("combined planning metadata exceeds bound"));
            }
            files.insert(name.into(), bytes.clone());
            Ok(bytes)
        }
        fn head(&self, name: &str, version: &str) -> Result<Option<u64>> {
            self.source.head(name, version)
        }
        fn archive(
            &self,
            _: &str,
            _: &str,
            _: &mut dyn Write,
            _: u64,
            _: u64,
        ) -> Result<crate::Transfer> {
            Err(invalid("planning cannot transfer archives"))
        }
    }
    let source = Cached {
        source,
        files: std::sync::Mutex::new(BTreeMap::new()),
        limit: catalog.crate_policy.max_metadata_bytes,
    };
    let fresh = super::plan(catalog, &source, None, estimate)?;
    let mut cohorts = if inventory.plan.cohorts.is_empty() {
        vec![Cohort::capture(&inventory.plan)]
    } else {
        inventory.plan.cohorts.clone()
    };
    cohorts.push(Cohort::capture(&fresh));
    let mut cache = Cache {
        source: &source,
        files: BTreeMap::new(),
        bytes: 0,
        limit: catalog.crate_policy.max_metadata_bytes,
    };
    let mut observations = Vec::new();
    for row in &inventory.plan.versions {
        let data = cache.index(&row.name, 32 * 1024 * 1024)?;
        let (_, _, line) = records(&data)?
            .into_iter()
            .find(|(_, r, _)| r.name == row.name && r.vers == row.version)
            .ok_or_else(|| invalid("retained version disappeared from upstream metadata"))?;
        let path = archive_path(&row.name, &row.version)?;
        let bytes = inventory
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| invalid("previous archive is missing"))?
            .bytes;
        let previously_yanked = inventory
            .plan
            .retention_observations
            .iter()
            .find(|o| o.identity == row.identity())
            .map(|o| records(o.index_line.as_bytes()).map(|r| r[0].1.yanked))
            .transpose()?
            .unwrap_or(row.yanked);
        observations.push(RetentionObservation {
            identity: row.identity(),
            index_line: String::from_utf8(line)?,
            bytes,
            previously_yanked,
            index_sha256: crate::sha256(&data),
            index_bytes: data.len() as u64,
        });
    }
    let value = merge(cohorts, observations, crate::sha256(&raw))?;
    value.validate()?;
    Ok(value)
}
/// Bind the retained prefix to the actual predecessor before copying any payloads.
pub(super) fn check_previous(plan: &FrozenPlan, previous: &Inventory) -> Result<()> {
    let expected = if previous.plan.cohorts.is_empty() {
        vec![Cohort::capture(&previous.plan)]
    } else {
        previous.plan.cohorts.clone()
    };
    if plan.cohorts.len() != expected.len() + 1
        || serde_json::to_value(&plan.cohorts[..expected.len()])? != serde_json::to_value(expected)?
    {
        return Err(Error::new(
            ErrorKind::ResumeMismatch,
            "retained proof lineage differs from previous generation",
        )
        .into());
    }
    for observation in &plan.retention_observations {
        let old = previous
            .plan
            .versions
            .iter()
            .find(|r| r.identity() == observation.identity)
            .ok_or_else(|| invalid("retained version is not in predecessor"))?;
        let path = archive_path(&old.name, &old.version)?;
        let bytes = previous
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| invalid("previous archive absent"))?
            .bytes;
        let prior_yank = previous
            .plan
            .retention_observations
            .iter()
            .find(|o| o.identity == observation.identity)
            .map(|o| records(o.index_line.as_bytes()).map(|r| r[0].1.yanked))
            .transpose()?
            .unwrap_or(old.yanked);
        if observation.bytes != bytes || observation.previously_yanked != prior_yank {
            return Err(invalid(
                "retained archive observation differs from predecessor",
            ));
        }
    }
    Ok(())
}
