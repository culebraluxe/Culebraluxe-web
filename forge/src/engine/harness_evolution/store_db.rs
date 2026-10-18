//! Durable [`RevisionStore`] over migration 294's `forge_harness_*` tables.
//!
//! [`DbRevisionStore`] adapts [`db::ForgeHarnessDao`] to the engine's synchronous catalog seam:
//!
//! * revisions are written through the DAO, which recomputes the canonical manifest hash, checks
//!   the component projection and refuses a cross-domain parent;
//! * activation is delegated to the DAO's single PostgreSQL transaction
//!   (`forge_harness_activation` pointer flip + lifecycle status writes), so the catalog's D4
//!   contract holds: a failed write leaves the active pointer and every status unchanged;
//! * the store keeps a write-through view of the revisions so the catalog's borrowed `get`/`all`
//!   reads work unchanged. The view is only updated after the database write succeeds.
//!
//! # Threading contract
//!
//! The catalog and adoption seams are synchronous, so this adapter owns a private Tokio runtime
//! and blocks on the DAO. That is the same shape as the engine's `vendor_session::with_shared`
//! helpers, and it carries the same restriction: **construct and use a `DbRevisionStore` outside
//! an async runtime**. Inside a Tokio worker use the async DAO methods directly; the store is for
//! synchronous engine steps, the CLI and tests.
//!
//! The in-memory store stays the default for unit tests and experiments: this module is the
//! production adapter, not a second source of truth.

use std::collections::BTreeMap;
use std::future::Future;

use db::{
    ActivationRequest, Database, DbResult, ForgeHarnessDao, NewForgeHarnessComponent,
    NewForgeHarnessRevision,
};
use tokio::runtime::Runtime;

use super::catalog::{ActivationKind, ActivationPlan, CatalogError, RevisionStore};
use super::model::{
    AuthorityClass, ComponentKind, EvolutionBudget, EvolutionMode, HarnessDomain, HarnessManifest,
    HarnessRevision, ProposalKind, RevisionStatus,
};
use super::policy;
use super::probe_replay::{RecordedReplayOutcome, ReplayError, RevisionSource};

/// A [`RevisionStore`] backed by the applied `forge_harness_*` tables.
pub struct DbRevisionStore {
    dao: ForgeHarnessDao,
    runtime: Runtime,
    revisions: BTreeMap<String, HarnessRevision>,
}

impl DbRevisionStore {
    /// Open the store against `db`, loading every existing revision into the write-through view.
    ///
    /// Must be called outside an async runtime (it drives its own runtime to load).
    pub fn open(db: Database) -> Result<Self, CatalogError> {
        let runtime = Runtime::new().map_err(|error| {
            CatalogError::Store(format!(
                "no runtime for the durable revision store: {error}"
            ))
        })?;
        let mut store = Self {
            dao: ForgeHarnessDao::new(db),
            runtime,
            revisions: BTreeMap::new(),
        };
        store.reload()?;
        Ok(store)
    }

    pub fn dao(&self) -> &ForgeHarnessDao {
        &self.dao
    }

    /// Re-read every revision row and verify each one before trusting it.
    pub fn reload(&mut self) -> Result<(), CatalogError> {
        let rows = self.block_on(self.dao.list_all_revisions())?;
        let mut revisions = BTreeMap::new();
        for row in rows {
            let revision = stored_revision(row)?;
            if let Some(existing) = revisions.get(&revision.id) {
                return Err(CatalogError::DuplicateRevision {
                    revision_id: format!("{} (rows differ: {existing:?})", revision.id),
                });
            }
            revisions.insert(revision.id.clone(), revision);
        }
        self.revisions = revisions;
        Ok(())
    }

    /// The durable current-revision id for a domain, if an activation row exists.
    pub fn durable_current(&self, domain: HarnessDomain) -> Result<Option<String>, CatalogError> {
        Ok(self
            .block_on(self.dao.current_activation(domain.id()))?
            .map(|activation| activation.to_revision_id))
    }

    /// The activation audit rows for a domain, newest first.
    pub fn activation_history(
        &self,
        domain: HarnessDomain,
        limit: i64,
    ) -> Result<Vec<db::ForgeHarnessActivationRow>, CatalogError> {
        self.block_on(self.dao.activation_history(domain.id(), limit))
    }

    fn block_on<F, T>(&self, future: F) -> Result<T, CatalogError>
    where
        F: Future<Output = DbResult<T>>,
    {
        self.runtime
            .block_on(future)
            .map_err(|error| CatalogError::Store(format!("{error}")))
    }

    fn block_on_dao_error<F, T>(&self, future: F) -> Result<T, CatalogError>
    where
        F: Future<Output = DbResult<T>>,
    {
        self.block_on(future)
    }
}

