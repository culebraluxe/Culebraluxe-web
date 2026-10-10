//! Completion unit — engine transition already won (ENG-13).
//!
//! Receipt + evidence merge + repair/replan spend are ONE atomic write (FORGE-B1 slice 2).
//! Receipt id: `forge.completion:{taskId}`. The receipt IS the unit's proof: it exists exactly when the
//! unit's effects are committed, so absence of a receipt is the crash window and a receipt is the answer
//! "already applied".
//!
//! WHY ONE WRITE. The unit used to be applied in four calls (claim → merge → count → finalize) and each
//! boundary was a window: a process that died after the merge and the count but before the finalize left
//! the evidence merged and the budget spent with no receipt, and the next process to look — a resume, a
//! fresh dispatch — re-applied the whole unit and spent the budget AGAIN. The repair budget could be
//! exhausted by crashes alone. Collapsing the unit into one committed effect removes the window rather
//! than compensating for it.

use std::collections::BTreeMap;
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use workflow::{Result, WorkflowError};

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::runtime::completion_receipt_id;

#[derive(Debug, Clone, Default)]
pub struct CompletionRecord {
    pub task_id: String,
    pub process_instance_id: String,
    pub story_id: String,
    pub node_id: Option<String>,
    pub evidence: ForgeGateEvidence,
}

impl CompletionRecord {
    /// The budget this node's completion spends, or `None` for a node that spends none.
    ///
    /// Both repair Smiths spend the one repair budget. `fast_repair_smith` was not counted, so the FAST
    /// lane's budget could never be spent and a failing candidate looped Smith ↔ QA until the money ran out.
    pub fn spend(&self) -> Option<CompletionSpend> {
        match self.node_id.as_deref() {
            Some("repair_smith") | Some("fast_repair_smith") => Some(CompletionSpend::Repair),
            Some("repair_architect") => Some(CompletionSpend::Replan),
            _ => None,
        }
    }

    /// The identity of the unit's receipt key: the one story this unit's effects belong to.
    /// The versioned digest covers every persisted effect and immutable event identity. `task_id` remains
    /// the receipt key, while incidental transport/logging metadata is intentionally excluded.
    pub fn fingerprint(&self) -> String {
        let unit = CompletionFingerprint {
            version: 2,
            task_id: &self.task_id,
            process_instance_id: &self.process_instance_id,
            story_id: &self.story_id,
            node_id: &self.node_id,
            spend: self.spend().map(|spend| match spend {
                CompletionSpend::Repair => "repair",
                CompletionSpend::Replan => "replan",
            }),
            evidence: evidence_fingerprint_value(&self.evidence),
        };
        // All fields are typed in-memory values and serde_json's serializer is infallible for this
        // structure; object maps are canonicalized before hashing to make key order irrelevant.
        let mut value = serde_json::to_value(unit).expect("completion fingerprint serialization");
        canonicalize_json(&mut value);
        let bytes = serde_json::to_vec(&value).expect("canonical completion serialization");
        format!("completion:v2:{:x}", Sha256::digest(bytes))
    }

    /// The durable assay receipt linked to this task, when the completed evidence carries one.
    /// Malformed metadata is an error: a completion may not silently drop a receipt link.
    pub fn assay_receipt_link(&self) -> std::result::Result<Option<AssayReceiptLink>, String> {
        let Some(value) = self.evidence.extra.get("assayReceipt") else {
            return Ok(None);
        };
        let get_required = |key: &str| {
            value
                .get(key)
                .and_then(workflow::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("assayReceipt.{key} is required"))
        };
        let get_optional = |key: &str| match value.get(key) {
            None | Some(workflow::Value::Null) => Ok(None),
            Some(value) => value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("assayReceipt.{key} must be a string or null"))
                .map(Some),
        };
        Ok(Some(AssayReceiptLink {
            artifact_id: get_required("artifactId")?,
            story_run_id: get_required("storyRunId")?,
            idempotency_key: get_required("idempotencyKey")?,
            measurement_node: get_required("measurementNode")?,
            gate_verdict: get_required("gateVerdict")?,
            plan_sha256: get_optional("planSha256")?,
            candidate_sha: get_optional("candidateSha")?,
        }))
    }
}

