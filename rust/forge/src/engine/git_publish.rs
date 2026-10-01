//! Fast-forward publish check matching `previewAcceptedCandidatePublish`.
//! Push only when `FORGE_ALLOW_PUBLISH=1`.

use std::path::Path;
use std::process::Command;

use crate::engine::evidence_store::evidence_patch;
use crate::engine::facts::{evidence_from_value, ForgeGateEvidence};
use crate::engine::release::{
    EvidenceStore, ForgeOperationResult, ForgeReleaseOperations, PublishOutcome,
};
use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn preview_publish(repo: &Path, candidate: &str) -> PublishOutcome {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return PublishOutcome::NoCandidate {
            reason: "no candidate commit recorded".into(),
        };
    }
    if git(
        repo,
        &["cat-file", "-e", &format!("{candidate}^{{commit}}")],
    )
    .is_err()
    {
        return PublishOutcome::NoCandidate {
            reason: format!("candidate {candidate} is not a commit in {repo:?}"),
        };
    }
    // Multiple Smiths may finish from the same base. Publication is therefore an optimistic CAS on origin/main:
    // fast-forward when possible; otherwise make a merge commit whose parents are the latest main and the exact
    // QA-approved candidate. A racing publisher simply refreshes main and retries the integration.
    let mut last_push_error = String::new();
    for _attempt in 0..4 {
        if let Err(error) = git(repo, &["fetch", "origin", "main"]) {
            return PublishOutcome::PublishConflict {
                reason: format!("cannot refresh origin/main before publish: {error}"),
            };
        }
        let remote_main = match git(repo, &["rev-parse", "--verify", "origin/main^{commit}"]) {
            Ok(sha) => sha,
            Err(error) => {
                return PublishOutcome::PublishConflict {
                    reason: format!("origin/main is unreadable: {error}"),
                }
            }
        };

        if remote_main == candidate {
            return PublishOutcome::Published {
                published_main_hash: candidate.into(),
            };
        }

        // Another publisher may already have integrated this candidate.
        if git(
            repo,
            &["merge-base", "--is-ancestor", candidate, &remote_main],
        )
        .is_ok()
        {
            return PublishOutcome::IntegratedAndPublished {
                published_main_hash: remote_main,
            };
        }

        let (publish_sha, integrated) = if git(
            repo,
            &["merge-base", "--is-ancestor", &remote_main, candidate],
        )
        .is_ok()
        {
            (candidate.to_string(), false)
        } else {
            let merged_tree = match git(
                repo,
                &["merge-tree", "--write-tree", &remote_main, candidate],
            ) {
                Ok(output) => output.lines().next().unwrap_or("").trim().to_string(),
                Err(error) => {
                    return PublishOutcome::IntegrationConflict {
                        reason: format!(
                            "candidate {candidate} does not merge cleanly with origin/main {remote_main}: {error}"
                        ),
                    }
                }
            };
            if merged_tree.len() != 40
                || !merged_tree.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return PublishOutcome::IntegrationConflict {
                    reason: format!(
                        "git merge-tree returned no usable tree for candidate {candidate}: {merged_tree:?}"
                    ),
                };
            }
            let message = format!(
                "forge: integrate candidate {}",
                &candidate[..candidate.len().min(12)]
            );
            let commit = match git(
                repo,
                &[
                    "commit-tree",
                    &merged_tree,
                    "-p",
                    &remote_main,
                    "-p",
                    candidate,
                    "-m",
                    &message,
                ],
            ) {
                Ok(commit) => commit,
                Err(error) => {
                    return PublishOutcome::IntegrationConflict {
                        reason: format!("could not create integration commit: {error}"),
                    }
                }
            };
            (commit, true)
        };

        if std::env::var("FORGE_ALLOW_PUBLISH").ok().as_deref() != Some("1") {
            return PublishOutcome::PublishConflict {
                reason: "Forge publication is disabled (FORGE_ALLOW_PUBLISH != 1)".into(),
            };
        }
        match git(
            repo,
            &["push", "origin", &format!("{publish_sha}:refs/heads/main")],
        ) {
            Ok(_) => {
                return if integrated {
                    PublishOutcome::IntegratedAndPublished {
                        published_main_hash: publish_sha,
                    }
                } else {
                    PublishOutcome::Published {
                        published_main_hash: publish_sha,
                    }
                };
            }
            Err(error) => {
                // A sibling may have won main between fetch and push. Refresh/reintegrate up to the bounded retry
                // count; a persistent auth/network/protection error comes back after the same bounded attempts.
                last_push_error = error;
            }
        }
    }

    PublishOutcome::PublishConflict {
        reason: format!(
            "origin/main moved or refused the candidate after 4 publish attempts: {last_push_error}"
        ),
    }
}

