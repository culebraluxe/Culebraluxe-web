//! Durable Harness Evolution persistence (migration 294).
//!
//! This is the database boundary for the native Harness Evolution subsystem. It owns SQL; it does
//! not own policy or lifecycle decisions. Two properties are enforced here, not at call sites:
//!
//! * **Revisions are verified on the way in and out.** A revision row carries a `manifest_hash`;
//!   [`manifest_hash`] recomputes the model's canonical SHA-256 from the stored `manifest` jsonb and
//!   [`ForgeHarnessDao::insert_revision`] refuses a row whose hash or component projection does not
//!   match. [`ForgeHarnessDao::get_revision`] recomputes the same hash on read, so a hand-edited row
//!   is a schema mismatch rather than a silently trusted harness.
//! * **The active pointer moves in one transaction.** [`ForgeHarnessDao::activate`] (and its
//!   [`ForgeHarnessDao::rollback`] sibling) locks the domain's current activation row, verifies the
//!   caller's expected pointer, flips `is_current`, writes the two lifecycle statuses and inserts the
//!   new activation row inside one database transaction. Any failure rolls all of it back: the
//!   partial unique index `forge_harness_activation_one_current` can never see two current rows.
//!
//! There is no telemetry copy here. Probe/contrast rows carry evidence **ids** (`evidence_refs`) and
//! optional foreign keys to the execution/run rows that already hold the reading; measurement
//! columns are deliberately absent.
//!
//! The hash algorithm is a deliberate duplicate of `forge`'s `model.rs::HarnessManifest::
//! manifest_hash` (the `db` crate cannot depend on `forge`): same domain separation, same component
//! order, same length-prefixed field encoding. `forge`'s `store_db` module tests the parity of the
//! two implementations so the duplication cannot drift silently.

use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::FromRow;

/// Domain-separation prefix mixed into every manifest hash. MUST equal
/// `forge::engine::harness_evolution::model::MANIFEST_HASH_DOMAIN`.
pub const MANIFEST_HASH_DOMAIN: &str = "forge.harness_evolution.manifest.v1";

/// The eight component kinds in the model's canonical (hash) order.
pub const COMPONENT_KINDS: [&str; 8] = [
    "system_prompt",
    "guardrails",
    "memory",
    "tool_guidance",
    "subagents",
    "skills",
    "scripts",
    "plugins",
];

/// The lifecycle statuses the schema accepts (migration 294 CHECK).
pub const REVISION_STATUSES: [&str; 13] = [
    "draft",
    "proposed",
    "probe_ready",
    "probing",
    "compared",
    "composed",
    "validating",
    "adoptable",
    "adopted",
    "rejected",
    "failed",
    "superseded",
    "rolled_back",
];

/// The three authority classes.
pub const COMPONENT_AUTHORITIES: [&str; 3] = ["green", "yellow", "red"];

const REVISION_COLUMNS: &str = "revision_id, parent_revision_id, domain, status, manifest, \
                                manifest_hash, created_at, created_by";
const ACTIVATION_COLUMNS: &str = "activation_id, domain, sequence, activation_kind, \
                                  from_revision_id, to_revision_id, decision_id, activated_by, \
                                  activated_at, is_current";

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    push_bytes(out, value.as_bytes());
}

/// One component as it appears in the stored `manifest` jsonb. Field names match
/// `model.rs::Component`'s serde representation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestComponent {
    kind: String,
    authority: String,
    content: String,
    description: String,
}

/// Parse the canonical manifest wire shape (`{"components":[{kind,authority,content,description}]}`).
///
/// Refuses anything that is not exactly that shape: an unknown/missing kind, a duplicate kind, or a
/// wrong field type is a schema mismatch, because a hash over an ambiguous manifest would be a
/// different hash from the one the model computed.
fn parse_manifest(manifest: &Value) -> DbResult<Vec<ManifestComponent>> {
    let components = manifest
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "forge_harness.manifest",
                "manifest is not {\"components\":[...]}",
            )
        })?;
    let mut parsed: Vec<ManifestComponent> = Vec::with_capacity(components.len());
    for component in components {
        let kind = component
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DbFailure::schema_mismatch("forge_harness.manifest.kind", "component has no kind")
            })?;
        if !COMPONENT_KINDS.contains(&kind) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.manifest.kind",
                format!("unknown component kind {kind}"),
            ));
        }
        let authority = component
            .get("authority")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "forge_harness.manifest.authority",
                    format!("component {kind} has no authority"),
                )
            })?;
        if !COMPONENT_AUTHORITIES.contains(&authority) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.manifest.authority",
                format!("component {kind} has unknown authority {authority}"),
            ));
        }
        let content = component
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "forge_harness.manifest.content",
                    format!("component {kind} has no content"),
                )
            })?;
        let description = component
            .get("description")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "forge_harness.manifest.description",
                    format!("component {kind} has no description"),
                )
            })?;
        if parsed.iter().any(|existing| existing.kind == kind) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.manifest.kind",
                format!("duplicate component kind {kind}"),
            ));
        }
        parsed.push(ManifestComponent {
            kind: kind.to_string(),
            authority: authority.to_string(),
            content: content.to_string(),
            description: description.to_string(),
        });
    }
    Ok(parsed)
}

/// The canonical manifest hash, recomputed from the stored jsonb.
///
/// Byte-for-byte the model's algorithm: domain prefix, component count, then each component (in the
/// canonical kind order) as length-prefixed kind/authority/content/description.
pub fn manifest_hash(manifest: &Value) -> DbResult<String> {
    let components = parse_manifest(manifest)?;
    let mut out = Vec::new();
    out.extend_from_slice(MANIFEST_HASH_DOMAIN.as_bytes());
    out.extend_from_slice(&(components.len() as u64).to_le_bytes());
    for kind in COMPONENT_KINDS {
        let Some(component) = components.iter().find(|component| component.kind == kind) else {
            continue;
        };
        push_str(&mut out, &component.kind);
        push_str(&mut out, &component.authority);
        push_str(&mut out, &component.content);
        push_str(&mut out, &component.description);
    }
    Ok(sha256_hex(&out))
}

/// Mirrors `model.rs::RevisionStatus::can_transition_to`. The model remains the source of truth;
/// the DAO keeps a copy so a writer that bypasses the model cannot walk a revision out of its
/// lifecycle (e.g. `rejected -> adopted`).
pub fn revision_status_transition_allowed(from: &str, to: &str) -> bool {
    match from {
        "draft" => matches!(to, "proposed" | "rejected" | "failed"),
        "proposed" => matches!(to, "probe_ready" | "rejected" | "failed"),
        "probe_ready" => matches!(to, "probing" | "rejected" | "failed"),
        "probing" => matches!(to, "compared" | "failed"),
        "compared" => matches!(to, "composed" | "rejected" | "failed"),
        "composed" => matches!(to, "validating" | "rejected" | "failed"),
        "validating" => matches!(to, "adoptable" | "rejected" | "failed"),
        "adoptable" => matches!(to, "adopted" | "rejected" | "failed"),
        "adopted" => matches!(to, "superseded" | "rolled_back"),
        "superseded" => matches!(to, "adopted" | "rolled_back"),
        "rejected" | "failed" | "rolled_back" => false,
        _ => false,
    }
}

