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

use super::catalog::{ActivationPlan, ActivationKind, CatalogError, RevisionStore};
use super::model::{
    AuthorityClass, ComponentKind, HarnessDomain, HarnessManifest, HarnessRevision,
    RevisionStatus,
};

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
            CatalogError::Store(format!("no runtime for the durable revision store: {error}"))
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

    fn current_activation(
        &self,
        domain: HarnessDomain,
    ) -> Result<Option<String>, CatalogError> {
        self.durable_current(domain)
    }

    fn last_activation_sequence(
        &self,
        domain: HarnessDomain,
    ) -> Result<u64, CatalogError> {
        let sequence = self.block_on(self.dao.last_activation_sequence(domain.id()))?;
        Ok(u64::try_from(sequence).unwrap_or(0))
    }
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
    let manifest: HarnessManifest = serde_json::from_value(row.manifest.clone()).map_err(|error| {
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
            CatalogError::Store(format!("component row names unknown kind {}", component.kind))
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
    ComponentKind::ALL
        .into_iter()
        .find(|kind| kind.id() == id)
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
                sqlx::query_as::<_, (Option<String>,)>(
                    "select to_regclass($1)::text",
                )
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
            let read = store
                .get(&revision_id)
                .ok_or_else(|| "inserted revision missing from the write-through view".to_string())?;
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
