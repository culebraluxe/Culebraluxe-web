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
}

#[derive(Debug, Clone)]
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

pub trait EvidenceStore: Send + Sync {
    fn read(&self, story_id: &str) -> ForgeGateEvidence;
    fn merge(
        &self,
        process_instance_id: &str,
        story_id: &str,
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
            self.operations
                .apply_migrations(target, &files, &env.command_id)
        };
        let operation_succeeded = result.success;
        let stage = if target == "dev" {
            "DEV_MIGRATION"
        } else {
            "PROD_MIGRATION"
        };
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
        if let Err(error) = self
            .evidence
            .merge(&env.process_instance_id, &env.story_id, patch)
        {
            return settlement_failure(
                env,
                operation_succeeded,
                format!("{}; {error}", result.detail),
            );
        }
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
        let operation_succeeded = result.success;
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
        if let Err(error) = self
            .evidence
            .merge(&env.process_instance_id, &env.story_id, patch)
        {
            return settlement_failure(
                env,
                operation_succeeded,
                format!("{}; {error}", result.detail),
            );
        }
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(result.detail),
        }
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
        let operation_succeeded = matches!(
            &outcome,
            PublishOutcome::Published { .. } | PublishOutcome::IntegratedAndPublished { .. }
        );
        let mut patch = ForgeGateEvidence::default();
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
            // The switch, not git. Filed under its own name so an operator greps the cause instead of reading
            // "remote main advanced" and going looking for a merge conflict that does not exist.
            PublishOutcome::PublishDisabled { reason } => {
                patch.publish_succeeded = Some(false);
                patch.failure_class = Some("PUBLISH_DISABLED".into());
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
                    } => format!(
                        "integration produced {integrated_commit} but it did not verify: {reason}"
                    ),
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
        if let Err(error) = self
            .evidence
            .merge(&env.process_instance_id, &env.story_id, patch)
        {
            return settlement_failure(env, operation_succeeded, format!("{message}; {error}"));
        }
        ApplicationCommandResult {
            command_id: env.command_id.clone(),
            outcome: ApplicationCommandOutcome::Success,
            message: Some(message),
        }
    }
}

fn settlement_failure(
    env: &ForgeCommandEnvelope,
    operation_succeeded: bool,
    detail: String,
) -> ApplicationCommandResult {
    ApplicationCommandResult {
        command_id: env.command_id.clone(),
        outcome: ApplicationCommandOutcome::PreconditionFailure,
        message: Some(format!(
            "release evidence settlement failed; external_operation_succeeded={operation_succeeded}; command_id={}; process_instance_id={}; story_id={}; {detail}",
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
mod tests {
    use super::*;
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
    }

    struct FakeEvidence {
        fail: bool,
        current: ForgeGateEvidence,
        patches: Mutex<Vec<ForgeGateEvidence>>,
    }
    impl EvidenceStore for FakeEvidence {
        fn read(&self, _: &str) -> ForgeGateEvidence {
            self.current.clone()
        }
        fn merge(
            &self,
            _: &str,
            _: &str,
            patch: ForgeGateEvidence,
        ) -> std::result::Result<(), String> {
            if self.fail {
                return Err("injected DB failure".into());
            }
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
                current: ForgeGateEvidence {
                    candidate_sha: Some("a".repeat(40)),
                    qa_verified_sha: Some("a".repeat(40)),
                    qa_passed: Some(true),
                    ..Default::default()
                },
                patches: Mutex::new(Vec::new()),
            },
            pending: None,
        }
    }

    fn envelope() -> ForgeCommandEnvelope {
        ForgeCommandEnvelope {
            command_type: "forge.publish_candidate".into(),
            command_id: "cmd-original".into(),
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
            ApplicationCommandOutcome::PreconditionFailure
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
                ApplicationCommandOutcome::PreconditionFailure
            );
            let message = command.message.expect("settlement diagnostic");
            assert!(
                message.contains("external_operation_succeeded=true"),
                "{message}"
            );
            assert!(message.contains(&env.command_id), "{message}");
        }
    }
}