impl RevisionStore for DbRevisionStore {
    fn insert(&mut self, revision: HarnessRevision) -> Result<(), CatalogError> {
        revision
            .verify_integrity()
            .map_err(|error| CatalogError::Integrity(error.to_string()))?;
        if revision.id.trim().is_empty() {
            return Err(CatalogError::EmptyRevisionId);
        }
        if self.revisions.contains_key(&revision.id) {
            return Err(CatalogError::DuplicateRevision {
                revision_id: revision.id,
            });
        }
        let row = revision_to_new(&revision)?;
        self.block_on_dao_error(self.dao.insert_revision(&row))?;
        // Only after the database committed does the write-through view change.
        self.revisions.insert(revision.id.clone(), revision);
        Ok(())
    }

    fn get(&self, revision_id: &str) -> Option<&HarnessRevision> {
        self.revisions.get(revision_id)
    }

    fn all(&self) -> Vec<&HarnessRevision> {
        self.revisions.values().collect()
    }

    fn len(&self) -> usize {
        self.revisions.len()
    }

    fn set_status(
        &mut self,
        revision_id: &str,
        status: RevisionStatus,
    ) -> Result<(), CatalogError> {
        if !self.revisions.contains_key(revision_id) {
            return Err(CatalogError::UnknownRevision {
                revision_id: revision_id.to_string(),
            });
        }
        self.block_on(self.dao.set_revision_status(revision_id, status.id()))?;
        if let Some(revision) = self.revisions.get_mut(revision_id) {
            revision.status = status;
        }
        Ok(())
    }

    /// Persist the pointer move through the DAO's one transaction, then mirror the lifecycle writes
    /// into the write-through view. If the transaction fails, this returns `Err` and neither the
    /// view nor the database pointer moved.
    fn apply_activation(&mut self, plan: &ActivationPlan) -> Result<(), CatalogError> {
        let request = ActivationRequest {
            domain: plan.domain.id().to_string(),
            sequence: plan.sequence as i64,
            activation_kind: plan.kind.id().to_string(),
            revision_id: plan.to_revision.clone(),
            expected_from_revision: plan.from_revision.clone(),
            decision_id: plan.decision_id.clone(),
            activated_by: plan.activated_by.clone(),
        };
        match plan.kind {
            ActivationKind::Adopt => {
                self.block_on(self.dao.activate(&request))?;
            }
            ActivationKind::Rollback => {
                self.block_on(self.dao.rollback(&request))?;
            }
        }
        // The transaction committed; mirror its status writes into the view.
        for (revision_id, next) in &plan.status_writes {
            if let Some(revision) = self.revisions.get_mut(revision_id) {
                revision.status = *next;
            }
        }
        Ok(())
    }

    fn current_activation(&self, domain: HarnessDomain) -> Result<Option<String>, CatalogError> {
        self.durable_current(domain)
    }

    fn last_activation_sequence(&self, domain: HarnessDomain) -> Result<u64, CatalogError> {
        let sequence = self.block_on(self.dao.last_activation_sequence(domain.id()))?;
        Ok(u64::try_from(sequence).unwrap_or(0))
    }
}

impl RevisionSource for DbRevisionStore {
    fn revision(&self, revision_id: &str) -> Result<HarnessRevision, ReplayError> {
        RevisionStore::get(self, revision_id)
            .cloned()
            .ok_or_else(|| ReplayError::UnknownRevision {
                revision_id: revision_id.to_string(),
            })
    }
}

/// The experiment identity a recorded replay persists under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayPersistenceMeta<'a> {
    pub experiment_id: &'a str,
    pub domain: HarnessDomain,
    pub candidate_kind: ProposalKind,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
}

/// The durable rows one completed replay writes. Built as pure values so the operator surface only
/// calls the DAO insert methods; a shape mistake is a unit-test failure, not a partial write.
#[derive(Debug, Clone)]
pub struct ReplayPersistencePlan {
    pub experiment: db::NewForgeHarnessExperiment,
    pub candidate: db::NewForgeHarnessCandidate,
    pub probes: Vec<db::NewForgeHarnessProbe>,
    pub contrast: db::NewForgeHarnessContrast,
}

impl ReplayPersistencePlan {
    pub fn experiment_id(&self) -> &str {
        &self.experiment.experiment_id
    }
}

