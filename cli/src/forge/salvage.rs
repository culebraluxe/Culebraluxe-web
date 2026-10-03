//! `forge salvage` — put a Smith candidate's code back out of the control plane.
//!
//! WHY THIS EXISTS. A lane's candidate is one commit on a throwaway branch inside a temp worktree, and every
//! other record of it is a *pointer* into git: `storyboard_story_run.candidate_sha`,
//! `forge_workflow_evidence.candidate_sha`, the `agent/<story>/run-<hex>` branch name. The failures this engine
//! has actually suffered are failures of exactly that pointer — the `run-<hex>` provisioning collision that
//! ended runs at exit 2, a worktree reaped under `/var/folders`, a push no credential would accept. Since
//! 2026-09-30 the Smith lane writes the diff itself into `forge_tool_artifact` (`kind='candidate-code'`,
//! migration 130, written by `engine::runner::smith_candidate_artifact`) as the candidate commit is stamped.
//! This is the way back out.
//!
//! READ ONLY, and it names the database it read — the same rule as `forge sql`, for the same reason: a
//! candidate read from the wrong control plane is a candidate nobody can trust. It writes a file only when
//! asked (`--out`), and it never touches a worktree, a branch or a remote, because a recovery tool that
//! quietly rewrites the repository is a second incident.
//!
//! stdout carries the patch and nothing else, so it composes:
//!   cargo run -p cli -- forge salvage --story TST-RUNTIME-POOL-003 --target prod | git apply --check -
//!   cargo run -p cli -- forge salvage --story TST-RUNTIME-POOL-003 --target prod --out /tmp/candidate.patch

use super::Failure;
use db::{Database, DbTarget, ForgeEngineDao};
use serde_json::json;

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader every other read uses.
    crate::apple_sync::load_env();

    let target = match super::sql::flag(args, "--target").as_deref() {
        Some("dev") | Some("development") => DbTarget::Dev,
        Some("prod") | Some("production") => DbTarget::Prod,
        Some(other) => {
            return Err(Failure::usage(format!(
                "unknown --target `{other}`; expected dev or prod"
            )))
        }
        // No default, exactly as `forge sql` refuses one: the capture lives in one control plane, and a
        // recovery run against the wrong one reads nothing and looks like the work was never there.
        None => {
            return Err(Failure::usage(
                "forge salvage needs an explicit --target dev|prod: the captured candidate lives in one control \
                 plane, and a recovery read that does not name it is a read someone will act on by mistake"
                    .to_string(),
            ))
        }
    };
    let story = match super::sql::flag(args, "--story") {
        Some(story) if !story.trim().is_empty() => story.trim().to_string(),
        _ => {
            return Err(Failure::usage(
                "forge salvage needs --story <id>: this command recovers one story's candidate, and there is no \
                 \"all stories\" form of it"
                    .to_string(),
            ))
        }
    };

    let database = Database::connect_target(target)
        .await
        .map_err(|error| Failure::failed(format!("cannot connect: {error}")))?;
    eprintln!("target={target:?}");
    let detail = ForgeEngineDao::new(database)
        .candidate_code_for_story(&story)
        .await
        .map_err(|error| Failure::failed(format!("candidate read failed: {error}")))?;

    // "Nothing was captured" is a *failure*, not an empty success: the operator asked to recover work, and
    // being told "no rows" without being told why is how a recovery run turns into a false alarm that the
    // story was never built. The two causes are named because they need different responses.
    let Some(detail) = detail else {
        return Err(Failure::failed(missing_candidate_message(&story, target)));
    };

    // Owned, not borrowed out of `detail`: the JSON branch moves the whole value into its output.
    let patch = detail
        .get("patch")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let sha = detail
        .get("candidateSha")
        .and_then(|value| value.as_str())
        .unwrap_or("(unnamed)")
        .to_string();
    let base = detail
        .get("base")
        .and_then(|value| value.as_str())
        .unwrap_or("(unnamed)")
        .to_string();
    let files = detail
        .get("changedFiles")
        .and_then(|value| value.as_array())
        .map(|files| files.len())
        .unwrap_or(0);
    // The metadata goes to stderr so the patch on stdout stays pipeable without a `tail -n +2`.
    eprintln!(
        "story={story} candidate={sha} base={base} files={files} patch_bytes={}",
        patch.len()
    );

    if super::sql::flag(args, "--format").as_deref() == Some("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "target": format!("{target:?}").to_lowercase(),
                "story": story,
                "candidate": sha,
                "base": base,
                "detail": detail,
            }))
            .map_err(|error| Failure::failed(format!("cannot render JSON: {error}")))?
        );
        return Ok(0);
    }

    match super::sql::flag(args, "--out") {
        Some(path) => {
            std::fs::write(&path, patch.as_bytes())
                .map_err(|error| Failure::failed(format!("cannot write --out {path}: {error}")))?;
            eprintln!("wrote {path}");
            // Said here rather than left to be guessed at: `--check` first is how you find out the patch does
            // not apply to the tree you have *before* it has touched it.
            eprintln!("replay it with: git apply --check {path} && git apply {path}");
        }
        None => {
            print!("{patch}");
            if !patch.ends_with('\n') {
                println!();
            }
        }
    }
    Ok(0)
}

/// What an operator is told when the control plane holds no capture for the story they asked about.
///
/// Its own function because in that case the sentence *is* the product of the command: "no lane has run yet"
/// and "it ran before the capture existed" need different responses from whoever typed it — wait, or go and
/// look in git — and an empty result that does not distinguish them teaches nothing.
fn missing_candidate_message(story: &str, target: DbTarget) -> String {
    format!(
        "{story} has no captured candidate on {target:?}: no `candidate-code` artifact was written. Either no \
         Smith lane has run against this story in this control plane, or the lane ran before the capture \
         existed. The candidate is in git if it exists anywhere — this command has nothing to replay."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this test exists for: a recovery read that finds nothing must not look like a clean run. It has
    /// to be a failure (exit 1), and it has to name the story and the control plane it actually read — a
    /// candidate recovered from the wrong database is worse than none.
    #[test]
    fn a_story_with_no_capture_is_refused_with_both_causes_named() {
        let message = missing_candidate_message("TST-RUNTIME-POOL-003", DbTarget::Prod);
        assert!(message.contains("TST-RUNTIME-POOL-003"), "{message}");
        assert!(message.contains("Prod"), "{message}");
        assert!(message.contains("no Smith lane has run"), "{message}");
        assert!(message.contains("before the capture existed"), "{message}");
        assert_eq!(
            Failure::failed(message).exit_code(),
            1,
            "nothing recovered is a failure, not an empty success"
        );
    }

    /// The refusal is per-environment, so the name of the environment has to reach the sentence: this is the
    /// same reason `--target` has no default.
    #[test]
    fn the_message_names_the_control_plane_that_was_actually_read() {
        let dev = missing_candidate_message("TST-X", DbTarget::Dev);
        let prod = missing_candidate_message("TST-X", DbTarget::Prod);
        assert!(dev.contains("Dev"), "{dev}");
        assert!(prod.contains("Prod"), "{prod}");
        assert_ne!(dev, prod);
    }
}
