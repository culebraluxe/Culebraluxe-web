//! Port of db-release-executor.ts. Operations are injected; Neon SQL is unchanged.

use crate::engine::facts::{forge_lineage_error, ForgeGateEvidence};
use workflow::{ApplicationCommandOutcome, ApplicationCommandResult};

#[derive(Debug, Clone)]
pub struct ForgeCommandEnvelope {
    pub command_type: String,
    pub command_id: String,
    pub process_instance_id: String,
    pub story_id: String,
}

#[derive(Debug, Clone)]
pub struct ForgeOperationResult {
    pub success: bool,
    pub detail: String,
}

pub trait ForgeReleaseOperations: Send + Sync {
    fn apply_migrations(&self, target: &str, files: &[String], command_id: &str) -> ForgeOperationResult;
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult;
    fn refresh_derived(&self, models: &[String], command_id: &str) -> ForgeOperationResult;
    fn verify_derived(&self, models: &[String], attempt_command_id: &str) -> ForgeOperationResult;
    fn publish(
        &self,
        candidate_sha: Option<&str>,
        frozen_proofs: &[String],
    ) -> PublishOutcome;
}

#[derive(Debug, Clone)]
pub enum PublishOutcome {
    Published { published_main_hash: String },
    IntegratedAndPublished { published_main_hash: String },
    NoCandidate { reason: String },
    IntegrationUnverified { integrated_commit: String, reason: String },
    IntegrationConflict { reason: String },
    CandidateSecret { reason: String },
    PublishConflict { reason: String },
}

pub trait EvidenceStore: Send + Sync {
    fn read(&self, story_id: &str) -> ForgeGateEvidence;
    fn merge(&self, process_instance_id: &str, story_id: &str, patch: ForgeGateEvidence);
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
        let stored = self.evidence.read(&envelope.story_id);
        let evidence = match &self.pending {
            Some(p) => p.merge_over(&stored),
            None => stored,
        };
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
            self.operations.apply_migrations(target, &files, &env.command_id)
        };
        let stage = if target == "dev" { "DEV_MIGRATION" } else { "PROD_MIGRATION" };
        let mut patch = ForgeGateEvidence::default();
        match (target, verify) {
            ("dev", true) => patch.dev_migration_verified = Some(result.success),
            ("dev", false) => patch.dev_migration_applied = Some(result.success),
            (_, true) => patch.prod_migration_verified = Some(result.success),
            (_, false) => patch.prod_migration_applied = Some(result.success),
        }
        if !result.success {
            patch.failure_class = Some("MIGRATION".into());
            patch.failed_release_stage = Some(stage.into());
        }
        self.evidence.merge(&env.process_instance_id, &env.story_id, patch);
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(result.detail),
        }
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
        let mut patch = ForgeGateEvidence::default();
        if verify {
            patch.derived_refresh_verified = Some(result.success);
        } else {
            patch.derived_refresh_succeeded = Some(result.success);
        }
        if !result.success {
            patch.failure_class = Some("ENVIRONMENT".into());
            patch.failed_release_stage = Some("DERIVED_REFRESH".into());
        }
        self.evidence.merge(&env.process_instance_id, &env.story_id, patch);
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(result.detail),
        }
    }

    fn publish(&self, env: &ForgeCommandEnvelope, evidence: &ForgeGateEvidence) -> ApplicationCommandResult {
        if let Some(err) = forge_lineage_error(evidence, "qa") {
            let mut patch = ForgeGateEvidence::default();
            patch.publish_succeeded = Some(false);
            patch.failure_class = Some("PUBLISH_CONFLICT".into());
            patch.failed_release_stage = Some("PUBLISH".into());
            self.evidence
                .merge(&env.process_instance_id, &env.story_id, patch);
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
        let mut patch = ForgeGateEvidence::default();
        let message = match outcome {
            PublishOutcome::Published { published_main_hash }
            | PublishOutcome::IntegratedAndPublished { published_main_hash } => {
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
            other => {
                let reason = match other {
                    PublishOutcome::NoCandidate { reason } => reason,
                    PublishOutcome::IntegrationUnverified {
                        integrated_commit,
                        reason,
                    } => format!("integration produced {integrated_commit} but it did not verify: {reason}"),
                    PublishOutcome::IntegrationConflict { reason } => reason,
                    PublishOutcome::PublishConflict { reason } => reason,
                    _ => unreachable!(),
                };
                patch.publish_succeeded = Some(false);
                patch.failure_class = Some("PUBLISH_CONFLICT".into());
                patch.failed_release_stage = Some("PUBLISH".into());
                patch.last_failure = Some(reason.clone());
                reason
            }
        };
        self.evidence
            .merge(&env.process_instance_id, &env.story_id, patch);
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(message),
        }
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
