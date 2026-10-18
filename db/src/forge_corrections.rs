//! Durable correction ledger (migration 295) and the learning writeback read/write helpers.
//!
//! The Phase 3 review deferred three learning pieces that all need the same durable row: the
//! correction → specification → rail pipeline (3.3), the hypothesis ledger (3.4), and the
//! improvement backlog's pattern sources (3.5). This DAO is the one store for all three:
//!
//! * [`ForgeCorrectionsDao::insert_correction`] records what a lane was corrected on. The
//!   `correction_key` is the SHA-256 of the normalized text, so repeats are a `group by` and not a
//!   heuristic.
//! * [`ForgeCorrectionsDao::repeated_corrections`] is the repeat-detection query the Harness
//!   Evolution feed turns into `ImprovementSignal::RepeatedCorrection`.
//! * [`ForgeCorrectionsDao::list_open_learn_findings`] exposes `forge_learn_finding` (the learn
//!   loop's *observation* rows — review candidates, not confirmed defects) as the hypothesis feed.
//!   It reads; it never writes the learn loop's rows.
//! * [`ForgeCorrectionsDao::record_experience`] writes an adopted experiment's lesson through the
//!   existing `forge_harness_experience` row/DAO (migration 294) — no second experience store.
//!
//! No telemetry is copied here: `evidence_refs` holds durable Forge evidence ids and
//! `experiment_id` is an FK to the Harness Evolution experiment. The ledger stores the correction
//! itself, which is not telemetry — it is the instruction a lane was given.
//!
//! Never run migrations from code: 295 is additive and applied by the human migration pipeline.