/// Which `(target, displaced)` lifecycle pair each activation kind may write.
///
/// This ties a caller-supplied `activation_kind` to the real statuses, so a request cannot say
/// "rollback" while adopting an arbitrary revision: a rollback restores a revision that an earlier
/// adoption displaced (`superseded -> adopted`), and its displaced revision must be `adopted`.
pub fn activation_pair_allowed(
    kind: &str,
    target_status: &str,
    displaced_status: Option<&str>,
) -> bool {
    match kind {
        "adopt" => {
            matches!(target_status, "adoptable" | "adopted" | "superseded")
                && displaced_status.is_none_or(|status| status == "adopted")
        }
        "rollback" => {
            target_status == "superseded" && displaced_status == Some("adopted")
        }
        _ => false,
    }
}

/// A revision row as stored.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessRevisionRow {
    pub revision_id: String,
    pub parent_revision_id: Option<String>,
    pub domain: String,
    pub status: String,
    pub manifest: Value,
    pub manifest_hash: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

/// One component row of a revision (the readable projection beside the hash-authoritative jsonb).
#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessComponentRow {
    pub revision_id: String,
    pub kind: String,
    pub authority: String,
    pub content: String,
    pub description: String,
}

/// A revision with its component projection.
#[derive(Debug, Clone)]
pub struct StoredForgeHarnessRevision {
    pub revision: ForgeHarnessRevisionRow,
    pub components: Vec<ForgeHarnessComponentRow>,
}

/// A new revision to persist. The DAO verifies hash, components and parent domain before writing.
#[derive(Debug, Clone)]
pub struct NewForgeHarnessRevision {
    pub revision_id: String,
    pub parent_revision_id: Option<String>,
    pub domain: String,
    pub status: String,
    pub manifest: Value,
    pub manifest_hash: String,
    pub created_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub components: Vec<NewForgeHarnessComponent>,
}

#[derive(Debug, Clone)]
pub struct NewForgeHarnessComponent {
    pub kind: String,
    pub authority: String,
    pub content: String,
    pub description: String,
}

/// A new experiment row.
#[derive(Debug, Clone)]
pub struct NewForgeHarnessExperiment {
    pub experiment_id: String,
    pub domain: String,
    pub base_revision_id: String,
    pub base_manifest_hash: String,
    pub mode: String,
    pub k: i32,
    pub max_steps: i32,
    pub max_probes: i32,
    pub seed: i64,
    pub state: String,
    pub successor_revision_id: Option<String>,
    pub validation: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessExperimentRow {
    pub experiment_id: String,
    pub domain: String,
    pub base_revision_id: String,
    pub base_manifest_hash: String,
    pub mode: String,
    pub k: i32,
    pub max_steps: i32,
    pub max_probes: i32,
    pub seed: i64,
    pub state: String,
    pub failure_stage: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub successor_revision_id: Option<String>,
    pub validation: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A new candidate row (invariant or domain candidate, expressed as changes jsonb).
#[derive(Debug, Clone)]
pub struct NewForgeHarnessCandidate {
    pub candidate_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub proposal_kind: String,
    pub changes: Value,
    pub candidate_revision_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessCandidateRow {
    pub candidate_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub proposal_kind: String,
    pub changes: Value,
    pub candidate_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A new probe row. Evidence is referenced, never copied: `evidence_refs` holds durable Forge
/// evidence ids and the optional FKs point at the execution/run rows that produced them.
#[derive(Debug, Clone)]
pub struct NewForgeHarnessProbe {
    pub probe_id: String,
    pub experiment_id: String,
    pub candidate_revision_id: Option<String>,
    pub side: String,
    pub revision_id: String,
    pub contract_digest: String,
    pub spec: Value,
    pub evidence_id: Option<String>,
    pub evidence_refs: Value,
    pub process_instance_id: Option<String>,
    pub story_run_id: Option<String>,
    pub engine_task_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessProbeRow {
    pub probe_id: String,
    pub experiment_id: String,
    pub candidate_revision_id: Option<String>,
    pub side: String,
    pub revision_id: String,
    pub contract_digest: String,
    pub spec: Value,
    pub evidence_id: Option<String>,
    pub evidence_refs: Value,
    pub process_instance_id: Option<String>,
    pub story_run_id: Option<String>,
    pub engine_task_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewForgeHarnessContrast {
    pub contrast_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub candidate_revision_id: String,
    pub reference_probe_id: Option<String>,
    pub candidate_probe_id: Option<String>,
    pub report: Value,
    pub evidence_refs: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessContrastRow {
    pub contrast_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub candidate_revision_id: String,
    pub reference_probe_id: Option<String>,
    pub candidate_probe_id: Option<String>,
    pub report: Value,
    pub evidence_refs: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewForgeHarnessComposition {
    pub composition_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub successor_revision_id: Option<String>,
    pub successor_manifest_hash: String,
    pub decision: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessCompositionRow {
    pub composition_id: String,
    pub experiment_id: String,
    pub base_revision_id: String,
    pub successor_revision_id: Option<String>,
    pub successor_manifest_hash: String,
    pub decision: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewForgeHarnessExperience {
    pub experience_id: String,
    pub domain: String,
    pub executor: String,
    pub mode: String,
    pub k: i32,
    pub max_steps: i32,
    pub max_probes: i32,
    pub seed: i64,
    pub harness_revision_id: Option<String>,
    pub manifest_hash: String,
    pub lesson: String,
    pub applicability: Value,
    pub counterevidence: Value,
    pub confidence: i32,
    pub contrast_id: String,
    pub validation: Value,
    pub evidence_refs: Value,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessExperienceRow {
    pub experience_id: String,
    pub domain: String,
    pub executor: String,
    pub mode: String,
    pub k: i32,
    pub max_steps: i32,
    pub max_probes: i32,
    pub seed: i64,
    pub harness_revision_id: Option<String>,
    pub manifest_hash: String,
    pub lesson: String,
    pub applicability: Value,
    pub counterevidence: Value,
    pub confidence: i32,
    pub contrast_id: String,
    pub validation: Value,
    pub evidence_refs: Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// One activation (adoption or rollback) pointer move.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeHarnessActivationRow {
    pub activation_id: i64,
    pub domain: String,
    pub sequence: i64,
    pub activation_kind: String,
    pub from_revision_id: Option<String>,
    pub to_revision_id: String,
    pub decision_id: String,
    pub activated_by: String,
    pub activated_at: DateTime<Utc>,
    pub is_current: bool,
}

/// What the caller wants the pointer moved to. `expected_from_revision` is an optimistic-concurrency
/// check: the transaction refuses to move a pointer that is no longer where the caller thought.
#[derive(Debug, Clone)]
pub struct ActivationRequest {
    pub domain: String,
    pub sequence: i64,
    pub activation_kind: String,
    pub revision_id: String,
    pub expected_from_revision: Option<String>,
    pub decision_id: String,
    pub activated_by: String,
}

/// DAO over the nine `forge_harness_*` tables (migration 294).
#[derive(Clone)]
pub struct ForgeHarnessDao {
    db: Database,
}

impl ForgeHarnessDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    // ---------------------------------------------------------------------------------------------
    // Revisions
    // ---------------------------------------------------------------------------------------------

    /// Persist one immutable revision plus its component projection in one transaction.
    ///
    /// Refuses before writing when the supplied hash does not equal the recomputed hash, when the
    /// component rows do not describe exactly the manifest, or when the parent revision belongs to
    /// another domain. The parent FK (on delete restrict) keeps lineage complete.
    pub async fn insert_revision(
        &self,
        new: &NewForgeHarnessRevision,
    ) -> DbResult<ForgeHarnessRevisionRow> {
        let computed = manifest_hash(&new.manifest)?;
        if computed != new.manifest_hash {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.hash",
                format!(
                    "revision {} supplied hash {} but its manifest hashes to {}",
                    new.revision_id, new.manifest_hash, computed
                ),
            ));
        }
        let manifest_components = parse_manifest(&new.manifest)?;
        verify_components_match_manifest(&new.revision_id, &manifest_components, &new.components)?;
        if new.revision_id.trim().is_empty() || new.domain.trim().is_empty() {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.identity",
                "revision id and domain must be non-empty",
            ));
        }
        if !REVISION_STATUSES.contains(&new.status.as_str()) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.status",
                format!("unknown revision status {}", new.status),
            ));
        }
        if let Some(parent_id) = new.parent_revision_id.as_deref() {
            let parent_domain: Option<(String,)> =
                sqlx::query_as("select domain from forge_harness_revision where revision_id=$1")
                    .bind(parent_id)
                    .fetch_optional(self.db.pool())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("forge_harness.insert_revision.parent", &error)
                    })?;
            let Some((parent_domain,)) = parent_domain else {
                return Err(DbFailure::schema_mismatch(
                    "forge_harness.insert_revision.parent",
                    format!(
                        "revision {} names parent {parent_id}, which does not exist",
                        new.revision_id
                    ),
                ));
            };
            if parent_domain != new.domain {
                return Err(DbFailure::schema_mismatch(
                    "forge_harness.insert_revision.parent_domain",
                    format!(
                        "revision {} is {}, but parent {parent_id} is {parent_domain}",
                        new.revision_id, new.domain
                    ),
                ));
            }
        }

        let mut tx = self.db.pool().begin().await.map_err(|error| {
            DbFailure::from_sqlx("forge_harness.insert_revision.begin", &error)
        })?;
        let row = sqlx::query_as::<_, ForgeHarnessRevisionRow>(sqlx::AssertSqlSafe(format!(
            "insert into forge_harness_revision
                 (revision_id, parent_revision_id, domain, status, manifest, manifest_hash,
                  created_at, created_by)
             values ($1, $2, $3, $4, $5, $6, coalesce($7, now()), $8)
             returning {REVISION_COLUMNS}"
        )))
        .bind(&new.revision_id)
        .bind(&new.parent_revision_id)
        .bind(&new.domain)
        .bind(&new.status)
        .bind(&new.manifest)
        .bind(&new.manifest_hash)
        .bind(new.created_at)
        .bind(&new.created_by)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_harness.insert_revision.row", &error)
        })?;