#[derive(serde::Serialize)]
struct CompletionFingerprint<'a> {
    version: u8,
    task_id: &'a str,
    process_instance_id: &'a str,
    story_id: &'a str,
    node_id: &'a Option<String>,
    spend: Option<&'static str>,
    evidence: serde_json::Value,
}

fn workflow_value_json(value: &workflow::Value) -> serde_json::Value {
    match value {
        workflow::Value::Null => serde_json::Value::Null,
        workflow::Value::Bool(value) => serde_json::Value::Bool(*value),
        workflow::Value::Number(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(format!("non-finite:{value:?}"))),
        workflow::Value::String(value) => serde_json::Value::String(value.clone()),
        workflow::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(workflow_value_json).collect())
        }
        workflow::Value::Object(values) => {
            let mut object = serde_json::Map::new();
            for (key, value) in values {
                object.insert(key.clone(), workflow_value_json(value));
            }
            serde_json::Value::Object(object)
        }
    }
}

fn evidence_fingerprint_value(evidence: &ForgeGateEvidence) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut object = Map::new();
    macro_rules! add {
        ($field:ident) => {
            object.insert(
                stringify!($field).to_string(),
                serde_json::to_value(&evidence.$field).expect("typed evidence serialization"),
            );
        };
    }
    macro_rules! add_workflow_value {
        ($field:ident) => {
            object.insert(
                stringify!($field).to_string(),
                evidence
                    .$field
                    .as_ref()
                    .map(workflow_value_json)
                    .unwrap_or(Value::Null),
            );
        };
    }
    add!(work_type);
    add!(research_disposition);
    add!(scout_required);
    add!(root_cause_known);
    add!(diagnosis_blocked);
    add!(architecture_suspect);
    add!(architecture_review_required);
    add!(lead_decision);
    add!(split_count);
    add_workflow_value!(lead_routing);
    add!(deployment_deferred_to_batch);
    add_workflow_value!(findings);
    add!(deliverable_rejection);
    add!(qa_review_required);
    add!(qa_review_passed);
    add!(qa_passed);
    add!(disposition);
    add!(verification_gap);
    add!(no_progress);
    add!(repair_attempts);
    add!(replan_attempts);
    add!(last_failure);
    add!(failure_class);
    add!(failed_release_stage);
    add!(classifier_failure_class);
    add!(stage_failure_class);
    add!(publish_succeeded);
    add!(migration_required);
    add!(migration_files);
    add!(dev_migration_applied);
    add!(dev_migration_verified);
    add!(prod_migration_applied);
    add!(prod_migration_verified);
    add!(derived_refresh_required);
    add!(derived_models);
    add!(derived_refresh_succeeded);
    add!(derived_refresh_verified);
    add!(deployment_required);
    add!(deployment_succeeded);
    add!(deployment_deferred);
    add!(release_deferred);
    add!(deployment_receipt);
    add!(production_verified);
    add!(production_verification_receipt);
    add!(candidate_sha);
    add!(qa_verified_sha);
    add!(published_sha);
    add!(deployed_sha);
    add!(production_verified_sha);
    add!(batch_released_sha);
    add!(batch_released_at);
    add!(batch_release_receipt);
    add!(resume_target);
    add!(role_output_schema_version);
    add!(role_output_diagnostic);
    object.insert("extra".into(), workflow_value_json(&evidence.extra));
    Value::Object(object)
}

fn canonicalize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => items.iter_mut().for_each(canonicalize_json),
        serde_json::Value::Object(object) => {
            let mut entries = object
                .iter_mut()
                .map(|(key, value)| {
                    canonicalize_json(value);
                    (key.clone(), value.clone())
                })
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            object.clear();
            for (key, value) in entries {
                object.insert(key, value);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssayReceiptLink {
    pub artifact_id: String,
    pub story_run_id: String,
    pub idempotency_key: String,
    pub measurement_node: String,
    pub gate_verdict: String,
    pub plan_sha256: Option<String>,
    pub candidate_sha: Option<String>,
}

/// Which budget a completion spends. The mapping from a node to a budget is
/// [`CompletionRecord::spend`]; this names the budget that was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSpend {
    Repair,
    Replan,
}

/// What applying a unit did — the two answers a caller may act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionApply {
    /// This call wrote the unit: receipt, evidence merge and budget spend are committed together.
    Applied,
    /// A committed receipt already proves this unit. Nothing was written, and nothing was replayed.
    AlreadyApplied,
}