/// Build the probe/candidate/contrast/experiment rows for a completed replay.
///
/// The plan is a projection of the replay outcome: candidate changes are the canonical
/// base→candidate diff, probe rows carry each reading's durable evidence ids and the pinned spec,
/// and the contrast row references the two probe ids. Nothing is invented: a field the replay did
/// not produce stays `None`/empty rather than a placeholder.
pub fn plan_replay_persistence(
    meta: &ReplayPersistenceMeta<'_>,
    outcome: &RecordedReplayOutcome,
) -> Result<ReplayPersistencePlan, CatalogError> {
    let experiment_id = meta.experiment_id.trim();
    if experiment_id.is_empty() {
        return Err(CatalogError::Store(
            "a recorded replay needs a non-empty experiment id".into(),
        ));
    }
    let changes = policy::changes_between(&outcome.base, &outcome.candidate);
    let changes = serde_json::to_value(&changes).map_err(|error| {
        CatalogError::Store(format!("composed changes are not serializable: {error}"))
    })?;
    let experiment = db::NewForgeHarnessExperiment {
        experiment_id: experiment_id.to_string(),
        domain: meta.domain.id().to_string(),
        base_revision_id: outcome.base.id.clone(),
        base_manifest_hash: outcome.base.manifest_hash.clone(),
        mode: meta.mode.id().to_string(),
        k: i32::try_from(meta.budget.k).unwrap_or(i32::MAX),
        max_steps: i32::try_from(meta.budget.max_steps).unwrap_or(i32::MAX),
        max_probes: i32::try_from(meta.budget.max_probes).unwrap_or(i32::MAX),
        seed: i64::try_from(meta.seed).unwrap_or(i64::MAX),
        state: "compared".to_string(),
        successor_revision_id: None,
        validation: None,
    };
    let candidate = db::NewForgeHarnessCandidate {
        candidate_id: format!("{experiment_id}-candidate"),
        experiment_id: experiment_id.to_string(),
        base_revision_id: outcome.base.id.clone(),
        proposal_kind: meta.candidate_kind.id().to_string(),
        changes,
        candidate_revision_id: Some(outcome.candidate.id.clone()),
    };
    let spec = serde_json::to_value(&outcome.spec)
        .map_err(|error| CatalogError::Store(format!("probe spec is not serializable: {error}")))?;
    let mut probes = Vec::with_capacity(2);
    for (side, reading) in [
        (super::probe::ProbeSide::Reference, &outcome.reference),
        (
            super::probe::ProbeSide::Candidate,
            &outcome.candidate_reading,
        ),
    ] {
        probes.push(db::NewForgeHarnessProbe {
            probe_id: reading.outcome.probe_id.clone(),
            experiment_id: experiment_id.to_string(),
            candidate_revision_id: Some(outcome.candidate.id.clone()),
            side: side.id().to_string(),
            revision_id: reading.outcome.revision.clone(),
            contract_digest: reading.outcome.contract_digest.clone(),
            spec: spec.clone(),
            evidence_id: Some(reading.outcome.evidence_id.clone()),
            evidence_refs: serde_json::json!(reading.outcome.evidence_refs),
            process_instance_id: None,
            story_run_id: Some(outcome.task.run_id.clone()),
            engine_task_id: None,
        });
    }
    let contrast = db::NewForgeHarnessContrast {
        contrast_id: outcome.contrast.id.clone(),
        experiment_id: experiment_id.to_string(),
        base_revision_id: outcome.base.id.clone(),
        candidate_revision_id: outcome.candidate.id.clone(),
        reference_probe_id: Some(outcome.reference.outcome.probe_id.clone()),
        candidate_probe_id: Some(outcome.candidate_reading.outcome.probe_id.clone()),
        report: serde_json::to_value(&outcome.contrast).map_err(|error| {
            CatalogError::Store(format!("contrast report is not serializable: {error}"))
        })?,
        evidence_refs: serde_json::json!(outcome.contrast.evidence_refs),
    };
    Ok(ReplayPersistencePlan {
        experiment,
        candidate,
        probes,
        contrast,
    })
}

/// Convert a model revision into the DAO's insert shape.
///
/// The component rows are the readable projection of the manifest that the DAO cross-checks before
/// writing; every field here comes from the same immutable manifest the hash covers.
pub fn revision_to_new(
    revision: &HarnessRevision,
) -> Result<NewForgeHarnessRevision, CatalogError> {
    revision
        .verify_integrity()
        .map_err(|error| CatalogError::Integrity(error.to_string()))?;
    let manifest = serde_json::to_value(&revision.manifest)
        .map_err(|error| CatalogError::Store(format!("manifest is not serializable: {error}")))?;
    let components = revision
        .manifest
        .iter()
        .map(|(kind, component)| NewForgeHarnessComponent {
            kind: kind.id().to_string(),
            authority: component.authority.id().to_string(),
            content: component.content.clone(),
            description: component.description.clone(),
        })
        .collect();
    Ok(NewForgeHarnessRevision {
        revision_id: revision.id.clone(),
        parent_revision_id: revision.parent_revision_id.clone(),
        domain: revision.domain.id().to_string(),
        status: revision.status.id().to_string(),
        manifest,
        manifest_hash: revision.manifest_hash.clone(),
        created_at: Some(revision.created_at),
        created_by: revision.created_by.clone(),
        components,
    })
}

