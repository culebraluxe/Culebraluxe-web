//! Fast-forward publish check matching `previewAcceptedCandidatePublish`.
//! Push only when `FORGE_ALLOW_PUBLISH=1`.

use std::path::Path;
use std::process::Command;

use crate::engine::release::{ForgeOperationResult, ForgeReleaseOperations, PublishOutcome};

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
    if git(repo, &["cat-file", "-e", &format!("{candidate}^{{commit}}")]).is_err() {
        return PublishOutcome::NoCandidate {
            reason: format!("candidate {candidate} is not a commit in {repo:?}"),
        };
    }
    let remote = git(repo, &["ls-remote", "origin", "refs/heads/main"])
        .unwrap_or_default();
    let remote_main = remote.split_whitespace().next().unwrap_or("").to_string();
    if remote_main.is_empty() {
        return PublishOutcome::PublishConflict {
            reason: "origin/main is unreadable (offline or no remote)".into(),
        };
    }
    if remote_main == candidate {
        return PublishOutcome::Published {
            published_main_hash: candidate.into(),
        };
    }
    if git(repo, &["merge-base", "--is-ancestor", &remote_main, candidate]).is_err() {
        return PublishOutcome::PublishConflict {
            reason: format!(
                "origin/main ({}) is not an ancestor of candidate {} — push would NOT fast-forward",
                &remote_main[..remote_main.len().min(12)],
                &candidate[..candidate.len().min(12)]
            ),
        };
    }
    if std::env::var("FORGE_ALLOW_PUBLISH").ok().as_deref() == Some("1") {
        match git(repo, &["push", "origin", &format!("{candidate}:refs/heads/main")]) {
            Ok(_) => PublishOutcome::Published {
                published_main_hash: candidate.into(),
            },
            Err(e) => PublishOutcome::PublishConflict {
                reason: e,
            },
        }
    } else {
        PublishOutcome::Published {
            published_main_hash: remote_main,
        }
    }
}

pub struct EmptyEvidence;

impl crate::engine::release::EvidenceStore for EmptyEvidence {
    fn read(&self, _story_id: &str) -> crate::engine::facts::ForgeGateEvidence { Default::default() }
    fn merge(&self, _i: &str, _s: &str, _p: crate::engine::facts::ForgeGateEvidence) {}
    fn latest_refresh_command_id(&self, _i: &str) -> Option<String> { None }
    fn frozen_proofs(&self, _s: &str) -> Vec<String> { vec![] }
}

pub struct HostReleaseExecutor {
    pub ops: GitReleaseOps,
}

impl crate::engine::writer::ForgeReleaseExecutor for HostReleaseExecutor {
    fn execute(&self, command_type: &str, input: &workflow::Value) -> workflow::ApplicationCommandResult {
        let story_id = input.get("storyId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let process_instance_id = input.get("processInstanceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let command_id = input.get("commandId").and_then(|v| v.as_str()).unwrap_or(command_type).to_string();
        crate::engine::release::DbForgeReleaseExecutor {
            operations: GitReleaseOps {
                repo_root: self.ops.repo_root.clone(),
            },
            evidence: EmptyEvidence,
            pending: None,
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
    fn apply_migrations(&self, target: &str, files: &[String], command_id: &str) -> ForgeOperationResult {
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
            detail: format!("verify {files:?} on {target} against forge_migration_execution + schema_migration"),
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