impl CompletionApply {
    /// `true` only for [`CompletionApply::Applied`] — the one answer a resume may count as work done.
    pub fn applied(self) -> bool {
        matches!(self, CompletionApply::Applied)
    }
}

pub trait CompletionLedger: Send + Sync {
    /// Apply the unit — receipt, evidence merge and budget spend — as ONE committed effect.
    ///
    /// `Applied` means this call committed it; `AlreadyApplied` means a committed receipt already proves
    /// the unit and nothing was replayed. Everything else — a conflicting unit, another process mid-unit,
    /// a database that cannot answer — is an `Err`: a lost connection must never read as "someone else
    /// applied it" and quietly drop the evidence merge and the counters.
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply>;
    fn has_final(&self, receipt_id: &str) -> Result<bool>;
    /// Read one bounded page of accepted completions that still lack final effects. Durable ledgers
    /// implement this from their authoritative event/receipt tables; `None` asks the runtime to use
    /// the workflow store's explicit instance/event pagination (the in-memory test ledger).
    fn unfinished(
        &self,
        _story_id: Option<&str>,
        _limit: usize,
    ) -> Result<Option<Vec<CompletionRecord>>> {
        Ok(None)
    }
    /// Return the story's repair and replan budget to full: a fresh instance is a fresh attempt (a board rerun),
    /// and the attempts an earlier instance spent are not this one's. The default (a ledger with no budget of its
    /// own) does nothing.
    fn reset_budget(&self, _story_id: &str) -> Result<()> {
        Ok(())
    }
    /// Newest finalized receipt time for `forge.completion:` — the reconcile watermark, in the same
    /// unit as `ProcessEvent::created_at` (epoch milliseconds).
    fn watermark(&self, _prefix: &str) -> Result<Option<i64>> {
        Ok(None)
    }
}

/// The in-memory ledger: the same unit in one lock and one map, so a reader can never see a receipt without
/// its effects.
///
/// It answers the durable routine's questions the same way on purpose, conflict included — a second
/// implementation that called an unfinished or foreign unit "applied" would be a second adjudicator of the
/// same fact.
#[derive(Default)]
pub struct MemoryLedger {
    state: Mutex<MemoryState>,
}

#[derive(Default)]
struct MemoryState {
    /// Receipt id -> the fingerprint of the unit it proves. A receipt exists ONLY with committed effects,
    /// and it is written last for that reason.
    receipts: BTreeMap<String, String>,
    /// Receipt id -> the clock stamp at which the unit committed (the reconcile watermark).
    committed_at: BTreeMap<String, i64>,
    clock: i64,
    evidence: BTreeMap<String, ForgeGateEvidence>,
    repairs: BTreeMap<String, u32>,
    replans: BTreeMap<String, u32>,
}

impl MemoryLedger {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn repairs(&self, story_id: &str) -> u32 {
        *self
            .state
            .lock()
            .unwrap()
            .repairs
            .get(story_id)
            .unwrap_or(&0)
    }
    pub fn replans(&self, story_id: &str) -> u32 {
        *self
            .state
            .lock()
            .unwrap()
            .replans
            .get(story_id)
            .unwrap_or(&0)
    }
    pub fn evidence_for(&self, story_id: &str) -> Option<ForgeGateEvidence> {
        self.state.lock().unwrap().evidence.get(story_id).cloned()
    }
}