        for component in &new.components {
            sqlx::query(
                "insert into forge_harness_component
                     (revision_id, kind, authority, content, description)
                 values ($1, $2, $3, $4, $5)",
            )
            .bind(&new.revision_id)
            .bind(&component.kind)
            .bind(&component.authority)
            .bind(&component.content)
            .bind(&component.description)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_harness.insert_revision.component", &error)
            })?;
        }

        tx.commit().await.map_err(|error| {
            DbFailure::from_sqlx("forge_harness.insert_revision.commit", &error)
        })?;
        Ok(row)
    }

    /// Read one revision with its components, recomputing the manifest hash from the stored jsonb.
    /// A row that no longer hashes to its stored `manifest_hash` is a schema mismatch.
    pub async fn get_revision(
        &self,
        revision_id: &str,
    ) -> DbResult<Option<StoredForgeHarnessRevision>> {
        let revision = sqlx::query_as::<_, ForgeHarnessRevisionRow>(sqlx::AssertSqlSafe(format!(
            "select {REVISION_COLUMNS} from forge_harness_revision where revision_id=$1"
        )))
        .bind(revision_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.get_revision", &error))?;
        let Some(revision) = revision else {
            return Ok(None);
        };
        let components = sqlx::query_as::<_, ForgeHarnessComponentRow>(
            "select revision_id, kind, authority, content, description
             from forge_harness_component where revision_id=$1 order by kind",
        )
        .bind(revision_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.get_revision.components", &error))?;
        verify_stored_revision(&revision, &components)?;
        Ok(Some(StoredForgeHarnessRevision {
            revision,
            components,
        }))
    }

    /// List one domain's revisions, **verified**: every row's `manifest_hash` is recomputed from
    /// the stored jsonb and its component projection is compared with the manifest in one batched
    /// component query, so a listing is evidence rather than a read of unverified rows.
    pub async fn list_revisions(&self, domain: &str) -> DbResult<Vec<ForgeHarnessRevisionRow>> {
        let rows = sqlx::query_as::<_, ForgeHarnessRevisionRow>(sqlx::AssertSqlSafe(format!(
            "select {REVISION_COLUMNS} from forge_harness_revision
             where domain=$1 order by created_at, revision_id"
        )))
        .bind(domain)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_revisions", &error))?;
        self.verify_rows(&rows).await?;
        Ok(rows)
    }

    /// List every revision, verified the same way as [`ForgeHarnessDao::list_revisions`].
    pub async fn list_all_revisions(&self) -> DbResult<Vec<ForgeHarnessRevisionRow>> {
        let rows = sqlx::query_as::<_, ForgeHarnessRevisionRow>(sqlx::AssertSqlSafe(format!(
            "select {REVISION_COLUMNS} from forge_harness_revision order by created_at, revision_id"
        )))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_all_revisions", &error))?;
        self.verify_rows(&rows).await?;
        Ok(rows)
    }

    /// Verify a batch of revision rows without an N+1 read: one query fetches every component row
    /// for the batch, then each revision's hash and projection are checked against its manifest.
    async fn verify_rows(&self, rows: &[ForgeHarnessRevisionRow]) -> DbResult<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = rows
            .iter()
            .map(|row| row.revision_id.clone())
            .collect();
        let components = sqlx::query_as::<_, ForgeHarnessComponentRow>(
            "select revision_id, kind, authority, content, description
             from forge_harness_component where revision_id = any($1) order by revision_id, kind",
        )
        .bind(&ids)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.verify_rows.components", &error))?;
        for row in rows {
            let row_components: Vec<ForgeHarnessComponentRow> = components
                .iter()
                .filter(|component| component.revision_id == row.revision_id)
                .cloned()
                .collect();
            verify_stored_revision(row, &row_components)?;
        }
        Ok(())
    }

    /// Write a lifecycle status, refusing an illegal transition.
    ///
    /// The model's transition table is mirrored by [`revision_status_transition_allowed`]; the row
    /// is locked `for update` inside a transaction so a concurrent writer cannot interleave a
    /// status between the read and the write. The pointer itself is never touched here: activation
    /// is [`ForgeHarnessDao::activate`].
    pub async fn set_revision_status(&self, revision_id: &str, status: &str) -> DbResult<()> {
        if !REVISION_STATUSES.contains(&status) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.set_revision_status",
                format!("unknown revision status {status}"),
            ));
        }
        let mut tx = self
            .db
            .pool()
            .begin()
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_harness.set_revision_status.begin", &error))?;
        let current: Option<(String,)> = sqlx::query_as(
            "select status from forge_harness_revision where revision_id=$1 for update",
        )
        .bind(revision_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_harness.set_revision_status.lock", &error)
        })?;
        let Some((current,)) = current else {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.set_revision_status.missing",
                format!("revision {revision_id} does not exist"),
            ));
        };
        if !revision_status_transition_allowed(&current, status) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.set_revision_status.transition",
                format!("revision {revision_id} may not move from {current} to {status}"),
            ));
        }
        sqlx::query("update forge_harness_revision set status=$2 where revision_id=$1")
            .bind(revision_id)
            .bind(status)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_harness.set_revision_status.update", &error)
            })?;
        tx.commit().await.map_err(|error| {
            DbFailure::from_sqlx("forge_harness.set_revision_status.commit", &error)
        })?;
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------
    // Activation pointer (transactional)
    // ---------------------------------------------------------------------------------------------

    pub async fn current_activation(
        &self,
        domain: &str,
    ) -> DbResult<Option<ForgeHarnessActivationRow>> {
        sqlx::query_as::<_, ForgeHarnessActivationRow>(sqlx::AssertSqlSafe(format!(
            "select {ACTIVATION_COLUMNS} from forge_harness_activation
             where domain=$1 and is_current"
        )))
        .bind(domain)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.current_activation", &error))
    }

    /// The domain's highest activation sequence (0 when the pointer has never moved). A durable
    /// catalog rehydrates its sequence counter from this so the next activation still advances.
    pub async fn last_activation_sequence(&self, domain: &str) -> DbResult<i64> {
        let (sequence,): (i64,) = sqlx::query_as(
            "select coalesce(max(sequence), 0) from forge_harness_activation where domain=$1",
        )
        .bind(domain)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.last_activation_sequence", &error))?;
        Ok(sequence)
    }

    pub async fn activation_history(
        &self,
        domain: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeHarnessActivationRow>> {
        sqlx::query_as::<_, ForgeHarnessActivationRow>(sqlx::AssertSqlSafe(format!(
            "select {ACTIVATION_COLUMNS} from forge_harness_activation
             where domain=$1 order by sequence desc limit $2"
        )))
        .bind(domain)
        .bind(limit.clamp(1, 500))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activation_history", &error))
    }

    /// Move the active pointer to `request.revision_id` and write both lifecycle statuses in one
    /// transaction.
    ///
    /// The prior current row is locked `for update`; `expected_from_revision` must still name it.
    /// Then: prior `is_current=false`, prior revision `superseded`, target revision `adopted`, new
    /// activation row `is_current=true`. Any error (missing/foreign revision, tampered hash, stale
    /// expected pointer, the one-current unique index) aborts the transaction, so the pointer is
    /// unchanged and no status was written — the D4 contract.
    pub async fn activate(
        &self,
        request: &ActivationRequest,
    ) -> DbResult<ForgeHarnessActivationRow> {
        self.apply_pointer(request, "adopted", "superseded").await
    }

    /// Roll the pointer back to `request.revision_id` (the recorded prior revision) in one
    /// transaction: displaced revision `rolled_back`, restored revision `adopted`.
    pub async fn rollback(
        &self,
        request: &ActivationRequest,
    ) -> DbResult<ForgeHarnessActivationRow> {
        self.apply_pointer(request, "adopted", "rolled_back").await
    }

    async fn apply_pointer(
        &self,
        request: &ActivationRequest,
        target_status: &'static str,
        displaced_status: &'static str,
    ) -> DbResult<ForgeHarnessActivationRow> {
        if !matches!(request.activation_kind.as_str(), "adopt" | "rollback") {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.kind",
                format!("unknown activation kind {}", request.activation_kind),
            ));
        }
        let mut tx = self
            .db
            .pool()
            .begin()
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.begin", &error))?;

        // 1. Lock the domain's current pointer, if any.
        let current = sqlx::query_as::<_, ForgeHarnessActivationRow>(sqlx::AssertSqlSafe(format!(
            "select {ACTIVATION_COLUMNS} from forge_harness_activation
             where domain=$1 and is_current for update"
        )))
        .bind(&request.domain)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.lock", &error))?;
        let current_target = current
            .as_ref()
            .map(|activation| activation.to_revision_id.clone());
        if current_target != request.expected_from_revision {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.stale_pointer",
                format!(
                    "expected active revision {:?} for {}, but the pointer is {:?}",
                    request.expected_from_revision, request.domain, current_target
                ),
            ));
        }

        // 2. Verify the target revision: exists, same domain, legal status, intact manifest hash.
        let target = sqlx::query_as::<_, (String, String, Value, String)>(
            "select revision_id, domain, manifest, manifest_hash from forge_harness_revision
             where revision_id=$1 for update",
        )
        .bind(&request.revision_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.target", &error))?
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "forge_harness.activate.target_missing",
                format!("revision {} does not exist", request.revision_id),
            )
        })?;
        let (target_id, target_domain, target_manifest, target_hash) = target;
        if target_domain != request.domain {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.target_domain",
                format!(
                    "revision {target_id} belongs to {target_domain}, not {}",
                    request.domain
                ),
            ));
        }
        let recomputed = manifest_hash(&target_manifest)?;
        if recomputed != target_hash {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.target_hash",
                format!("revision {target_id} manifest does not match its stored hash"),
            ));
        }
        if let Some(prior) = &current_target {
            if prior == &target_id {
                return Err(DbFailure::schema_mismatch(
                    "forge_harness.activate.already_active",
                    format!("revision {target_id} is already the active harness"),
                ));
            }
        }
        let target_lifecycle: (String,) = sqlx::query_as(
            "select status from forge_harness_revision where revision_id=$1 for update",
        )
        .bind(&target_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.target_status", &error))?;
        let displaced_lifecycle: Option<String> = match current_target.as_deref() {
            Some(prior) => sqlx::query_scalar(
                "select status from forge_harness_revision where revision_id=$1 for update",
            )
            .bind(prior)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_harness.activate.displaced_status_read", &error)
            })?,
            None => None,
        };
        // The kind is caller-supplied, so tie it to the statuses it claims to move: an adoption
        // may only displace an `adopted` revision, and a rollback may only restore a revision an
        // earlier adoption displaced (`superseded`) while displacing an `adopted` one.
        if !activation_pair_allowed(
            &request.activation_kind,
            &target_lifecycle.0,
            displaced_lifecycle.as_deref(),
        ) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.lifecycle_pair",
                format!(
                    "{} may not move {:?} to {target_id} ({}) with displaced {:?}",
                    request.activation_kind,
                    current_target,
                    target_lifecycle.0,
                    displaced_lifecycle
                ),
            ));
        }
        if request.activation_kind == "rollback" {
            let current = current.as_ref().ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "forge_harness.activate.rollback_without_pointer",
                    "a rollback requires an active revision to displace",
                )
            })?;
            // The restored revision must be exactly the revision the current activation
            // displaced, not an arbitrary revision the caller names.
            if current.from_revision_id.as_deref() != Some(target_id.as_str()) {
                return Err(DbFailure::schema_mismatch(
                    "forge_harness.activate.rollback_tie",
                    format!(
                        "rollback target {target_id} is not the revision activation {} displaced ({:?})",
                        current.activation_id, current.from_revision_id
                    ),
                ));
            }
        }
        let max_sequence: (i64,) = sqlx::query_as(
            "select coalesce(max(sequence), 0) from forge_harness_activation where domain=$1",
        )
        .bind(&request.domain)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.sequence", &error))?;
        if request.sequence <= max_sequence.0 {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.activate.sequence",
                format!(
                    "activation sequence {} must be greater than the domain's last ({})",
                    request.sequence, max_sequence.0
                ),
            ));
        }

        // 3. Flip the prior pointer off (if any).
        if let Some(activation) = &current {
            sqlx::query(
                "update forge_harness_activation set is_current=false where activation_id=$1",
            )
            .bind(activation.activation_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.flip", &error))?;
        }

        // 4. Lifecycle writes, still inside the one transaction.
        if let Some(prior) = &current_target {
            sqlx::query("update forge_harness_revision set status=$2 where revision_id=$1")
                .bind(prior)
                .bind(displaced_status)
                .execute(&mut *tx)
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("forge_harness.activate.displaced_status", &error)
                })?;
        }
        sqlx::query("update forge_harness_revision set status=$2 where revision_id=$1")
            .bind(&target_id)
            .bind(target_status)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_harness.activate.target_status_write", &error)
            })?;

        // 5. New current row. The partial unique index is the last line of defence.
        let row = sqlx::query_as::<_, ForgeHarnessActivationRow>(sqlx::AssertSqlSafe(format!(
            "insert into forge_harness_activation
                 (domain, sequence, activation_kind, from_revision_id, to_revision_id,
                  decision_id, activated_by, is_current)
             values ($1, $2, $3, $4, $5, $6, $7, true)
             returning {ACTIVATION_COLUMNS}"
        )))
        .bind(&request.domain)
        .bind(request.sequence)
        .bind(&request.activation_kind)
        .bind(&current_target)
        .bind(&target_id)
        .bind(&request.decision_id)
        .bind(&request.activated_by)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.insert", &error))?;

        tx.commit()
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_harness.activate.commit", &error))?;
        Ok(row)
    }

    // ---------------------------------------------------------------------------------------------
    // Experiments and lineage rows
    // ---------------------------------------------------------------------------------------------

    pub async fn insert_experiment(
        &self,
        new: &NewForgeHarnessExperiment,
    ) -> DbResult<ForgeHarnessExperimentRow> {
        sqlx::query_as::<_, ForgeHarnessExperimentRow>(
            "insert into forge_harness_experiment
                 (experiment_id, domain, base_revision_id, base_manifest_hash, mode, k,
                  max_steps, max_probes, seed, state, successor_revision_id, validation)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
             returning experiment_id, domain, base_revision_id, base_manifest_hash, mode, k,
                       max_steps, max_probes, seed, state, failure_stage, failure_code,
                       failure_message, successor_revision_id, validation, created_at, updated_at",
        )
        .bind(&new.experiment_id)
        .bind(&new.domain)
        .bind(&new.base_revision_id)
        .bind(&new.base_manifest_hash)
        .bind(&new.mode)
        .bind(new.k)
        .bind(new.max_steps)
        .bind(new.max_probes)
        .bind(new.seed)
        .bind(&new.state)
        .bind(&new.successor_revision_id)
        .bind(&new.validation)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_experiment", &error))
    }

    pub async fn get_experiment(
        &self,
        experiment_id: &str,
    ) -> DbResult<Option<ForgeHarnessExperimentRow>> {
        sqlx::query_as::<_, ForgeHarnessExperimentRow>(
            "select experiment_id, domain, base_revision_id, base_manifest_hash, mode, k,
                    max_steps, max_probes, seed, state, failure_stage, failure_code,
                    failure_message, successor_revision_id, validation, created_at, updated_at
             from forge_harness_experiment where experiment_id=$1",
        )
        .bind(experiment_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.get_experiment", &error))
    }

    pub async fn list_experiments(
        &self,
        domain: &str,
    ) -> DbResult<Vec<ForgeHarnessExperimentRow>> {
        sqlx::query_as::<_, ForgeHarnessExperimentRow>(
            "select experiment_id, domain, base_revision_id, base_manifest_hash, mode, k,
                    max_steps, max_probes, seed, state, failure_stage, failure_code,
                    failure_message, successor_revision_id, validation, created_at, updated_at
             from forge_harness_experiment where domain=$1 order by created_at desc",
        )
        .bind(domain)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_experiments", &error))
    }

    pub async fn insert_candidate(
        &self,
        new: &NewForgeHarnessCandidate,
    ) -> DbResult<ForgeHarnessCandidateRow> {
        sqlx::query_as::<_, ForgeHarnessCandidateRow>(
            "insert into forge_harness_candidate
                 (candidate_id, experiment_id, base_revision_id, proposal_kind, changes,
                  candidate_revision_id)
             values ($1,$2,$3,$4,$5,$6)
             returning candidate_id, experiment_id, base_revision_id, proposal_kind, changes,
                       candidate_revision_id, created_at",
        )
        .bind(&new.candidate_id)
        .bind(&new.experiment_id)
        .bind(&new.base_revision_id)
        .bind(&new.proposal_kind)
        .bind(&new.changes)
        .bind(&new.candidate_revision_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_candidate", &error))
    }

    pub async fn list_candidates(
        &self,
        experiment_id: &str,
    ) -> DbResult<Vec<ForgeHarnessCandidateRow>> {
        sqlx::query_as::<_, ForgeHarnessCandidateRow>(
            "select candidate_id, experiment_id, base_revision_id, proposal_kind, changes,
                    candidate_revision_id, created_at
             from forge_harness_candidate where experiment_id=$1 order by candidate_id",
        )
        .bind(experiment_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_candidates", &error))
    }

    pub async fn insert_probe(
        &self,
        new: &NewForgeHarnessProbe,
    ) -> DbResult<ForgeHarnessProbeRow> {
        sqlx::query_as::<_, ForgeHarnessProbeRow>(
            "insert into forge_harness_probe
                 (probe_id, experiment_id, candidate_revision_id, side, revision_id,
                  contract_digest, spec, evidence_id, evidence_refs, process_instance_id,
                  story_run_id, engine_task_id)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::text::uuid,$11::text::uuid,$12::text::uuid)
             returning probe_id, experiment_id, candidate_revision_id, side, revision_id,
                       contract_digest, spec, evidence_id, evidence_refs,
                       process_instance_id::text as process_instance_id,
                       story_run_id::text as story_run_id,
                       engine_task_id::text as engine_task_id, created_at",
        )
        .bind(&new.probe_id)
        .bind(&new.experiment_id)
        .bind(&new.candidate_revision_id)
        .bind(&new.side)
        .bind(&new.revision_id)
        .bind(&new.contract_digest)
        .bind(&new.spec)
        .bind(&new.evidence_id)
        .bind(&new.evidence_refs)
        .bind(new.process_instance_id.as_deref())
        .bind(new.story_run_id.as_deref())
        .bind(new.engine_task_id.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_probe", &error))
    }

    pub async fn list_probes(&self, experiment_id: &str) -> DbResult<Vec<ForgeHarnessProbeRow>> {
        sqlx::query_as::<_, ForgeHarnessProbeRow>(
            "select probe_id, experiment_id, candidate_revision_id, side, revision_id,
                    contract_digest, spec, evidence_id, evidence_refs,
                    process_instance_id::text as process_instance_id,
                    story_run_id::text as story_run_id,
                    engine_task_id::text as engine_task_id, created_at
             from forge_harness_probe where experiment_id=$1 order by created_at, probe_id",
        )
        .bind(experiment_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_probes", &error))
    }

    pub async fn insert_contrast(
        &self,
        new: &NewForgeHarnessContrast,
    ) -> DbResult<ForgeHarnessContrastRow> {
        sqlx::query_as::<_, ForgeHarnessContrastRow>(
            "insert into forge_harness_contrast
                 (contrast_id, experiment_id, base_revision_id, candidate_revision_id,
                  reference_probe_id, candidate_probe_id, report, evidence_refs)
             values ($1,$2,$3,$4,$5,$6,$7,$8)
             returning contrast_id, experiment_id, base_revision_id, candidate_revision_id,
                       reference_probe_id, candidate_probe_id, report, evidence_refs, created_at",
        )
        .bind(&new.contrast_id)
        .bind(&new.experiment_id)
        .bind(&new.base_revision_id)
        .bind(&new.candidate_revision_id)
        .bind(&new.reference_probe_id)
        .bind(&new.candidate_probe_id)
        .bind(&new.report)
        .bind(&new.evidence_refs)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_contrast", &error))
    }

    pub async fn list_contrasts(
        &self,
        experiment_id: &str,
    ) -> DbResult<Vec<ForgeHarnessContrastRow>> {
        sqlx::query_as::<_, ForgeHarnessContrastRow>(
            "select contrast_id, experiment_id, base_revision_id, candidate_revision_id,
                    reference_probe_id, candidate_probe_id, report, evidence_refs, created_at
             from forge_harness_contrast where experiment_id=$1 order by created_at, contrast_id",
        )
        .bind(experiment_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_contrasts", &error))
    }

    pub async fn insert_composition(
        &self,
        new: &NewForgeHarnessComposition,
    ) -> DbResult<ForgeHarnessCompositionRow> {
        sqlx::query_as::<_, ForgeHarnessCompositionRow>(
            "insert into forge_harness_composition
                 (composition_id, experiment_id, base_revision_id, successor_revision_id,
                  successor_manifest_hash, decision)
             values ($1,$2,$3,$4,$5,$6)
             returning composition_id, experiment_id, base_revision_id, successor_revision_id,
                       successor_manifest_hash, decision, created_at",
        )
        .bind(&new.composition_id)
        .bind(&new.experiment_id)
        .bind(&new.base_revision_id)
        .bind(&new.successor_revision_id)
        .bind(&new.successor_manifest_hash)
        .bind(&new.decision)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_composition", &error))
    }

    pub async fn get_composition(
        &self,
        experiment_id: &str,
    ) -> DbResult<Option<ForgeHarnessCompositionRow>> {
        sqlx::query_as::<_, ForgeHarnessCompositionRow>(
            "select composition_id, experiment_id, base_revision_id, successor_revision_id,
                    successor_manifest_hash, decision, created_at
             from forge_harness_composition where experiment_id=$1",
        )
        .bind(experiment_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.get_composition", &error))
    }

    pub async fn insert_experience(
        &self,
        new: &NewForgeHarnessExperience,
    ) -> DbResult<ForgeHarnessExperienceRow> {
        sqlx::query_as::<_, ForgeHarnessExperienceRow>(
            "insert into forge_harness_experience
                 (experience_id, domain, executor, mode, k, max_steps, max_probes, seed,
                  harness_revision_id, manifest_hash, lesson, applicability, counterevidence,
                  confidence, contrast_id, validation, evidence_refs, expires_at)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
             returning experience_id, domain, executor, mode, k, max_steps, max_probes, seed,
                       harness_revision_id, manifest_hash, lesson, applicability, counterevidence,
                       confidence, contrast_id, validation, evidence_refs, created_at, expires_at",
        )
        .bind(&new.experience_id)
        .bind(&new.domain)
        .bind(&new.executor)
        .bind(&new.mode)
        .bind(new.k)
        .bind(new.max_steps)
        .bind(new.max_probes)
        .bind(new.seed)
        .bind(&new.harness_revision_id)
        .bind(&new.manifest_hash)
        .bind(&new.lesson)
        .bind(&new.applicability)
        .bind(&new.counterevidence)
        .bind(new.confidence)
        .bind(&new.contrast_id)
        .bind(&new.validation)
        .bind(&new.evidence_refs)
        .bind(new.expires_at)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.insert_experience", &error))
    }

    /// Lessons are domain-scoped (domain + executor + mode + seed), matching `ExperienceScope`.
    pub async fn list_experiences(
        &self,
        domain: &str,
        executor: &str,
        mode: &str,
        seed: i64,
        limit: i64,
    ) -> DbResult<Vec<ForgeHarnessExperienceRow>> {
        sqlx::query_as::<_, ForgeHarnessExperienceRow>(
            "select experience_id, domain, executor, mode, k, max_steps, max_probes, seed,
                    harness_revision_id, manifest_hash, lesson, applicability, counterevidence,
                    confidence, contrast_id, validation, evidence_refs, created_at, expires_at
             from forge_harness_experience
             where domain=$1 and executor=$2 and mode=$3 and seed=$4
             order by created_at desc limit $5",
        )
        .bind(domain)
        .bind(executor)
        .bind(mode)
        .bind(seed)
        .bind(limit.clamp(1, 200))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_harness.list_experiences", &error))
    }
}

fn verify_components_match_manifest(
    revision_id: &str,
    manifest: &[ManifestComponent],
    components: &[NewForgeHarnessComponent],
) -> DbResult<()> {
    if manifest.len() != components.len() {
        return Err(DbFailure::schema_mismatch(
            "forge_harness.insert_revision.components",
            format!(
                "revision {revision_id} has {} manifest components but {} component rows",
                manifest.len(),
                components.len()
            ),
        ));
    }
    for component in components {
        if !COMPONENT_KINDS.contains(&component.kind.as_str()) {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.components",
                format!("unknown component kind {}", component.kind),
            ));
        }
        let Some(expected) = manifest
            .iter()
            .find(|expected| expected.kind == component.kind)
        else {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.components",
                format!(
                    "revision {revision_id} has a {} component row but no matching manifest entry",
                    component.kind
                ),
            ));
        };
        if expected.authority != component.authority
            || expected.content != component.content
            || expected.description != component.description
        {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.insert_revision.components",
                format!(
                    "revision {revision_id} component {} does not match the manifest",
                    component.kind
                ),
            ));
        }
    }
    Ok(())
}