use crate::{
    Database, DbFailure, DbResult, ForgeHarnessDao, ForgeHarnessExperienceRow,
    NewForgeHarnessExperience,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::FromRow;

/// The durable identity of a correction text: **plain SHA-256 of the normalized UTF-8 text**
/// (trim, internal whitespace collapsed to one space, lower-cased), hex encoded. No salt, no
/// domain prefix, no length prefix — exactly what migration 295's column comment promises, so a
/// reader can recompute a stored key from the text with any SHA-256 tool.
///
/// Normalization deliberately does NOT strip punctuation or words: two different corrections that
/// happen to mention the same file are still different corrections. The key answers "was this same
/// instruction given again?", not "do these sentences look similar?".
pub fn correction_key(correction: &str) -> String {
    let normalized = correction
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let digest = Sha256::digest(normalized.as_bytes());
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// A correction to record. `correction_key` is computed by the DAO from `correction`, never passed
/// by the caller, so a row cannot claim a key the text does not hash to.
#[derive(Debug, Clone)]
pub struct NewForgeCorrection<'a> {
    pub lane: &'a str,
    pub correction: &'a str,
    pub context: Option<&'a str>,
    pub root_cause: Option<&'a str>,
    pub spec_location: Option<&'a str>,
    pub permanent_rule_created: bool,
    pub evidence_refs: Value,
    pub experiment_id: Option<&'a str>,
}

impl<'a> NewForgeCorrection<'a> {
    /// A minimal correction: lane + text + its evidence. Root cause/spec location stay NULL until a
    /// reviewer states them; the ledger does not invent either.
    pub fn new(lane: &'a str, correction: &'a str, evidence_refs: Value) -> Self {
        Self {
            lane,
            correction,
            context: None,
            root_cause: None,
            spec_location: None,
            permanent_rule_created: false,
            evidence_refs,
            experiment_id: None,
        }
    }
}

/// One stored correction.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeCorrectionRow {
    pub id: i64,
    pub lane: String,
    pub correction_key: String,
    pub correction: String,
    pub context: Option<String>,
    pub root_cause: Option<String>,
    pub spec_location: Option<String>,
    pub permanent_rule_created: bool,
    pub evidence_refs: Value,
    pub experiment_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One detected repeat group: same lane, same correction key, `occurrences` times.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeCorrectionRepeatRow {
    pub lane: String,
    pub correction_key: String,
    /// The most recent correction text in the group (the representative instruction).
    pub correction: String,
    pub occurrences: i64,
    pub latest_at: DateTime<Utc>,
    /// Union of the durable evidence ids carried by every row in the group, deduplicated. This is
    /// what lets the feed build an evidence-linked `RepeatedCorrection` target from real rows.
    pub evidence_refs: Value,
}

/// One learn-loop observation ("hypothesis"), read from `forge_learn_finding` (migration 284).
///
/// These are review candidates, not confirmed defects: `occurrence` is how many passes saw the
/// pattern, and `last_story_id` is the story the learn loop filed/updated when it did.
#[derive(Debug, Clone, FromRow)]
pub struct ForgeLearnFindingRow {
    pub repository_key: String,
    pub finding_key: String,
    pub occurrence: i32,
    pub last_story_id: Option<String>,
    pub last_seen_revision: String,
    pub last_seen_at: DateTime<Utc>,
}

/// The correction ledger DAO. Also owns the read of the learn finding rows and the writeback of an
/// adopted experiment's experience record, so the Phase 3 learning loop has one place to write.
#[derive(Clone)]
pub struct ForgeCorrectionsDao {
    db: Database,
}

impl ForgeCorrectionsDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Record one correction. Returns the stored row (including its computed key).
    pub async fn insert_correction(
        &self,
        new: &NewForgeCorrection<'_>,
    ) -> DbResult<ForgeCorrectionRow> {
        let lane = new.lane.trim();
        if lane.is_empty() || new.correction.trim().is_empty() {
            return Err(DbFailure::schema_mismatch(
                "forge_corrections.insert",
                "a correction row needs a non-empty lane and correction text",
            ));
        }
        // The text is stored VERBATIM (only validated through its trimmed view); the key is
        // computed from the normalized view. Storing the trimmed text would quietly rewrite the
        // instruction a lane was given.
        let correction = new.correction;
        let key = correction_key(correction);
        sqlx::query_as::<_, ForgeCorrectionRow>(
            "insert into forge_correction_ledger
                 (lane, correction_key, correction, context, root_cause, spec_location,
                  permanent_rule_created, evidence_refs, experiment_id)
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
             returning id, lane, correction_key, correction, context, root_cause, spec_location,
                       permanent_rule_created, evidence_refs, experiment_id, created_at",
        )
        .bind(lane)
        .bind(&key)
        .bind(correction)
        .bind(new.context)
        .bind(new.root_cause)
        .bind(new.spec_location)
        .bind(new.permanent_rule_created)
        .bind(&new.evidence_refs)
        .bind(new.experiment_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_corrections.insert", &error))
    }

    /// One lane's corrections, newest first.
    pub async fn list_corrections(
        &self,
        lane: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeCorrectionRow>> {
        sqlx::query_as::<_, ForgeCorrectionRow>(
            "select id, lane, correction_key, correction, context, root_cause, spec_location,
                    permanent_rule_created, evidence_refs, experiment_id, created_at
             from forge_correction_ledger where lane=$1
             order by created_at desc, id desc limit $2",
        )
        .bind(lane)
        .bind(limit.clamp(1, 500))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_corrections.list", &error))
    }

    /// Corrections that never became a permanent rule — the correction → rail backlog.
    pub async fn open_corrections(
        &self,
        lane: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeCorrectionRow>> {
        sqlx::query_as::<_, ForgeCorrectionRow>(
            "select id, lane, correction_key, correction, context, root_cause, spec_location,
                    permanent_rule_created, evidence_refs, experiment_id, created_at
             from forge_correction_ledger
             where lane=$1 and permanent_rule_created = false
             order by created_at desc, id desc limit $2",
        )
        .bind(lane)
        .bind(limit.clamp(1, 500))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_corrections.open", &error))
    }

    /// Repeat detection: groups with at least `min_occurrences` rows, strongest first. This is the
    /// query the feed turns into `ImprovementSignal::RepeatedCorrection`.
    pub async fn repeated_corrections(
        &self,
        min_occurrences: i64,
        limit: i64,
    ) -> DbResult<Vec<ForgeCorrectionRepeatRow>> {
        sqlx::query_as::<_, ForgeCorrectionRepeatRow>(
            "select lane,
                    correction_key,
                    (array_agg(correction order by created_at desc, id desc))[1] as correction,
                    count(distinct id)::bigint as occurrences,
                    max(created_at) as latest_at,
                    coalesce(
                        jsonb_agg(distinct evidence_ref)
                            filter (where evidence_ref is not null),
                        '[]'::jsonb
                    ) as evidence_refs
             from forge_correction_ledger row
             left join lateral jsonb_array_elements_text(row.evidence_refs) as evidence_ref on true
             group by lane, correction_key
             having count(distinct id) >= $1
             order by count(distinct id) desc, max(created_at) desc, lane, correction_key
             limit $2",
        )
        .bind(min_occurrences.max(1))
        .bind(limit.clamp(1, 200))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_corrections.repeated", &error))
    }

    /// Learn-loop observations as hypothesis candidates, most-seen first. Read-only.
    pub async fn list_open_learn_findings(
        &self,
        limit: i64,
    ) -> DbResult<Vec<ForgeLearnFindingRow>> {
        sqlx::query_as::<_, ForgeLearnFindingRow>(
            "select repository_key, finding_key, occurrence, last_story_id,
                    last_seen_revision, last_seen_at
             from forge_learn_finding
             order by occurrence desc, last_seen_at desc, repository_key, finding_key
             limit $1",
        )
        .bind(limit.clamp(1, 200))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_corrections.learn_findings", &error))
    }

    /// Writeback for an adopted experiment's lesson: the existing `forge_harness_experience` row and
    /// DAO (migration 294), not a new experience store.
    pub async fn record_experience(
        &self,
        new: &NewForgeHarnessExperience,
    ) -> DbResult<ForgeHarnessExperienceRow> {
        // The existing DAO's own operation name is the better audit label; delegate unchanged.
        ForgeHarnessDao::new(self.db.clone())
            .insert_experience(new)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_correction_key_is_plain_sha256_of_the_normalized_text() {
        // The contract migration 295 documents: any SHA-256 tool must reproduce the key from the
        // trimmed / whitespace-collapsed / lower-cased UTF-8 text.
        let digest = Sha256::digest(b"name the migration");
        let expected: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(correction_key("  Name\tthe   migration "), expected);
    }

    #[test]
    fn the_correction_key_is_normalized_deterministic_and_content_sensitive() {
        let one = correction_key("  Use the   typed DAO,\nnot raw SQL.  ");
        let same = correction_key("use the typed dao, not raw sql.");
        assert_eq!(one, same, "case and whitespace are normalization, not identity");
        assert_eq!(one.len(), 64);
        assert!(one.bytes().all(|byte| byte.is_ascii_hexdigit()));

        let different = correction_key("use the typed DAO, not raw SQL. also name the migration");
        assert_ne!(one, different, "different instruction text is a different correction");
    }

    #[test]
    fn the_correction_key_does_not_merge_documentation_or_telemetry() {
        // Punctuation and wording are preserved (only whitespace/case normalize): "not raw SQL."
        // and "not raw SQL" are the same instruction, but "not raw S Q L" is not.
        assert_eq!(
            correction_key("not raw SQL."),
            correction_key("Not  raw SQL.")
        );
        assert_ne!(
            correction_key("not raw SQL."),
            correction_key("not raw S Q L.")
        );
        // An empty correction still has a stable key; the DAO refuses to store it, the key function
        // does not panic on it (pure input handling).
        assert_eq!(correction_key(""), correction_key("   \n\t "));
    }

    /// DEV smoke test for the repeat grouping and evidence aggregation (migration 295 applied).
    /// Ignored by default; run with DATABASE_URL_DEV present:
    ///
    /// ```sh
    /// set -a; . /Users/Shared/dev/.env.local; set +a
    /// cargo test -p db --lib forge_corrections -- --ignored
    /// ```
    #[test]
    #[ignore = "needs DATABASE_URL_DEV; keeps the unit suite database-free"]
    fn dev_smoke_repeat_grouping_exposes_evidence_and_cleans_up() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let database = runtime
            .block_on(Database::connect_target(crate::DbTarget::Dev))
            .expect("connect DEV");
        let dao = ForgeCorrectionsDao::new(database.clone());
        let lane = format!("smoke-corrections-{}", uuid::Uuid::new_v4());

        let outcome: Result<(), String> = (|| {
            for (index, evidence) in [
                serde_json::json!(["storyboard_story_run:smoke-1"]),
                serde_json::json!(["forge_hold_record:smoke-2"]),
                serde_json::json!(["forge_hold_record:smoke-2", "storyboard_story_run:smoke-1"]),
            ]
            .into_iter()
            .enumerate()
            {
                runtime
                    .block_on(dao.insert_correction(&NewForgeCorrection {
                        lane: &lane,
                        correction: if index == 2 {
                            "  Name the   migration.  "
                        } else {
                            "name the migration."
                        },
                        context: Some("smoke"),
                        root_cause: None,
                        spec_location: None,
                        permanent_rule_created: false,
                        evidence_refs: evidence,
                        experiment_id: None,
                    }))
                    .map_err(|error| format!("insert correction {index}: {error}"))?;
            }
            let repeats = runtime
                .block_on(dao.repeated_corrections(2, 10))
                .map_err(|error| format!("repeat query: {error}"))?;
            let group = repeats
                .iter()
                .find(|row| row.lane == lane)
                .ok_or_else(|| "repeat group for the smoke lane is missing".to_string())?;
            if group.occurrences != 3 {
                return Err(format!("expected 3 occurrences, got {}", group.occurrences));
            }
            let refs: Vec<String> = group
                .evidence_refs
                .as_array()
                .map(|array| {
                    array
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            for expected in [
                "storyboard_story_run:smoke-1",
                "forge_hold_record:smoke-2",
            ] {
                if !refs.contains(&expected.to_string()) {
                    return Err(format!("evidence union is missing {expected}: {refs:?}"));
                }
            }
            if refs.len() != 2 {
                return Err(format!("evidence union should deduplicate: {refs:?}"));
            }
            // The representative text is verbatim (not trimmed) and the key ignores spacing/case.
            if !group.correction.contains("Name the   migration") {
                return Err(format!(
                    "representative correction was rewritten: {:?}",
                    group.correction
                ));
            }
            Ok(())
        })();

        // Cleanup always runs, even when an assertion failed above.
        let cleanup = runtime.block_on(async {
            sqlx::query("delete from forge_correction_ledger where lane=$1")
                .bind(&lane)
                .execute(database.pool())
                .await
        });
        assert!(cleanup.is_ok(), "smoke cleanup must remove its rows");
        outcome.expect("correction repeat-group smoke");
    }

    #[test]
    fn a_minimal_correction_leaves_root_cause_and_spec_location_unset() {
        let evidence = serde_json::json!(["forge_hold_record:1"]);
        let correction = NewForgeCorrection::new("smith", "name the migration", evidence.clone());
        assert_eq!(correction.lane, "smith");
        assert!(correction.root_cause.is_none(), "not invented");
        assert!(correction.spec_location.is_none(), "not invented");
        assert!(!correction.permanent_rule_created);
        assert_eq!(correction.evidence_refs, evidence);
        assert!(correction.experiment_id.is_none());
    }
}
