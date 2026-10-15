// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

struct FakeOps(PublishOutcome);
impl ForgeReleaseOperations for FakeOps {
    fn apply_migrations(&self, _: &str, _: &[String], _: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "migrations applied".into(),
        }
    }
    fn verify_migrations(&self, _: &str, _: &[String]) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "migrations verified".into(),
        }
    }
    fn refresh_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "refresh complete".into(),
        }
    }
    fn verify_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: true,
            detail: "refresh verified".into(),
        }
    }
    fn publish(&self, _: Option<&str>, _: &[String]) -> PublishOutcome {
        self.0.clone()
    }
    fn reconcile_publish(&self, _: &str) -> PublishReconciliation {
        PublishReconciliation::NotPublished
    }
}

struct RecoveryOps {
    publishes: std::sync::atomic::AtomicUsize,
    readback: Option<PublishOutcome>,
}

impl ForgeReleaseOperations for RecoveryOps {
    fn apply_migrations(&self, _: &str, _: &[String], _: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "must not reapply during recovery".into(),
        }
    }
    fn verify_migrations(&self, _: &str, _: &[String]) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn refresh_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn verify_derived(&self, _: &[String], _: &str) -> ForgeOperationResult {
        self.apply_migrations("", &[], "")
    }
    fn publish(&self, _: Option<&str>, _: &[String]) -> PublishOutcome {
        self.publishes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        PublishOutcome::PublishConflict {
            reason: "recovery must not publish".into(),
        }
    }
    fn reconcile_publish(&self, _: &str) -> PublishReconciliation {
        match self.readback.clone() {
            Some(outcome) => PublishReconciliation::Published(outcome),
            None => PublishReconciliation::Unknown("fixture readback unavailable".into()),
        }
    }
}