impl MemoryState {
    /// The entire unit, under the caller's lock: evidence, spend, then the receipt — so no observer can
    /// read a spent budget or a merged evidence row that no receipt stands behind.
    fn apply(&mut self, rec: &CompletionRecord) -> Result<CompletionApply> {
        let id = completion_receipt_id(&rec.task_id);
        let fingerprint = rec.fingerprint();
        if let Some(stored) = self.receipts.get(&id) {
            if *stored == fingerprint {
                return Ok(CompletionApply::AlreadyApplied);
            }
            return Err(WorkflowError::generic(format!(
                "completion receipt {id} belongs to {stored}, not {fingerprint}: refusing to replay a settled unit"
            )));
        }
        match self.evidence.get(&rec.story_id) {
            Some(prev) => {
                let next = rec.evidence.merge_over(prev);
                self.evidence.insert(rec.story_id.clone(), next);
            }
            None => {
                self.evidence
                    .insert(rec.story_id.clone(), rec.evidence.clone());
            }
        }
        match rec.spend() {
            Some(CompletionSpend::Repair) => {
                *self.repairs.entry(rec.story_id.clone()).or_insert(0) += 1
            }
            Some(CompletionSpend::Replan) => {
                *self.replans.entry(rec.story_id.clone()).or_insert(0) += 1
            }
            None => {}
        }
        self.receipts.insert(id.clone(), fingerprint);
        self.clock += 1;
        self.committed_at.insert(id, self.clock);
        Ok(CompletionApply::Applied)
    }
}

impl CompletionLedger for MemoryLedger {
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply> {
        self.state.lock().unwrap().apply(rec)
    }

    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        Ok(self.state.lock().unwrap().receipts.contains_key(receipt_id))
    }

    fn reset_budget(&self, story_id: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.repairs.remove(story_id);
        state.replans.remove(story_id);
        Ok(())
    }

    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .committed_at
            .iter()
            .filter(|(id, _)| id.starts_with(prefix))
            .map(|(_, t)| *t)
            .max())
    }
}

/// Apply the post-transition unit, through whichever ledger the runtime holds.
///
/// One call, one effect. The whole unit is handed to the ledger because the receipt and its effects must
/// commit together: the old four-call sequence (claim → merge → count → finalize) could die between its
/// steps and leave the evidence merged and the budget spent with no receipt standing behind them — which is
/// exactly what made a resume spend the repair budget a second time
/// (`legacy/workflow_app/tests/interrupted-sequences.test.ts`: the transition is durable, the evidence is
/// incomplete, no receipt). The durable ledger commits the lot in one transaction; the in-memory one holds
/// one lock across it. Either way a receipt that exists is a unit that happened, and its absence is the
/// crash window the resume reconciles.
pub fn apply_completion_unit(
    ledger: &dyn CompletionLedger,
    rec: CompletionRecord,
) -> Result<CompletionApply> {
    ledger.apply(&rec)
}

#[cfg(test)]
mod spend_tests {
    use super::*;

    fn unit_at(node: Option<&str>) -> CompletionRecord {
        CompletionRecord {
            task_id: "t-1".into(),
            process_instance_id: "p-1".into(),
            story_id: "ENG-1".into(),
            node_id: node.map(str::to_string),
            evidence: ForgeGateEvidence::default(),
        }
    }

    /// Work order §6: both repair Smiths spend the repair budget, `repair_architect` spends the replan one.
    /// The FAST lane is the case that was missed — `fast_repair_smith` was not counted, so its budget could
    /// never be exhausted and a failing candidate looped Smith ↔ QA.
    #[test]
    fn both_repair_smiths_spend_the_repair_budget() {
        assert_eq!(
            unit_at(Some("repair_smith")).spend(),
            Some(CompletionSpend::Repair)
        );
        assert_eq!(
            unit_at(Some("fast_repair_smith")).spend(),
            Some(CompletionSpend::Repair)
        );
        assert_eq!(
            unit_at(Some("repair_architect")).spend(),
            Some(CompletionSpend::Replan)
        );
    }

    /// Every other node spends nothing, and a unit with no node at all spends nothing: an unspent node must
    /// never move a counter (the counter is the budget guard, not a progress log).
    #[test]
    fn only_the_repair_and_replan_nodes_spend() {
        for node in [
            "smith",
            "fast_smith",
            "architect",
            "qa",
            "lead_post",
            "dev_ops",
        ] {
            assert_eq!(unit_at(Some(node)).spend(), None, "{node} spends no budget");
        }
        assert_eq!(
            unit_at(None).spend(),
            None,
            "a node-less unit spends nothing"
        );
    }
}

#[cfg(test)]
mod assay_receipt_link_tests {
    use super::*;