/// Verify one stored revision: recompute the canonical hash from the jsonb and compare the
/// component projection with the manifest. Used by the single-row read and by batched listings.
fn verify_stored_revision(
    revision: &ForgeHarnessRevisionRow,
    components: &[ForgeHarnessComponentRow],
) -> DbResult<()> {
    let recomputed = manifest_hash(&revision.manifest)?;
    if recomputed != revision.manifest_hash {
        return Err(DbFailure::schema_mismatch(
            "forge_harness.verify.hash",
            format!(
                "revision {} stores hash {} but its manifest hashes to {recomputed}",
                revision.revision_id, revision.manifest_hash
            ),
        ));
    }
    let manifest_components = parse_manifest(&revision.manifest)?;
    if manifest_components.len() != components.len() {
        return Err(DbFailure::schema_mismatch(
            "forge_harness.verify.components",
            format!(
                "revision {} has {} manifest components but {} component rows",
                revision.revision_id,
                manifest_components.len(),
                components.len()
            ),
        ));
    }
    for expected in &manifest_components {
        let Some(stored) = components
            .iter()
            .find(|component| component.kind == expected.kind)
        else {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.verify.components",
                format!(
                    "revision {} manifest has {} but no component row does",
                    revision.revision_id, expected.kind
                ),
            ));
        };
        if stored.authority != expected.authority
            || stored.content != expected.content
            || stored.description != expected.description
        {
            return Err(DbFailure::schema_mismatch(
                "forge_harness.verify.components",
                format!(
                    "revision {} component {} does not match the manifest",
                    revision.revision_id, expected.kind
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(components: &[(&str, &str, &str, &str)]) -> Value {
        json!({
            "components": components.iter().map(|(kind, authority, content, description)| json!({
                "kind": kind,
                "authority": authority,
                "content": content,
                "description": description,
            })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn the_manifest_hash_is_order_independent_and_content_sensitive() {
        let one = manifest(&[
            ("system_prompt", "green", "base prompt", "the prompt"),
            ("memory", "green", "notes", "the memory"),
        ]);
        let reordered = manifest(&[
            ("memory", "green", "notes", "the memory"),
            ("system_prompt", "green", "base prompt", "the prompt"),
        ]);
        assert_eq!(
            manifest_hash(&one).unwrap(),
            manifest_hash(&reordered).unwrap(),
            "the hash sorts by canonical component kind order"
        );
        let changed = manifest(&[
            ("system_prompt", "green", "different prompt", "the prompt"),
            ("memory", "green", "notes", "the memory"),
        ]);
        assert_ne!(manifest_hash(&one).unwrap(), manifest_hash(&changed).unwrap());
        let different_authority = manifest(&[
            ("system_prompt", "yellow", "base prompt", "the prompt"),
            ("memory", "green", "notes", "the memory"),
        ]);
        assert_ne!(
            manifest_hash(&one).unwrap(),
            manifest_hash(&different_authority).unwrap()
        );
    }

    #[test]
    fn an_ambiguous_manifest_is_a_schema_mismatch_not_a_hash() {
        assert!(manifest_hash(&json!({})).is_err());
        assert!(manifest_hash(&json!({"components": "nope"})).is_err());
        assert!(manifest_hash(&json!({"components": [{"kind": "unknown", "authority": "green",
            "content": "x", "description": "y"}]}))
        .is_err());
        assert!(manifest_hash(&json!({"components": [
            {"kind": "memory", "authority": "green", "content": "a", "description": "b"},
            {"kind": "memory", "authority": "green", "content": "c", "description": "d"}
        ]}))
        .is_err());
    }

    #[test]
    fn the_hash_matches_the_documented_encoding_shape() {
        // An empty manifest still hashes the domain prefix plus a zero component count.
        let empty = manifest(&[]);
        let mut expected = Vec::new();
        expected.extend_from_slice(MANIFEST_HASH_DOMAIN.as_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(manifest_hash(&empty).unwrap(), sha256_hex(&expected));
    }

    #[test]
    fn the_dao_transition_table_mirrors_the_model_lifecycle() {
        // Forward path is allowed.
        assert!(revision_status_transition_allowed("draft", "proposed"));
        assert!(revision_status_transition_allowed("adoptable", "adopted"));
        assert!(revision_status_transition_allowed("adopted", "superseded"));
        assert!(revision_status_transition_allowed("superseded", "adopted"));
        assert!(revision_status_transition_allowed("adopted", "rolled_back"));
        // Terminal statuses cannot be walked out of, and aborted steps cannot skip the pipeline.
        assert!(!revision_status_transition_allowed("rejected", "adopted"));
        assert!(!revision_status_transition_allowed("failed", "adopted"));
        assert!(!revision_status_transition_allowed("rolled_back", "adopted"));
        assert!(!revision_status_transition_allowed("draft", "adopted"));
        assert!(!revision_status_transition_allowed("probing", "composed"));
        assert!(!revision_status_transition_allowed("unknown", "adopted"));
    }

    #[test]
    fn an_activation_kind_is_tied_to_its_lifecycle_pair() {
        // Adoption: an adoptable/superseded/imported revision displaces an adopted one.
        assert!(activation_pair_allowed("adopt", "adoptable", Some("adopted")));
        assert!(activation_pair_allowed("adopt", "adopted", None));
        assert!(activation_pair_allowed("adopt", "superseded", Some("adopted")));
        assert!(!activation_pair_allowed("adopt", "draft", None));
        assert!(!activation_pair_allowed("adopt", "adoptable", Some("superseded")));
        // Rollback restores exactly a revision an adoption displaced, and displaces the adopter.
        assert!(activation_pair_allowed("rollback", "superseded", Some("adopted")));
        assert!(!activation_pair_allowed("rollback", "adoptable", Some("adopted")));
        assert!(!activation_pair_allowed("rollback", "adopted", None));
        assert!(!activation_pair_allowed("rollback", "rejected", Some("adopted")));
        // A made-up kind is never a pointer move.
        assert!(!activation_pair_allowed("switch", "adoption", None));
    }

    /// DEV smoke test for the experiment/probe lineage rows: proves the DAO insert/read SQL matches
    /// migration 294 (including the experiment's text/jsonb params and the probe's uuid FKs), then
    /// removes every row it created. Ignored by default; run with DATABASE_URL_DEV present:
    ///
    /// ```sh
    /// set -a; . /Users/Shared/dev/.env.local; set +a
    /// cargo test -p db --lib forge_harness -- --ignored
    /// ```
    #[test]
    #[ignore = "needs DATABASE_URL_DEV; keeps the unit suite database-free"]
    fn dev_smoke_experiment_and_probe_roundtrip_and_cleanup() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let database = runtime
            .block_on(Database::connect_target(crate::DbTarget::Dev))
            .expect("connect DEV");
        let dao = ForgeHarnessDao::new(database.clone());
        let suffix = uuid::Uuid::new_v4();
        let revision_id = format!("smoke-rev-{suffix}");
        let experiment_id = format!("smoke-exp-{suffix}");
        let probe_id = format!("smoke-probe-{suffix}");

        let manifest = json!({"components": [
            {"kind": "system_prompt", "authority": "green", "content": "smoke", "description": "smoke"}
        ]});
        let manifest_hash = manifest_hash(&manifest).expect("hash");

        let outcome: Result<(), String> = (|| {
            runtime
                .block_on(dao.insert_revision(&NewForgeHarnessRevision {
                    revision_id: revision_id.clone(),
                    parent_revision_id: None,
                    domain: "rust-backend".into(),
                    status: "adopted".into(),
                    manifest: manifest.clone(),
                    manifest_hash: manifest_hash.clone(),
                    created_at: None,
                    created_by: "smoke".into(),
                    components: vec![NewForgeHarnessComponent {
                        kind: "system_prompt".into(),
                        authority: "green".into(),
                        content: "smoke".into(),
                        description: "smoke".into(),
                    }],
                }))
                .map_err(|error| format!("insert revision: {error}"))?;
            runtime
                .block_on(dao.insert_experiment(&NewForgeHarnessExperiment {
                    experiment_id: experiment_id.clone(),
                    domain: "rust-backend".into(),
                    base_revision_id: revision_id.clone(),
                    base_manifest_hash: manifest_hash.clone(),
                    mode: "parallel".into(),
                    k: 1,
                    max_steps: 1,
                    max_probes: 3,
                    seed: 0,
                    state: "draft".into(),
                    successor_revision_id: None,
                    validation: None,
                }))
                .map_err(|error| format!("insert experiment: {error}"))?;
            let experiment = runtime
                .block_on(dao.get_experiment(&experiment_id))
                .map_err(|error| format!("read experiment: {error}"))?
                .ok_or_else(|| "experiment missing after insert".to_string())?;
            if experiment.experiment_id != experiment_id || experiment.base_revision_id != revision_id
            {
                return Err("experiment identity did not round-trip".into());
            }
            runtime
                .block_on(dao.insert_probe(&NewForgeHarnessProbe {
                    probe_id: probe_id.clone(),
                    experiment_id: experiment_id.clone(),
                    candidate_revision_id: None,
                    side: "reference".into(),
                    revision_id: revision_id.clone(),
                    contract_digest: "0".repeat(64),
                    spec: json!({"task": "smoke"}),
                    evidence_id: None,
                    evidence_refs: json!([]),
                    process_instance_id: None,
                    story_run_id: None,
                    engine_task_id: None,
                }))
                .map_err(|error| format!("insert probe: {error}"))?;
            let probes = runtime
                .block_on(dao.list_probes(&experiment_id))
                .map_err(|error| format!("read probes: {error}"))?;
            if probes.len() != 1 || probes[0].probe_id != probe_id {
                return Err("probe did not round-trip".into());
            }
            Ok(())
        })();

        let cleanup = runtime.block_on(async {
            for sql in [
                "delete from forge_harness_probe where probe_id=$1",
                "delete from forge_harness_experiment where experiment_id=$1",
            ] {
                sqlx::query(sql)
                    .bind(if sql.contains("probe") {
                        &probe_id
                    } else {
                        &experiment_id
                    })
                    .execute(database.pool())
                    .await?;
            }
            sqlx::query("delete from forge_harness_component where revision_id=$1")
                .bind(&revision_id)
                .execute(database.pool())
                .await?;
            sqlx::query("delete from forge_harness_revision where revision_id=$1")
                .bind(&revision_id)
                .execute(database.pool())
                .await
        });
        assert!(cleanup.is_ok(), "smoke cleanup must remove its rows");
        outcome.expect("experiment + probe smoke roundtrip");
    }

    #[test]
    fn component_projection_must_match_the_manifest() {
        let value = manifest(&[("system_prompt", "green", "content", "description")]);
        let manifest_components = parse_manifest(&value).unwrap();
        let good = vec![NewForgeHarnessComponent {
            kind: "system_prompt".into(),
            authority: "green".into(),
            content: "content".into(),
            description: "description".into(),
        }];
        verify_components_match_manifest("rev-1", &manifest_components, &good).unwrap();
        let mut tampered = good.clone();
        tampered[0].content = "different".into();
        assert!(verify_components_match_manifest("rev-1", &manifest_components, &tampered).is_err());
        assert!(verify_components_match_manifest("rev-1", &manifest_components, &[]).is_err());
    }
}