/// Rebuild a model revision from a stored row, refusing any hash/identity mismatch.
///
/// The stored hash is checked three ways: the model recomputes it from the deserialized manifest,
/// the database's own canonical hash recomputes it from the raw jsonb, and both must equal the
/// stored value. A row edited behind the subsystem's back is a refusal, not a trusted harness.
pub fn stored_revision(row: db::ForgeHarnessRevisionRow) -> Result<HarnessRevision, CatalogError> {
    let database_hash = db::forge_harness::manifest_hash(&row.manifest)
        .map_err(|error| CatalogError::Store(format!("{error}")))?;
    if database_hash != row.manifest_hash {
        return Err(CatalogError::Integrity(format!(
            "stored revision {} has hash {} but its manifest hashes to {database_hash}",
            row.revision_id, row.manifest_hash
        )));
    }
    let domain = HarnessDomain::from_id(&row.domain).ok_or_else(|| {
        CatalogError::Store(format!(
            "stored revision {} has unknown domain {}",
            row.revision_id, row.domain
        ))
    })?;
    let status = status_from_id(&row.status).ok_or_else(|| {
        CatalogError::Store(format!(
            "stored revision {} has unknown status {}",
            row.revision_id, row.status
        ))
    })?;
    let manifest: HarnessManifest =
        serde_json::from_value(row.manifest.clone()).map_err(|error| {
            CatalogError::Store(format!(
                "stored revision {} manifest is not readable: {error}",
                row.revision_id
            ))
        })?;
    let revision = HarnessRevision {
        id: row.revision_id,
        parent_revision_id: row.parent_revision_id,
        domain,
        status,
        manifest,
        manifest_hash: row.manifest_hash,
        created_at: row.created_at,
        created_by: row.created_by,
    };
    revision
        .verify_integrity()
        .map_err(|error| CatalogError::Integrity(error.to_string()))?;
    revision.verify_against(&database_hash)?;
    Ok(revision)
}

/// Rebuild a revision from a DAO read (row + component projection), checking the projection too.
pub fn stored_revision_with_components(
    stored: db::StoredForgeHarnessRevision,
) -> Result<HarnessRevision, CatalogError> {
    let revision = stored_revision(stored.revision)?;
    for component in &stored.components {
        let kind = component_kind_from_id(&component.kind).ok_or_else(|| {
            CatalogError::Store(format!(
                "component row names unknown kind {}",
                component.kind
            ))
        })?;
        let authority = authority_from_id(&component.authority).ok_or_else(|| {
            CatalogError::Store(format!(
                "component {kind} names unknown authority {}",
                component.authority
            ))
        })?;
        let expected = revision.manifest.get(kind).ok_or_else(|| {
            CatalogError::Integrity(format!(
                "revision {} has a {kind} component row but no manifest entry",
                revision.id
            ))
        })?;
        if expected.authority != authority
            || expected.content != component.content
            || expected.description != component.description
        {
            return Err(CatalogError::Integrity(format!(
                "revision {} component {kind} does not match the manifest",
                revision.id
            )));
        }
    }
    Ok(revision)
}

fn status_from_id(id: &str) -> Option<RevisionStatus> {
    RevisionStatus::ALL
        .into_iter()
        .find(|status| status.id() == id)
}

fn component_kind_from_id(id: &str) -> Option<ComponentKind> {
    ComponentKind::ALL.into_iter().find(|kind| kind.id() == id)
}

