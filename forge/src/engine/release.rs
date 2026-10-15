//! Port of db-release-executor.ts. Operations are injected; Neon SQL is unchanged.

use crate::engine::facts::{forge_lineage_error, ForgeGateEvidence};
use serde::{Deserialize, Serialize};
use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

#[derive(Debug, Clone)]
pub struct ForgeCommandEnvelope {
    pub command_type: String,
    pub command_id: String,
    /// Original operation identity when this invocation is a settlement-only recovery.
    pub causation_id: Option<String>,
    pub process_instance_id: String,
    pub story_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeOperationResult {
    pub success: bool,
    pub detail: String,
}

pub trait ForgeReleaseOperations: Send + Sync {
    fn apply_migrations(
        &self,
        target: &str,
        files: &[String],
        command_id: &str,
    ) -> ForgeOperationResult;
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult;
    fn refresh_derived(&self, models: &[String], command_id: &str) -> ForgeOperationResult;
    fn verify_derived(&self, models: &[String], attempt_command_id: &str) -> ForgeOperationResult;
    fn publish(&self, candidate_sha: Option<&str>, frozen_proofs: &[String]) -> PublishOutcome;
    /// Read-only reconciliation after a crash left no saved operation receipt. This must never publish.
    fn reconcile_publish(&self, _candidate_sha: &str) -> PublishReconciliation {
        PublishReconciliation::Unknown("publish readback is unavailable".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublishOutcome {
    Published {
        published_main_hash: String,
    },
    IntegratedAndPublished {
        published_main_hash: String,
    },
    NoCandidate {
        reason: String,
    },
    IntegrationUnverified {
        integrated_commit: String,
        reason: String,
    },
    IntegrationConflict {
        reason: String,
    },
    CandidateSecret {
        reason: String,
    },
    PublishConflict {
        reason: String,
    },
    /// The publish kill switch is held open (`FORGE_ALLOW_PUBLISH` says off). Its own outcome, and never folded
    /// into `PublishConflict`: a configuration refusal wearing a git conflict's name is a refusal nobody
    /// investigates, which is exactly how TST-ACCOUNTING-CORE-008's candidate stranded on 2026-10-01.
    PublishDisabled {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub enum PublishReconciliation {
    /// Destination history proves the candidate is already published.
    Published(PublishOutcome),
    /// Destination history is authoritative and proves the candidate is not present.
    NotPublished,
    /// The check could not establish the outcome. Callers must hold without publishing.
    Unknown(String),
}

pub trait EvidenceStore: Send + Sync {
    /// A read that FAILED is not a read that found nothing. The publish gate cannot be evaluated without the
    /// durable evidence, so the caller must be told: defaulting here is how a story with sound evidence got
    /// `publishSucceeded=false` written onto it, and how the originating error never reached the receipt.
    fn read(&self, story_id: &str) -> std::result::Result<ForgeGateEvidence, String>;
    fn merge(
        &self,
        process_instance_id: &str,
        story_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String>;
    fn operation_receipt(
        &self,
        operation_command_id: &str,
    ) -> std::result::Result<Option<db::ForgeReleaseOperationReceipt>, String>;
    fn record_operation_receipt(
        &self,
        env: &ForgeCommandEnvelope,
        operation_command_id: &str,
        result: &serde_json::Value,
    ) -> std::result::Result<(), String>;
    fn settle_operation(
        &self,
        env: &ForgeCommandEnvelope,
        operation_command_id: &str,
        patch: ForgeGateEvidence,
    ) -> std::result::Result<(), String>;
    fn latest_refresh_command_id(&self, process_instance_id: &str) -> Option<String>;
    fn frozen_proofs(&self, story_id: &str) -> Vec<String>;
}

pub struct DbForgeReleaseExecutor<O, E> {
    pub operations: O,
    pub evidence: E,
    pub pending: Option<ForgeGateEvidence>,
}

impl<O: ForgeReleaseOperations, E: EvidenceStore> DbForgeReleaseExecutor<O, E> {
    pub fn execute(&self, envelope: &ForgeCommandEnvelope) -> ApplicationCommandResult {
        if envelope.process_instance_id.trim().is_empty() || envelope.story_id.trim().is_empty() {
            return ApplicationCommandResult {
                command_id: envelope.command_id.clone(),
                outcome: ApplicationCommandOutcome::PreconditionFailure,
                message: Some("Forge release command is missing process/story context".into()),
            };
        }
        // An unreadable store holds the command; it does not become empty evidence. With default evidence the
        // lineage check fails for a missing candidate SHA and, if the merge below then succeeds, the run writes
        // `publishSucceeded=false` onto a story whose evidence was never read — a settlement failure reported as
        // a publish failure. `settlement_failure` carries this error into the receipt, where it can be read.
        let stored = match self.evidence.read(&envelope.story_id) {
            Ok(stored) => stored,
            Err(error) => return settlement_failure(envelope, false, error),
        };
        let evidence = match &self.pending {
            Some(p) => p.merge_over(&stored),
            None => stored,
        };
        if let Some(operation_command_id) = envelope.causation_id.as_deref() {
            return self.resume_settlement(envelope, operation_command_id, &evidence);
        }
        match self.evidence.operation_receipt(&envelope.command_id) {
            Ok(Some(receipt)) => {
                if receipt.command_type != envelope.command_type
                    || receipt.process_instance_id != envelope.process_instance_id
                    || receipt.story_id != envelope.story_id
                {
                    return settlement_failure(
                        envelope,
                        false,
                        "saved result identity does not match this command".into(),
                    );
                }
                return self.settle_saved_result(envelope, &envelope.command_id, &receipt.result);
            }
            Ok(None) => {}
            Err(error) => {
                return settlement_failure(
                    envelope,
                    false,
                    format!("could not check for a saved command result: {error}"),
                )
            }
        }
        if envelope.command_type == "forge.publish_candidate"
            && forge_lineage_error(&evidence, "qa").is_none()
        {
            if let Some(candidate) = evidence.candidate_sha.as_deref() {
                match self.operations.reconcile_publish(candidate) {
                    PublishReconciliation::Published(outcome @ (PublishOutcome::Published { .. }
                    | PublishOutcome::IntegratedAndPublished { .. })) => {
                        let (patch, detail, succeeded) = publish_patch(outcome.clone());
                        return self.record_and_settle(
                            envelope,
                            &envelope.command_id,
                            &outcome,
                            patch,
                            succeeded,
                            detail,
                        );
                    }
                    PublishReconciliation::Published(_) => {
                        return settlement_failure(
                            envelope,
                            false,
                            "destination readback returned a non-success publish result".into(),
                        )
                    }
                    PublishReconciliation::NotPublished => {}
                    PublishReconciliation::Unknown(reason) => {
                        return settlement_failure(
                            envelope,
                            false,
                            format!("publish outcome cannot be established; no publish was attempted: {reason}"),
                        )
                    }
                }
            }
        }
        match envelope.command_type.as_str() {
            "forge.migrate_dev" => self.migrate(&envelope, &evidence, "dev", false),
            "forge.verify_dev_migration" => self.migrate(&envelope, &evidence, "dev", true),
            "forge.migrate_prod" => self.migrate(&envelope, &evidence, "prod", false),
            "forge.verify_prod_migration" => self.migrate(&envelope, &evidence, "prod", true),
            "forge.refresh_derived_models" => self.derived(&envelope, &evidence, false),
            "forge.verify_derived_models" => self.derived(&envelope, &evidence, true),
            "forge.publish_candidate" => self.publish(&envelope, &evidence),
            other => ApplicationCommandResult {
                command_id: envelope.command_id.clone(),
                outcome: ApplicationCommandOutcome::PreconditionFailure,
                message: Some(format!("unsupported Forge release command {other}")),
            },
        }
    }

    fn record_and_settle<T: Serialize>(
        &self,
        env: &ForgeCommandEnvelope,
        operation_command_id: &str,
        result: &T,
        patch: ForgeGateEvidence,
        operation_succeeded: bool,
        detail: String,
    ) -> ApplicationCommandResult {
        let serialized = match serde_json::to_value(result) {
            Ok(value) => value,
            Err(error) => {
                return settlement_failure(
                    env,
                    operation_succeeded,
                    format!("could not serialize observed operation result: {error}"),
                )
            }
        };
        if let Err(error) =
            self.evidence
                .record_operation_receipt(env, operation_command_id, &serialized)
        {
            return settlement_failure(
                env,
                operation_succeeded,
                format!(
                    "{}; operation result receipt could not be saved: {error}",
                    detail
                ),
            );
        }
        if let Err(error) = self
            .evidence
            .settle_operation(env, operation_command_id, patch)
        {
            return settlement_failure(
                env,
                operation_succeeded,
                format!("{}; evidence settlement failed: {error}", detail),
            );
        }
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(detail),
        }
    }

    fn resume_settlement(
        &self,
        env: &ForgeCommandEnvelope,
        operation_command_id: &str,
        evidence: &ForgeGateEvidence,
    ) -> ApplicationCommandResult {
        match self.evidence.operation_receipt(operation_command_id) {
            Ok(Some(receipt)) => {
                if receipt.command_type != env.command_type
                    || receipt.process_instance_id != env.process_instance_id
                    || receipt.story_id != env.story_id
                {
                    return settlement_failure(
                        env,
                        false,
                        format!("saved result identity does not match operation {operation_command_id}"),
                    );
                }
                self.settle_saved_result(env, operation_command_id, &receipt.result)
            }
            Ok(None) if env.command_type == "forge.publish_candidate" => {
                let Some(candidate) = evidence.candidate_sha.as_deref() else {
                    return settlement_failure(
                        env,
                        false,
                        format!("no saved publish result or candidate SHA for {operation_command_id}"),
                    );
                };
                match self.operations.reconcile_publish(candidate) {
                    PublishReconciliation::Published(outcome @ (PublishOutcome::Published { .. }
                    | PublishOutcome::IntegratedAndPublished { .. })) => {
                        let (patch, detail, succeeded) = publish_patch(outcome.clone());
                        self.record_and_settle(
                            env,
                            operation_command_id,
                            &outcome,
                            patch,
                            succeeded,
                            detail,
                        )
                    }
                    PublishReconciliation::Published(_) => settlement_failure(
                        env,
                        false,
                        "destination readback did not prove a successful publish".into(),
                    ),
                    PublishReconciliation::NotPublished => settlement_failure(
                        env,
                        false,
                        format!("no saved publish result for {operation_command_id}; candidate is not in destination history; no publish was repeated"),
                    ),
                    PublishReconciliation::Unknown(reason) => settlement_failure(
                        env,
                        false,
                        format!(
                            "no saved publish result for {operation_command_id}; destination history could not establish the outcome; no publish was repeated: {reason}"
                        ),
                    ),
                }
            }
            Ok(None) => settlement_failure(
                env,
                false,
                format!(
                    "no saved result for {} and {} has no automatic readback; external action was not repeated",
                    operation_command_id, env.command_type
                ),
            ),
            Err(error) => settlement_failure(
                env,
                false,
                format!("could not read saved result {operation_command_id}: {error}"),
            ),
        }
    }

    fn settle_saved_result(
        &self,
        env: &ForgeCommandEnvelope,
        operation_command_id: &str,
        result: &serde_json::Value,
    ) -> ApplicationCommandResult {
        if env.command_type == "forge.publish_candidate" {
            return match serde_json::from_value::<PublishOutcome>(result.clone()) {
                Ok(outcome) => {
                    let (patch, detail, succeeded) = publish_patch(outcome);
                    match self
                        .evidence
                        .settle_operation(env, operation_command_id, patch)
                    {
                        Ok(()) => ApplicationCommandResult {
                            command_id: env.command_id.clone(),
                            outcome: ApplicationCommandOutcome::Success,
                            message: Some(detail),
                        },
                        Err(error) => settlement_failure(
                            env,
                            succeeded,
                            format!("saved publish result remains unsettled: {error}"),
                        ),
                    }
                }
                Err(error) => settlement_failure(
                    env,
                    false,
                    format!("saved publish result is unreadable: {error}"),
                ),
            };
        }
        let operation = match serde_json::from_value::<ForgeOperationResult>(result.clone()) {
            Ok(operation) => operation,
            Err(error) => {
                return settlement_failure(
                    env,
                    false,
                    format!("saved operation result is unreadable: {error}"),
                )
            }
        };
        let patch = operation_patch(&env.command_type, &operation);
        match self
            .evidence
            .settle_operation(env, operation_command_id, patch)
        {
            Ok(()) => ApplicationCommandResult {
                command_id: env.command_id.clone(),
                outcome: ApplicationCommandOutcome::Success,
                message: Some(operation.detail),
            },
            Err(error) => settlement_failure(
                env,
                operation.success,
                format!("saved operation result remains unsettled: {error}"),
            ),
        }
    }

    fn migrate(
        &self,
        env: &ForgeCommandEnvelope,
        evidence: &ForgeGateEvidence,
        target: &str,
        verify: bool,
    ) -> ApplicationCommandResult {
        let files = evidence.migration_files.clone().unwrap_or_default();
        let result = if verify {
            self.operations.verify_migrations(target, &files)
        } else {
            self.operations
                .apply_migrations(target, &files, &env.command_id)
        };
        let operation_succeeded = result.success;
        let patch = operation_patch(&env.command_type, &result);
        let detail = result.detail.clone();
        self.record_and_settle(
            env,
            &env.command_id,
            &result,
            patch,
            operation_succeeded,
            detail,
        )
    }

    fn derived(
        &self,
        env: &ForgeCommandEnvelope,
        evidence: &ForgeGateEvidence,
        verify: bool,
    ) -> ApplicationCommandResult {
        let models = evidence.derived_models.clone().unwrap_or_default();
        let result = if verify {
            let id = self
                .evidence
                .latest_refresh_command_id(&env.process_instance_id)
                .unwrap_or_default();
            self.operations.verify_derived(&models, &id)
        } else {
            self.operations.refresh_derived(&models, &env.command_id)
        };
        let operation_succeeded = result.success;
        let patch = operation_patch(&env.command_type, &result);
        let detail = result.detail.clone();
        self.record_and_settle(
            env,
            &env.command_id,
            &result,
            patch,
            operation_succeeded,
            detail,
        )
    }

    fn publish(
        &self,
        env: &ForgeCommandEnvelope,
        evidence: &ForgeGateEvidence,
    ) -> ApplicationCommandResult {
        if let Some(err) = forge_lineage_error(evidence, "qa") {
            let mut patch = ForgeGateEvidence::default();
            patch.publish_succeeded = Some(false);
            patch.failure_class = Some("PUBLISH_CONFLICT".into());
            patch.failed_release_stage = Some("PUBLISH".into());
            if let Err(error) = self
                .evidence
                .merge(&env.process_instance_id, &env.story_id, patch)
            {
                return settlement_failure(env, false, format!("{err}; {error}"));
            }
            return ApplicationCommandResult {
                command_id: env.command_id.clone(),
                outcome: ApplicationCommandOutcome::Success,
                message: Some(err),
            };
        }
        let proofs = self.evidence.frozen_proofs(&env.story_id);
        let outcome = self
            .operations
            .publish(evidence.candidate_sha.as_deref(), &proofs);
        let (patch, message, operation_succeeded) = publish_patch(outcome.clone());
        self.record_and_settle(
            env,
            &env.command_id,
            &outcome,
            patch,
            operation_succeeded,
            message,
        )
    }
}

fn operation_patch(command_type: &str, result: &ForgeOperationResult) -> ForgeGateEvidence {
    let mut patch = ForgeGateEvidence::default();
    match command_type {
        "forge.migrate_dev" => patch.dev_migration_applied = Some(result.success),
        "forge.verify_dev_migration" => patch.dev_migration_verified = Some(result.success),
        "forge.migrate_prod" => patch.prod_migration_applied = Some(result.success),
        "forge.verify_prod_migration" => patch.prod_migration_verified = Some(result.success),
        "forge.refresh_derived_models" => patch.derived_refresh_succeeded = Some(result.success),
        "forge.verify_derived_models" => patch.derived_refresh_verified = Some(result.success),
        _ => {}
    }
    if !result.success {
        // THE STAGE DECIDES THE CLASS, BY NAME. This used to ask `command_type.contains("migration")`, and the
        // two apply commands are named `forge.migrate_dev` / `forge.migrate_prod` — "migrate", not "migration" —
        // so a failed *migration* was classified ENVIRONMENT and its stage reported as DERIVED_REFRESH
        // (TST-FORGE-RELEASE-001). A reader would blame the environment and refresh derived models on a migration
        // that never applied. One table, both answers, so the class and the stage cannot drift apart.
        let (stage, class) =
            release_stage_of(command_type).unwrap_or(("DERIVED_REFRESH", "ENVIRONMENT"));
        patch.failure_class = Some(class.into());
        patch.failed_release_stage = Some(stage.into());
        patch.last_failure = Some(result.detail.clone());
    }
    patch
}

/// The release stage a command drives, and the failure class a failed run of it carries.
///
/// Keyed on the commands the release workflow actually issues, not on a substring of their names.
fn release_stage_of(command_type: &str) -> Option<(&'static str, &'static str)> {
    match command_type {
        "forge.migrate_dev" | "forge.verify_dev_migration" => Some(("DEV_MIGRATION", "MIGRATION")),
        "forge.migrate_prod" | "forge.verify_prod_migration" => {
            Some(("PROD_MIGRATION", "MIGRATION"))
        }
        "forge.refresh_derived_models" | "forge.verify_derived_models" => {
            Some(("DERIVED_REFRESH", "ENVIRONMENT"))
        }
        _ => None,
    }
}

fn publish_patch(outcome: PublishOutcome) -> (ForgeGateEvidence, String, bool) {
    let mut patch = ForgeGateEvidence::default();
    let succeeded = matches!(
        &outcome,
        PublishOutcome::Published { .. } | PublishOutcome::IntegratedAndPublished { .. }
    );
    let message = match outcome {
        PublishOutcome::Published {
            published_main_hash,
        }
        | PublishOutcome::IntegratedAndPublished {
            published_main_hash,
        } => {
            patch.publish_succeeded = Some(true);
            patch.published_sha = Some(published_main_hash.clone());
            format!("published {published_main_hash}")
        }
        PublishOutcome::CandidateSecret { reason } => {
            patch.publish_succeeded = Some(false);
            patch.failure_class = Some("HOLD".into());
            patch.failed_release_stage = Some("PUBLISH".into());
            patch.last_failure = Some(reason.clone());
            reason
        }
        PublishOutcome::PublishDisabled { reason } => {
            patch.publish_succeeded = Some(false);
            patch.failure_class = Some("PUBLISH_DISABLED".into());
            patch.failed_release_stage = Some("PUBLISH".into());
            patch.last_failure = Some(reason.clone());
            reason
        }
        other => {
            let reason = match other {
                PublishOutcome::NoCandidate { reason }
                | PublishOutcome::IntegrationConflict { reason }
                | PublishOutcome::PublishConflict { reason } => reason,
                PublishOutcome::IntegrationUnverified {
                    integrated_commit,
                    reason,
                } => format!(
                    "integration produced {integrated_commit} but it did not verify: {reason}"
                ),
                _ => unreachable!(),
            };
            patch.publish_succeeded = Some(false);
            patch.failure_class = Some("PUBLISH_CONFLICT".into());
            patch.failed_release_stage = Some("PUBLISH".into());
            patch.last_failure = Some(reason.clone());
            reason
        }
    };
    (patch, message, succeeded)
}

fn settlement_failure(
    env: &ForgeCommandEnvelope,
    operation_succeeded: bool,
    detail: String,
) -> ApplicationCommandResult {
    let operation_command_id = env.causation_id.as_deref().unwrap_or(&env.command_id);
    ApplicationCommandResult {
        command_id: env.command_id.clone(),
        outcome: ApplicationCommandOutcome::SettlementRequired,
        message: Some(format!(
            "release evidence settlement failed; external_operation_succeeded={operation_succeeded}; command_id={}; operation_command_id={operation_command_id}; process_instance_id={}; story_id={}; {detail}",
            env.command_id, env.process_instance_id, env.story_id
        )),
    }
}

pub struct ClosedReleaseOps;

impl ForgeReleaseOperations for ClosedReleaseOps {
    fn apply_migrations(&self, _t: &str, _f: &[String], _c: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "release operations not wired".into(),
        }
    }
    fn verify_migrations(&self, _t: &str, _f: &[String]) -> ForgeOperationResult {
        self.apply_migrations(_t, _f, "")
    }
    fn refresh_derived(&self, _m: &[String], _c: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: "release operations not wired".into(),
        }
    }
    fn verify_derived(&self, _m: &[String], _a: &str) -> ForgeOperationResult {
        self.refresh_derived(_m, _a)
    }
    fn publish(&self, _c: Option<&str>, _p: &[String]) -> PublishOutcome {
        PublishOutcome::PublishConflict {
            reason: "release operations not wired".into(),
        }
    }
}

#[cfg(test)]
mod tests;