    fn record_with_assay_receipt() -> CompletionRecord {
        let mut record = CompletionRecord {
            task_id: "task-qa".into(),
            process_instance_id: "process-1".into(),
            story_id: "STORY-1".into(),
            node_id: Some("qa_verify".into()),
            evidence: ForgeGateEvidence::default(),
        };
        record.evidence.extra.insert(
            "assayReceipt",
            workflow::json!({
                "artifactId": "artifact-1",
                "storyRunId": "run-1",
                "idempotencyKey": "assay-key",
                "measurementNode": "qa_verify",
                "gateVerdict": "Pass",
                "planSha256": "plan-hash",
                "candidateSha": "candidate-sha",
            }),
        );
        record
    }

    #[test]
    fn assay_receipt_identity_is_part_of_completion_fingerprint_and_link() {
        let record = record_with_assay_receipt();
        assert!(record.fingerprint().starts_with("completion:v2:"));
        assert_eq!(
            record.assay_receipt_link().unwrap(),
            Some(AssayReceiptLink {
                artifact_id: "artifact-1".into(),
                story_run_id: "run-1".into(),
                idempotency_key: "assay-key".into(),
                measurement_node: "qa_verify".into(),
                gate_verdict: "Pass".into(),
                plan_sha256: Some("plan-hash".into()),
                candidate_sha: Some("candidate-sha".into()),
            })
        );
        let mut different = record.clone();
        different.evidence.extra.insert(
            "assayReceipt",
            workflow::json!({
                "artifactId": "artifact-2",
                "storyRunId": "run-1",
                "idempotencyKey": "assay-key-2",
                "measurementNode": "qa_verify",
                "gateVerdict": "Pass",
                "planSha256": "plan-hash",
                "candidateSha": "candidate-sha",
            }),
        );
        assert_ne!(record.fingerprint(), different.fingerprint());
    }

    #[test]
    fn fingerprint_covers_every_effect_and_provenance_field() {
        let record = record_with_assay_receipt();
        let baseline = record.fingerprint();
        let mut variants = Vec::new();
        let mut changed = record.clone();
        changed.process_instance_id.push_str("-other");
        variants.push(changed);
        let mut changed = record.clone();
        changed.node_id = Some("repair_smith".into());
        variants.push(changed);
        let mut changed = record.clone();
        changed.story_id.push_str("-other");
        variants.push(changed);
        let mut changed = record.clone();
        changed.evidence.last_failure = Some("different accepted failure".into());
        variants.push(changed);
        let mut changed = record.clone();
        changed.evidence.qa_passed = Some(false);
        variants.push(changed);
        let mut changed = record.clone();
        changed.evidence.extra.insert(
            "assayReceipt",
            workflow::json!({
                "artifactId": "artifact-1",
                "storyRunId": "run-1",
                "idempotencyKey": "assay-key",
                "measurementNode": "qa_verify",
                "gateVerdict": "Fail",
                "planSha256": "plan-hash",
                "candidateSha": "candidate-sha",
            }),
        );
        variants.push(changed);
        for variant in variants {
            assert_ne!(baseline, variant.fingerprint());
        }
    }

    #[test]
    fn fingerprint_is_stable_for_equivalent_json_object_key_order() {
        let mut first = record_with_assay_receipt();
        let mut second = first.clone();
        let mut left = std::collections::BTreeMap::new();
        left.insert("alpha".to_string(), workflow::Value::from("one"));
        left.insert("omega".to_string(), workflow::Value::from("two"));
        let mut right = std::collections::BTreeMap::new();
        right.insert("omega".to_string(), workflow::Value::from("two"));
        right.insert("alpha".to_string(), workflow::Value::from("one"));
        first
            .evidence
            .extra
            .insert("payload", workflow::Value::Object(left));
        second
            .evidence
            .extra
            .insert("payload", workflow::Value::Object(right));
        assert_eq!(first.fingerprint(), second.fingerprint());
    }

    #[test]
    fn malformed_assay_receipt_metadata_is_not_dropped() {
        let mut record = CompletionRecord {
            task_id: "task-qa".into(),
            process_instance_id: "process-1".into(),
            story_id: "STORY-1".into(),
            node_id: Some("qa_verify".into()),
            evidence: ForgeGateEvidence::default(),
        };
        record
            .evidence
            .extra
            .insert("assayReceipt", workflow::json!({"artifactId":"artifact-1"}));
        assert!(record.assay_receipt_link().is_err());
    }
}