fn authority_from_id(id: &str) -> Option<AuthorityClass> {
    AuthorityClass::ALL
        .into_iter()
        .find(|authority| authority.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{Component, QaVerdict};

    fn prompt(content: &str) -> Component {
        Component::green(ComponentKind::SystemPrompt, content, "store_db test prompt")
    }

    fn revision() -> HarnessRevision {
        HarnessRevision::draft(
            "rev-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([prompt("base prompt")]),
            "owner",
        )
    }

    /// The database duplicate of the canonical hash must agree with the model's own hash.
    #[test]
    fn the_db_manifest_hash_matches_the_model_hash() {
        let revision = revision();
        let value = serde_json::to_value(&revision.manifest).expect("manifest json");
        let database_hash = db::forge_harness::manifest_hash(&value).expect("database hash");
        assert_eq!(database_hash, revision.manifest_hash);
        assert_eq!(database_hash, revision.manifest.manifest_hash());
    }

    #[test]
    fn revision_to_new_carries_the_exact_manifest_and_component_projection() {
        let revision = revision();
        let row = revision_to_new(&revision).expect("new row");
        assert_eq!(row.revision_id, "rev-1");
        assert_eq!(row.domain, "rust-backend");
        assert_eq!(row.status, "draft");
        assert_eq!(row.manifest_hash, revision.manifest_hash);
        assert_eq!(row.components.len(), 1);
        assert_eq!(row.components[0].kind, "system_prompt");
        assert_eq!(
            db::forge_harness::manifest_hash(&row.manifest).unwrap(),
            revision.manifest_hash
        );
    }

    #[test]
    fn a_stored_row_with_a_tampered_manifest_is_refused() {
        let revision = revision();
        let mut row = revision_to_new(&revision).expect("new row");
        // Change the manifest without changing the hash: exactly what a hand-edited row looks like.
        row.manifest["components"][0]["content"] = serde_json::json!("tampered prompt");
        let stored = db::ForgeHarnessRevisionRow {
            revision_id: row.revision_id.clone(),
            parent_revision_id: None,
            domain: row.domain.clone(),
            status: row.status.clone(),
            manifest: row.manifest.clone(),
            manifest_hash: row.manifest_hash.clone(),
            created_at: revision.created_at,
            created_by: row.created_by.clone(),
        };
        let error = stored_revision(stored).expect_err("tampered manifest");
        assert!(matches!(
            error,
            CatalogError::Integrity(_) | CatalogError::Store(_)
        ));
    }

    #[test]
    fn a_stored_row_with_unknown_domain_or_status_is_refused() {
        let revision = revision();
        let row = revision_to_new(&revision).expect("new row");
        let base = db::ForgeHarnessRevisionRow {
            revision_id: row.revision_id.clone(),
            parent_revision_id: None,
            domain: "not-a-domain".into(),
            status: row.status.clone(),
            manifest: row.manifest.clone(),
            manifest_hash: row.manifest_hash.clone(),
            created_at: revision.created_at,
            created_by: row.created_by.clone(),
        };
        assert!(stored_revision(base).is_err());
        let mut second = revision_to_new(&revision).expect("new row");
        second.status = "not-a-status".into();
        let base = db::ForgeHarnessRevisionRow {
            revision_id: second.revision_id.clone(),
            parent_revision_id: None,
            domain: second.domain.clone(),
            status: second.status.clone(),
            manifest: second.manifest.clone(),
            manifest_hash: second.manifest_hash.clone(),
            created_at: revision.created_at,
            created_by: second.created_by.clone(),
        };
        assert!(stored_revision(base).is_err());
    }

    #[test]
    fn every_status_and_component_identity_maps_round_trip() {
        for status in RevisionStatus::ALL {
            assert_eq!(status_from_id(status.id()), Some(status));
        }
        for kind in ComponentKind::ALL {
            assert_eq!(component_kind_from_id(kind.id()), Some(kind));
        }
        for authority in AuthorityClass::ALL {
            assert_eq!(authority_from_id(authority.id()), Some(authority));
        }
        // A committed behavior reading still maps through the same model contract.
        let evidence = crate::engine::harness_evolution::model::BehaviorEvidence::new("e", "p");
        assert_eq!(evidence.qa, QaVerdict::NotRun);
    }

    fn replay_outcome() -> crate::engine::harness_evolution::probe_replay::RecordedReplayOutcome {
        use crate::engine::harness_evolution::model::{
            BehaviorEvidence, ProbeSpec, QaVerdict,
        };
        use crate::engine::harness_evolution::probe::{ProbeOutcome, ProbeReading, ProbeSide};
        use crate::engine::harness_evolution::probe_replay::HistoricalTask;

        let base = revision();
        let candidate = HarnessRevision::draft(
            "rev-2",
            HarnessDomain::RustBackend,
            base.manifest
                .clone()
                .with_component(prompt("candidate prompt")),
            "owner",
        )
        .with_parent(Some("rev-1".to_string()));
        let spec = ProbeSpec::new("forge-story:story-1", "rev-1", "rev-2")
            .with_model("deepseek-chat")
            .with_model_policy("default")
            .with_executor("opencode")
            .with_tools(Vec::new())
            .with_environment_class(
                crate::engine::harness_evolution::model::EnvironmentClass::Development,
            )
            .with_input_dataset("forge-harness-evolution")
            .with_test_plan("cargo test -p forge")
            .with_replay_id("replay-harness-evolution");
        let digest = "0".repeat(64);
        let reading = |probe_id: &str, side: ProbeSide, revision_id: &str| {
            let mut behavior = BehaviorEvidence::new(format!("evidence-{probe_id}"), probe_id);
            behavior.success = true;
            behavior.tests_passed = 1;
            behavior.qa = QaVerdict::Pass;
            behavior.evidence_refs = vec![format!("sandbox-command:exec:{probe_id}")];
            let outcome = ProbeOutcome {
                probe_id: probe_id.into(),
                side,
                revision: revision_id.into(),
                contract_digest: digest.clone(),
                evidence_id: behavior.evidence_id.clone(),
                evidence_refs: behavior.evidence_refs.clone(),
            };
            ProbeReading::new(outcome, behavior).expect("reading")
        };
        let reference = reading("probe-ref", ProbeSide::Reference, "rev-1");
        let candidate_reading = reading("probe-cand", ProbeSide::Candidate, "rev-2");
        let identity = spec.identity();
        let report = crate::engine::harness_evolution::contrast::contrast(
            "contrast-1",
            "rev-1",
            "rev-2",
            &reference.behavior,
            &candidate_reading.behavior,
            &identity,
            &identity,
        )
        .expect("contrast");
        crate::engine::harness_evolution::probe_replay::RecordedReplayOutcome {
            task: HistoricalTask::new("story-1", "run-1", "title", "packet"),
            base,
            candidate,
            contract_digest: digest.clone(),
            spec,
            reference,
            candidate_reading,
            contrast: report,
        }
    }

    #[test]
    fn a_replay_persistence_plan_projects_real_rows_without_inventing_fields() {
        let outcome = replay_outcome();
        let meta = super::ReplayPersistenceMeta {
            experiment_id: "exp-probe-1",
            domain: HarnessDomain::RustBackend,
            candidate_kind: crate::engine::harness_evolution::model::ProposalKind::Invariant,
            mode: crate::engine::harness_evolution::model::EvolutionMode::Parallel,
            budget: crate::engine::harness_evolution::model::EvolutionBudget::k1(),
            seed: 7,
        };
        let plan = super::plan_replay_persistence(&meta, &outcome).expect("plan");
        assert_eq!(plan.experiment.experiment_id, "exp-probe-1");
        assert_eq!(plan.experiment.state, "compared");
        assert_eq!(plan.experiment.domain, "rust-backend");
        assert_eq!(plan.experiment.k, 1);
        assert_eq!(plan.experiment.seed, 7);
        assert_eq!(plan.candidate.proposal_kind, "invariant");
        assert!(
            plan.candidate
                .changes
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "candidate changes are the canonical base->candidate diff"
        );
        assert_eq!(
            plan.candidate.candidate_revision_id.as_deref(),
            Some("rev-2")
        );
        assert_eq!(plan.probes.len(), 2);
        let reference = plan
            .probes
            .iter()
            .find(|probe| probe.side == "reference")
            .expect("reference probe");
        assert_eq!(reference.revision_id, "rev-1");
        assert_eq!(reference.evidence_id.as_deref(), Some("evidence-probe-ref"));
        assert_eq!(reference.story_run_id.as_deref(), Some("run-1"));
        assert_eq!(
            reference.evidence_refs,
            serde_json::json!(["sandbox-command:exec:probe-ref"])
        );
        assert_eq!(
            plan.contrast.reference_probe_id.as_deref(),
            Some("probe-ref")
        );
        assert_eq!(
            plan.contrast.candidate_probe_id.as_deref(),
            Some("probe-cand")
        );
        assert_eq!(plan.contrast.contrast_id, "contrast-1");
    }

    /// DEV smoke test for the O2 persistence path: insert a real experiment/candidate/probe/contrast
    /// set through the DAO, read it back, then remove every row. Ignored by default (needs
    /// DATABASE_URL_DEV); migration 294 is applied on DEV.
    #[test]
    #[ignore = "needs DATABASE_URL_DEV; keeps the unit suite database-free"]
    fn dev_smoke_replay_persistence_rows_and_cleanup() {
        use crate::engine::harness_evolution::model::{
            BehaviorEvidence, HarnessDomain, ProbeSpec, QaVerdict,
        };
        use crate::engine::harness_evolution::probe::{ProbeOutcome, ProbeReading, ProbeSide};
        use crate::engine::harness_evolution::probe_replay::HistoricalTask;

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let database = runtime
            .block_on(db::Database::connect_target(db::DbTarget::Dev))
            .expect("connect DEV");
        let dao = db::ForgeHarnessDao::new(database.clone());
        let suffix = uuid::Uuid::new_v4();
        let base = HarnessRevision::draft(
            format!("smoke-base-{suffix}"),
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([prompt("base prompt")]),
            "smoke",
        );
        let candidate = HarnessRevision::draft(
            format!("smoke-cand-{suffix}"),
            HarnessDomain::RustBackend,
            base.manifest
                .clone()
                .with_component(prompt("candidate prompt")),
            "smoke",
        )
        .with_parent(Some(base.id.clone()));
        // probe.story_run_id is an FK to a real storyboard_story_run when present. Prefer a real
        // DEV run id; if DEV has none, the plan's probe FK is nulled below rather than invented.
        let existing_run: Option<String> = runtime
            .block_on(async {
                sqlx::query_scalar::<_, String>("select id::text from storyboard_story_run limit 1")
                    .fetch_optional(database.pool())
                    .await
            })
            .ok()
            .flatten();
        let run_id = existing_run
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let spec = ProbeSpec::new("forge-story:smoke", &base.id, &candidate.id)
            .with_model("deepseek-chat")
            .with_model_policy("default")
            .with_executor("opencode")
            .with_tools(Vec::new())
            .with_environment_class(
                crate::engine::harness_evolution::model::EnvironmentClass::Development,
            )
            .with_input_dataset("forge-harness-evolution")
            .with_test_plan("cargo test -p forge")
            .with_replay_id("replay-harness-evolution");
        let digest = "0".repeat(64);
        let reading = |probe_id: String, side: ProbeSide, revision_id: &str| {
            let mut behavior = BehaviorEvidence::new(format!("evidence-{probe_id}"), &probe_id);
            behavior.success = true;
            behavior.qa = QaVerdict::Pass;
            behavior.evidence_refs = vec![format!("sandbox-command:exec:{probe_id}")];
            let outcome = ProbeOutcome {
                probe_id: probe_id.clone(),
                side,
                revision: revision_id.into(),
                contract_digest: digest.clone(),
                evidence_id: behavior.evidence_id.clone(),
                evidence_refs: behavior.evidence_refs.clone(),
            };
            ProbeReading::new(outcome, behavior).expect("reading")
        };
        let reference = reading(
            format!("smoke-probe-ref-{suffix}"),
            ProbeSide::Reference,
            &base.id,
        );
        let candidate_reading = reading(
            format!("smoke-probe-cand-{suffix}"),
            ProbeSide::Candidate,
            &candidate.id,
        );
        let identity = spec.identity();
        let report = crate::engine::harness_evolution::contrast::contrast(
            format!("smoke-contrast-{suffix}"),
            &base.id,
            &candidate.id,
            &reference.behavior,
            &candidate_reading.behavior,
            &identity,
            &identity,
        )
        .expect("contrast");
        let outcome = crate::engine::harness_evolution::probe_replay::RecordedReplayOutcome {
            task: HistoricalTask::new("smoke-story", &run_id, "smoke", "packet"),
            base: base.clone(),
            candidate: candidate.clone(),
            contract_digest: digest,
            spec,
            reference,
            candidate_reading,
            contrast: report,
        };
        let experiment_id = format!("smoke-exp-{suffix}");
        let meta = ReplayPersistenceMeta {
            experiment_id: &experiment_id,
            domain: HarnessDomain::RustBackend,
            candidate_kind: crate::engine::harness_evolution::model::ProposalKind::Invariant,
            mode: crate::engine::harness_evolution::model::EvolutionMode::Parallel,
            budget: crate::engine::harness_evolution::model::EvolutionBudget::k1(),
            seed: 0,
        };
        let mut rows = plan_replay_persistence(&meta, &outcome).expect("plan");
        if existing_run.is_none() {
            for probe in &mut rows.probes {
                probe.story_run_id = None;
            }
        }

        let outcome_result: Result<(), String> = (|| {
            for revision in [&outcome.base, &outcome.candidate] {
                let new =
                    revision_to_new(revision).map_err(|error| format!("revision row: {error}"))?;
                runtime
                    .block_on(dao.insert_revision(&new))
                    .map_err(|error| format!("insert revision: {error}"))?;
            }
            runtime
                .block_on(dao.insert_experiment(&rows.experiment))
                .map_err(|error| format!("insert experiment: {error}"))?;
            runtime
                .block_on(dao.insert_candidate(&rows.candidate))
                .map_err(|error| format!("insert candidate: {error}"))?;
            for probe in &rows.probes {
                runtime
                    .block_on(dao.insert_probe(probe))
                    .map_err(|error| format!("insert probe: {error}"))?;
            }
            runtime
                .block_on(dao.insert_contrast(&rows.contrast))
                .map_err(|error| format!("insert contrast: {error}"))?;
            let probes = runtime
                .block_on(dao.list_probes(&experiment_id))
                .map_err(|error| format!("read probes: {error}"))?;
            if probes.len() != 2 {
                return Err(format!("expected 2 probes, got {}", probes.len()));
            }
            let experiment = runtime
                .block_on(dao.get_experiment(&experiment_id))
                .map_err(|error| format!("read experiment: {error}"))?
                .ok_or_else(|| "experiment missing after insert".to_string())?;
            if experiment.state != "compared" {
                return Err(format!("unexpected experiment state {}", experiment.state));
            }
            let contrasts = runtime
                .block_on(dao.list_contrasts(&experiment_id))
                .map_err(|error| format!("read contrasts: {error}"))?;
            if contrasts.len() != 1 {
                return Err(format!("expected 1 contrast, got {}", contrasts.len()));
            }
            Ok(())
        })();

        // Cleanup always runs, in FK order.
        let cleanup = runtime.block_on(async {
            sqlx::query("delete from forge_harness_contrast where experiment_id=$1")
                .bind(&experiment_id)
                .execute(database.pool())
                .await?;
            sqlx::query("delete from forge_harness_probe where experiment_id=$1")
                .bind(&experiment_id)
                .execute(database.pool())
                .await?;
            sqlx::query("delete from forge_harness_candidate where experiment_id=$1")
                .bind(&experiment_id)
                .execute(database.pool())
                .await?;
            for revision in [&outcome.candidate, &outcome.base] {
                sqlx::query("delete from forge_harness_component where revision_id=$1")
                    .bind(&revision.id)
                    .execute(database.pool())
                    .await?;
            }
            sqlx::query("delete from forge_harness_experiment where experiment_id=$1")
                .bind(&experiment_id)
                .execute(database.pool())
                .await?;
            // Children first: a child revision references its parent ON DELETE RESTRICT.
            for revision in [&outcome.candidate, &outcome.base] {
                sqlx::query("delete from forge_harness_revision where revision_id=$1")
                    .bind(&revision.id)
                    .execute(database.pool())
                    .await?;
            }
            Ok::<(), sqlx::Error>(())
        });
        if let Err(error) = cleanup {
            panic!("smoke cleanup failed: {error}");
        }
        if let Err(error) = outcome_result {
            panic!("O2 persistence smoke failed: {error}");
        }
    }

    /// DEV smoke test: proves migration 294's tables exist and one revision can be inserted and read
    /// through the DAO, then removes the row. Ignored by default (needs DATABASE_URL_DEV):
    ///
    /// ```sh
    /// set -a; . /Users/Shared/dev/.env.local; set +a
    /// cargo test -p forge --lib harness_evolution::store_db -- --ignored
    /// ```
    #[test]
    #[ignore = "needs DATABASE_URL_DEV; keeps the unit suite database-free"]
    fn dev_smoke_revision_roundtrip_and_cleanup() {
        let runtime = Runtime::new().expect("runtime");
        let database = runtime
            .block_on(db::Database::connect_target(db::DbTarget::Dev))
            .expect("connect DEV");
        let tables = runtime
            .block_on(async {
                sqlx::query_as::<_, (Option<String>,)>("select to_regclass($1)::text")
                    .bind("public.forge_harness_revision")
                    .fetch_one(database.pool())
                    .await
            })
            .expect("table lookup");
        assert!(tables.0.is_some(), "migration 294 must be applied on DEV");

        let revision_id = format!("smoke-{}", uuid::Uuid::new_v4());
        let mut revision = revision();
        revision.id = revision_id.clone();

        let outcome: Result<(String, String), String> = (|| {
            let mut store = DbRevisionStore::open(database.clone())
                .map_err(|error| format!("open store: {error}"))?;
            store
                .insert(revision.clone())
                .map_err(|error| format!("insert: {error}"))?;
            let read = store.get(&revision_id).ok_or_else(|| {
                "inserted revision missing from the write-through view".to_string()
            })?;
            Ok((read.id.clone(), read.manifest_hash.clone()))
        })();

        // Cleanup always runs, even if the smoke assertions above failed.
        let cleanup = runtime.block_on(async {
            sqlx::query("delete from forge_harness_component where revision_id=$1")
                .bind(&revision_id)
                .execute(database.pool())
                .await?;
            sqlx::query("delete from forge_harness_revision where revision_id=$1")
                .bind(&revision_id)
                .execute(database.pool())
                .await
        });
        assert!(cleanup.is_ok(), "smoke cleanup must remove its row");

        let (id, hash) = outcome.expect("smoke roundtrip");
        assert_eq!(id, revision_id);
        assert_eq!(hash, revision.manifest_hash);
    }
}