struct FakeEvidence {
    fail: bool,
    read_failure: Option<String>,
    current: ForgeGateEvidence,
    patches: Mutex<Vec<ForgeGateEvidence>>,
    receipts: Mutex<HashMap<String, db::ForgeReleaseOperationReceipt>>,
}
impl EvidenceStore for FakeEvidence {
    fn read(&self, _: &str) -> std::result::Result<ForgeGateEvidence, String> {
        match &self.read_failure {
            Some(detail) => Err(detail.clone()),
            None => Ok(self.current.clone()),
        }
    }
    fn merge(&self, _: &str, _: &str, patch: ForgeGateEvidence) -> std::result::Result<(), String> {
        if self.fail {
            return Err("injected DB failure".into());
        }
        self.patches.lock().unwrap().push(patch);
        Ok(())
    }
    fn operation_receipt(
        &self,
        command_id: &str,
    ) -> std::result::Result<Option<db::ForgeReleaseOperationReceipt>, String> {
        Ok(self.receipts.lock().unwrap().get(command_id).cloned())
    }
    fn record_operation_receipt(
        &self,
        env: &ForgeCommandEnvelope,
        command_id: &str,
        result: &serde_json::Value,
    ) -> std::result::Result<(), String> {
        let mut receipts = self.receipts.lock().unwrap();
        let next = db::ForgeReleaseOperationReceipt {
            command_id: command_id.into(),
            process_instance_id: env.process_instance_id.clone(),
            story_id: env.story_id.clone(),
            command_type: env.command_type.clone(),
            result: result.clone(),
            settled: false,
        };
        if let Some(existing) = receipts.get(command_id) {
            if existing.process_instance_id != next.process_instance_id
                || existing.story_id != next.story_id
                || existing.command_type != next.command_type
                || existing.result != next.result
            {
                return Err("release operation identity conflict".into());
            }
        } else {
            receipts.insert(command_id.into(), next);
        }
        Ok(())
    }
    fn settle_operation(
        &self,
        env: &ForgeCommandEnvelope,
        command_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String> {
        if self.fail {
            return Err("injected DB failure".into());
        }
        let mut receipts = self.receipts.lock().unwrap();
        let receipt = receipts
            .get_mut(command_id)
            .ok_or_else(|| "receipt missing".to_string())?;
        if receipt.process_instance_id != env.process_instance_id
            || receipt.story_id != env.story_id
        {
            return Err("release operation identity conflict".into());
        }
        receipt.settled = true;
        self.patches.lock().unwrap().push(patch);
        Ok(())
    }
    fn latest_refresh_command_id(&self, _: &str) -> Option<String> {
        None
    }
    fn frozen_proofs(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
}

fn executor(
    outcome: PublishOutcome,
    fail_merge: bool,
) -> DbForgeReleaseExecutor<FakeOps, FakeEvidence> {
    DbForgeReleaseExecutor {
        operations: FakeOps(outcome),
        evidence: FakeEvidence {
            fail: fail_merge,
            read_failure: None,
            current: ForgeGateEvidence {
                candidate_sha: Some("a".repeat(40)),
                qa_verified_sha: Some("a".repeat(40)),
                qa_passed: Some(true),
                ..Default::default()
            },
            patches: Mutex::new(Vec::new()),
            receipts: Mutex::new(HashMap::new()),
        },
        pending: None,
    }
}

fn envelope() -> ForgeCommandEnvelope {
    ForgeCommandEnvelope {
        command_type: "forge.publish_candidate".into(),
        command_id: "cmd-original".into(),
        causation_id: None,
        process_instance_id: "process-1".into(),
        story_id: "story-1".into(),
    }
}

#[test]
fn successful_publish_with_failed_evidence_is_explicitly_unsettled() {
    let command = executor(
        PublishOutcome::Published {
            published_main_hash: "b".repeat(40),
        },
        true,
    )
    .execute(&envelope());
    assert_eq!(
        command.outcome,
        ApplicationCommandOutcome::SettlementRequired
    );
    let message = command.message.unwrap();
    assert!(message.contains("external_operation_succeeded=true"));
    assert!(message.contains("cmd-original"));
    assert!(message.contains("injected DB failure"));
}

#[test]
fn successful_publish_and_evidence_settlement_keep_success_outcome() {
    let executor = executor(
        PublishOutcome::Published {
            published_main_hash: "b".repeat(40),
        },
        false,
    );
    let command = executor.execute(&envelope());
    assert_eq!(command.outcome, ApplicationCommandOutcome::Success);
    assert_eq!(
        executor.evidence.patches.lock().unwrap()[0].publish_succeeded,
        Some(true)
    );
}

#[test]
fn unreadable_release_evidence_holds_instead_of_reading_as_empty() {
    let mut executor = executor(
        PublishOutcome::Published {
            published_main_hash: "b".repeat(40),
        },
        false,
    );
    executor.evidence.read_failure = Some("injected evidence read failure".into());

    let command = executor.execute(&envelope());

    assert_eq!(
        command.outcome,
        ApplicationCommandOutcome::SettlementRequired,
        "a gate that could not be read is not a gate that passed"
    );
    let message = command.message.expect("settlement diagnostic");
    assert!(
        message.contains("injected evidence read failure"),
        "{message}"
    );
    assert!(
        message.contains("external_operation_succeeded=false"),
        "{message}"
    );
    assert!(
        executor.evidence.patches.lock().unwrap().is_empty(),
        "no evidence may be written when the evidence could not be read"
    );
}

#[test]
fn failed_publish_remains_a_failed_fact_when_its_evidence_settles() {
    let executor = executor(
        PublishOutcome::PublishConflict {
            reason: "remote conflict".into(),
        },
        false,
    );
    let command = executor.execute(&envelope());
    assert_eq!(command.outcome, ApplicationCommandOutcome::Success);
    let patch = &executor.evidence.patches.lock().unwrap()[0];
    assert_eq!(patch.publish_succeeded, Some(false));
    assert_eq!(patch.failure_class.as_deref(), Some("PUBLISH_CONFLICT"));
}

#[test]
fn migration_and_refresh_settlement_failures_are_non_successful() {
    for command_type in ["forge.migrate_dev", "forge.refresh_derived_models"] {
        let mut env = envelope();
        env.command_type = command_type.into();
        let command = executor(
            PublishOutcome::PublishConflict {
                reason: "unused".into(),
            },
            true,
        )
        .execute(&env);
        assert_eq!(
            command.outcome,
            ApplicationCommandOutcome::SettlementRequired
        );
        let message = command.message.expect("settlement diagnostic");
        assert!(
            message.contains("external_operation_succeeded=true"),
            "{message}"
        );
        assert!(message.contains(&env.command_id), "{message}");
    }
}

#[test]
fn saved_publish_result_settles_without_repeating_the_operation() {
    let executor = recovery_executor(None);
    let original = envelope();
    let result = PublishOutcome::Published {
        published_main_hash: "b".repeat(40),
    };
    let result_json = serde_json::to_value(&result).unwrap();
    executor
        .evidence
        .record_operation_receipt(&original, &original.command_id, &result_json)
        .unwrap();
    executor.evidence.patches.lock().unwrap().clear();
    let mut resumed = original.clone();
    resumed.command_id = "cmd-resume".into();
    resumed.causation_id = Some(original.command_id.clone());

    let command = executor.execute(&resumed);
    assert_eq!(command.outcome, ApplicationCommandOutcome::Success);
    assert_eq!(
        executor
            .operations
            .publishes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let expected_sha = "b".repeat(40);
    assert_eq!(
        executor.evidence.patches.lock().unwrap()[0]
            .published_sha
            .as_deref(),
        Some(expected_sha.as_str())
    );
    assert!(
        executor
            .evidence
            .operation_receipt(&original.command_id)
            .unwrap()
            .unwrap()
            .settled
    );
}

fn recovery_executor(
    readback: Option<PublishOutcome>,
) -> DbForgeReleaseExecutor<RecoveryOps, FakeEvidence> {
    DbForgeReleaseExecutor {
        operations: RecoveryOps {
            publishes: std::sync::atomic::AtomicUsize::new(0),
            readback,
        },
        evidence: FakeEvidence {
            fail: false,
            read_failure: None,
            current: ForgeGateEvidence {
                candidate_sha: Some("a".repeat(40)),
                qa_verified_sha: Some("a".repeat(40)),
                qa_passed: Some(true),
                ..Default::default()
            },
            patches: Mutex::new(Vec::new()),
            receipts: Mutex::new(HashMap::new()),
        },
        pending: None,
    }
}

#[test]
fn authoritative_publish_readback_reconciles_without_republishing() {
    let executor = recovery_executor(Some(PublishOutcome::IntegratedAndPublished {
        published_main_hash: "c".repeat(40),
    }));
    let mut resumed = envelope();
    resumed.command_id = "cmd-resume".into();
    resumed.causation_id = Some("cmd-original".into());
    let command = executor.execute(&resumed);
    assert_eq!(command.outcome, ApplicationCommandOutcome::Success);
    assert_eq!(
        executor
            .operations
            .publishes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let receipt = executor
        .evidence
        .operation_receipt("cmd-original")
        .unwrap()
        .unwrap();
    assert!(receipt.settled);
    assert_eq!(receipt.command_id, "cmd-original");
    assert_eq!(
        executor.evidence.patches.lock().unwrap()[0]
            .published_sha
            .as_deref(),
        Some("c".repeat(40).as_str())
    );
}

#[test]
fn inconclusive_publish_readback_remains_held_without_blind_retry() {
    let executor = recovery_executor(None);
    let mut resumed = envelope();
    resumed.command_id = "cmd-resume".into();
    resumed.causation_id = Some("cmd-original".into());
    let command = executor.execute(&resumed);
    assert_eq!(
        command.outcome,
        ApplicationCommandOutcome::SettlementRequired
    );
    assert!(command.message.unwrap().contains("no publish was repeated"));
    assert_eq!(
        executor
            .operations
            .publishes
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(executor.evidence.patches.lock().unwrap().is_empty());
}