pub struct DbReleaseEvidenceStore;

impl EvidenceStore for DbReleaseEvidenceStore {
    fn read(&self, story_id: &str) -> ForgeGateEvidence {
        crate::engine::db_writer::read_story_evidence(story_id)
    }

    fn merge(&self, process_instance_id: &str, story_id: &str, patch: ForgeGateEvidence) {
        let mapped = evidence_patch(&patch);
        let resolved = patch.publish_succeeded == Some(true);
        let result = with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.merge_workflow_evidence(
                    process_instance_id,
                    story_id,
                    &mapped,
                    resolved,
                )
                .await
                .map_err(|error| error.to_string())
            })
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) | Err(error) => {
                eprintln!(
                    "forge release evidence merge failed story={story_id} process={process_instance_id}: {error}"
                );
            }
        }
    }

    fn latest_refresh_command_id(&self, _process_instance_id: &str) -> Option<String> {
        None
    }

    fn frozen_proofs(&self, _story_id: &str) -> Vec<String> {
        Vec::new()
    }
}

pub struct HostReleaseExecutor {
    pub ops: GitReleaseOps,
}

impl crate::engine::writer::ForgeReleaseExecutor for HostReleaseExecutor {
    fn execute(
        &self,
        command_type: &str,
        input: &workflow::Value,
    ) -> workflow::ApplicationCommandResult {
        let story_id = input
            .get("storyId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let process_instance_id = input
            .get("processInstanceId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let command_id = input
            .get("commandId")
            .and_then(|v| v.as_str())
            .unwrap_or(command_type)
            .to_string();
        crate::engine::release::DbForgeReleaseExecutor {
            operations: GitReleaseOps {
                repo_root: self.ops.repo_root.clone(),
            },
            evidence: DbReleaseEvidenceStore,
            pending: Some(evidence_from_value(input)),
        }
        .execute(&crate::engine::release::ForgeCommandEnvelope {
            command_type: command_type.into(),
            command_id,
            process_instance_id,
            story_id,
        })
    }
}

pub struct GitReleaseOps {
    pub repo_root: std::path::PathBuf,
}

impl ForgeReleaseOperations for GitReleaseOps {
    fn apply_migrations(
        &self,
        target: &str,
        files: &[String],
        command_id: &str,
    ) -> ForgeOperationResult {
        if files.is_empty() {
            return ForgeOperationResult {
                success: false,
                detail: "migrationRequired=true but migrationFiles is empty".into(),
            };
        }
        ForgeOperationResult {
            success: false,
            detail: format!(
                "apply {files:?} to {target} via existing Neon schema_migration ledger (command {command_id}); run through ForgeDB pool in the TS operations host or enable postgres feature"
            ),
        }
    }
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!(
                "verify {files:?} on {target} against forge_migration_execution + schema_migration"
            ),
        }
    }
    fn refresh_derived(&self, models: &[String], command_id: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!("refresh {models:?} command {command_id}"),
        }
    }
    fn verify_derived(&self, models: &[String], attempt: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!("verify derived {models:?} attempt {attempt}"),
        }
    }
    fn publish(&self, candidate_sha: Option<&str>, frozen_proofs: &[String]) -> PublishOutcome {
        let Some(sha) = candidate_sha else {
            return PublishOutcome::NoCandidate {
                reason: "no candidate commit recorded".into(),
            };
        };
        for cmd in frozen_proofs {
            let out = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.repo_root)
                .output();
            match out {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    return PublishOutcome::IntegrationUnverified {
                        integrated_commit: sha.into(),
                        reason: format!(
                            "frozen proof `{cmd}` exit {}",
                            o.status.code().unwrap_or(-1)
                        ),
                    };
                }
                Err(e) => {
                    return PublishOutcome::IntegrationUnverified {
                        integrated_commit: sha.into(),
                        reason: e.to_string(),
                    };
                }
            }
        }
        preview_publish(&self.repo_root, sha)
    }
}
