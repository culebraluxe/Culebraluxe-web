//! Fast-forward publish check matching `previewAcceptedCandidatePublish`.
//!
//! `FORGE_ALLOW_PUBLISH` is a KILL SWITCH, not an opt-in key: unset (or `1`) publishes, and only an explicit
//! `0`/`false`/`off`/`no` refuses. Read as `== Some("1")` it refused the candidate of every run launched
//! outside the scheduler's env file — TST-ACCOUNTING-CORE-008 passed QA at 2026-10-01T16:46Z, was refused
//! here, and stayed Held with a 453-line candidate that never reached `origin/main`. The refusal was also
//! filed as a git conflict (`PUBLISH_CONFLICT`), so it read like "remote main advanced" and nobody looked.
//!
//! THIS PATH IS THE ONLY DOOR. House Rule 1 refuses a push of any branch but `main` (`.githooks/pre-push`),
//! so a candidate cannot leave the machine as an `agent/*` branch: either this function publishes it, or the
//! code strands. A refusal here is therefore always loud and always its own outcome — never a quiet conflict.

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

/// Publish with no integration proofs: a fast-forward lands, and an integration commit is refused as unverified.
pub fn preview_publish(repo: &Path, candidate: &str) -> PublishOutcome {
    publish_candidate(repo, candidate, &[])
}

/// Publish `candidate` to `origin/main`.
///
/// A fast-forward pushes the exact commit QA approved. When `origin/main` has moved, the publish builds an
/// integration commit (latest main + the candidate) that NOBODY has built or tested — two candidates can each pass QA
/// and still break main together — so that commit is proven first: every command in `proofs` (the story's QA
/// commands) is run in a disposable checkout of it, and it is pushed only if all of them pass. No proofs, or a proof
/// that fails, is `IntegrationUnverified`: the story holds with the reason, and main does not move.
pub fn publish_candidate(repo: &Path, candidate: &str, proofs: &[String]) -> PublishOutcome {
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
            if merged_tree.len() != 40 || !merged_tree.bytes().all(|byte| byte.is_ascii_hexdigit())
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

        if integrated {
            if let Err(reason) = prove_integration(repo, &publish_sha, proofs) {
                return PublishOutcome::IntegrationUnverified {
                    integrated_commit: publish_sha,
                    reason,
                };
            }
        }
        if publish_switch_off(std::env::var("FORGE_ALLOW_PUBLISH").ok().as_deref()) {
            return PublishOutcome::PublishDisabled {
                reason: "publication disabled by FORGE_ALLOW_PUBLISH".into(),
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

/// Run every proof against `commit` in a disposable checkout of it. `Ok` only when there was something to run and all
/// of it passed.
fn prove_integration(repo: &Path, commit: &str, proofs: &[String]) -> Result<(), String> {
    if proofs.is_empty() {
        return Err(format!(
            "origin/main moved after QA, so publishing needs integration commit {commit}, and there is no QA command \
             to prove it with; refusing to publish an untested merge (rebase the candidate onto main and run QA again)"
        ));
    }
    crate::engine::worktree::with_detached_checkout(repo, commit, |dir| {
        for proof in proofs {
            let status = Command::new("sh")
                .arg("-c")
                .arg(proof)
                .current_dir(dir)
                .status()
                .map_err(|error| format!("proof `{proof}` could not start: {error}"))?;
            if !status.success() {
                return Err(format!(
                    "proof `{proof}` failed on integration commit {commit} (exit {})",
                    status.code().unwrap_or(-1)
                ));
            }
        }
        Ok(())
    })?
}

/// Whether the publish kill switch is held open, as a pure predicate over the raw value.
///
/// The switch used to be read as `== Some("1")`, which made an ABSENT variable mean "do not publish" — so
/// every run launched without the scheduler's `.env.scheduler` (an attended `pnpm forge:engine`, a direct
/// `--bin forge` run, any checkout that is not the scheduler's) silently refused its own candidate. Only
/// words turn the switch off now, and the words are named here so the rule has one home.
///
/// Pure, and deliberately so: `std::env::set_var` is process-global and unsafe to drive from a test that runs
/// beside others, and this is the decision those tests need to pin.
pub fn publish_switch_off(value: Option<&str>) -> bool {
    // Case-insensitive on purpose: `FORGE_ALLOW_PUBLISH=OFF` is a person saying off, and a comparison that
    // missed it would publish anyway — the same class of silent surprise this function exists to end.
    match value.map(str::trim) {
        Some(word) => {
            word.eq_ignore_ascii_case("0")
                || word.eq_ignore_ascii_case("false")
                || word.eq_ignore_ascii_case("off")
                || word.eq_ignore_ascii_case("no")
        }
        None => false,
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
                dao.merge_workflow_evidence(process_instance_id, story_id, &mapped, resolved)
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
                integration_proofs: self.ops.integration_proofs.clone(),
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
    /// The story's QA commands: what an integration commit must pass before it is pushed (`publish_candidate`).
    pub integration_proofs: Vec<String>,
}

impl ForgeReleaseOperations for GitReleaseOps {
    // MIGRATIONS AND DERIVED REFRESHES ARE A HUMAN'S, BY DESIGN — NOT A MISSING PORT. A migration on PROD is a
    // production action, and AGENTS.md makes every production action the Captain's explicit go; Forge does not apply
    // one unattended. Each of these therefore refuses with the instruction a person needs to finish the release,
    // and the workflow holds the story on that stage (`failure_class` MIGRATION / ENVIRONMENT). These used to say
    // "run through ForgeDB pool in the TS operations host", a host that no longer exists — a refusal that read like
    // a bug to fix rather than a step to take.
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
                "HUMAN STEP: Forge does not apply migrations unattended. Apply {files:?} to {target} \
                 (`pnpm db:migrations` shows per-target state), then resume this hold (command {command_id})."
            ),
        }
    }
    fn verify_migrations(&self, target: &str, files: &[String]) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!(
                "HUMAN STEP: confirm {files:?} are recorded in {target}'s schema_migration ledger \
                 (`pnpm db:migrations`), then resume this hold."
            ),
        }
    }
    fn refresh_derived(&self, models: &[String], command_id: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!(
                "HUMAN STEP: Forge does not refresh derived models unattended. Refresh {models:?}, then resume \
                 this hold (command {command_id})."
            ),
        }
    }
    fn verify_derived(&self, models: &[String], attempt: &str) -> ForgeOperationResult {
        ForgeOperationResult {
            success: false,
            detail: format!(
                "HUMAN STEP: confirm the refresh of {models:?} landed (attempt {attempt}), then resume this hold."
            ),
        }
    }
    fn publish(&self, candidate_sha: Option<&str>, frozen_proofs: &[String]) -> PublishOutcome {
        let Some(sha) = candidate_sha else {
            return PublishOutcome::NoCandidate {
                reason: "no candidate commit recorded".into(),
            };
        };
        // The proofs run against the commit being published, in a checkout of it — never against whatever the
        // engine's working tree happens to hold, which is where they used to run.
        let proofs: Vec<String> = frozen_proofs
            .iter()
            .chain(self.integration_proofs.iter())
            .cloned()
            .collect();
        publish_candidate(&self.repo_root, sha, &proofs)
    }
}
