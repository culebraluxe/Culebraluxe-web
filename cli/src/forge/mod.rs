//! The Forge harness gates, in Rust.
//!
//! These check the HARNESS — packets, skill packs, the memory file, the generated artefacts and the
//! allowlists — not the application. Both used to be TypeScript (`scripts/forge-packet-lint.ts`,
//! `scripts/forge-sync-agents.ts`) importing `lib/agent-vendor-block.ts`, `lib/scope-manifest.ts`,
//! `lib/forge-decision.ts` and `lib/secret-shapes.ts`, all deleted with the TypeScript application in
//! `4cf98110`: every run exited `ERR_MODULE_NOT_FOUND`, and `scripts/vercel-build-prod.sh` called the
//! lint as a non-fatal release step, so a gate that could never pass was wired into production.
//!
//! The captain's ruling is that this tool is useful and stays: it is the checker half of a pair, and
//! `forge sync-agents` is the writer half, so a finding always has a fix command. No Node runtime is
//! left in this repository, so the pair is Rust now — same rule names, same output shape, same exit
//! contract.
//!
//! Usage:
//!   cargo run -p cli -- forge harness-lint [--strict] [--format json]
//!   cargo run -p cli -- forge sync-agents [--check] [--format json]

pub mod citations;
pub mod dead_commands;
pub mod decision;
pub mod doctor;
pub mod guard_paths;
pub mod lint;
pub mod manifest;
pub mod opencode_config;
pub mod opencode_skills;
pub mod protected_files;
pub mod read_tools;
#[cfg(test)]
pub mod repo_guards;
pub mod reset;
pub mod roi;
pub mod salvage;
pub mod secret_shapes;
pub mod sql;
pub mod sync_agents;
pub mod test_section;
pub mod ts_sweep;
pub mod vendor_block;

use std::fmt;
use std::path::PathBuf;
use std::process::Command;

use db::{Database, DbFailure};

/// A gate failure. Exit codes are part of the contract: 0 clean or reported-not-blocking, 1 refused or
/// drift, 2 configuration or usage.
#[derive(Debug)]
pub struct Failure {
    message: String,
    code: u8,
}

impl Failure {
    /// The gate ran and refused the work (drift, a missing handbook, a bad flag combination).
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }

    /// The gate could not run as asked: wrong arguments.
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    /// The environment cannot support the run at all — no database declared, no connection URL. The same
    /// distinction `db-tool` draws: a clean run and an unstartable one must never look alike.
    pub fn configuration(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    pub fn exit_code(&self) -> u8 {
        self.code
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for Failure {}

pub async fn dispatch(args: &[String]) -> Result<u8, Failure> {
    match args.first().map(String::as_str).unwrap_or_default() {
        "harness-lint" => lint::run(&args[1..]),
        // The handbook's `guard:` paths, checked against the tree they claim to describe. Separate from
        // `harness-lint` because it BLOCKS: a dead guard path is a rule with no enforcement, and that is
        // not something to report-and-continue.
        "guard-lint" => guard_paths::run(&args[1..]),
        "sync-agents" => sync_agents::run(&args[1..]),
        // The generated repo-root OpenCode config: one renderer shared with the runtime, so the file a human
        // reads with `opencode` and the document Forge injects through `OPENCODE_CONFIG_CONTENT` agree.
        "opencode-config" => opencode_config::run(&args[1..]),
        // The generated vendor skill tree: `.opencode/skills/<id>/SKILL.md` is rendered from the canonical
        // library in `docs/agent/skills/`. It is the writer half of the pair whose checker is this same command
        // in `--check`, and it is what makes a `skill` grant in the config loadable at all — the vendor reads
        // skills from the filesystem, and a `docs/`-only skill was measured to answer `Unable to load skill`.
        "opencode-skills" => opencode_skills::run(&args[1..]),
        // The dead-TypeScript ledger, two gates in one family: the loader sweep answers which FILES
        // cannot load, the menu sweep which COMMANDS cannot run. Both are pure file checks that touch
        // no database, like the harness gates above — they replaced `scripts/broken-ts-sweep.mjs` and
        // `scripts/dead-command-sweep.mjs` when the last Node runtime left this repository.
        "ts-sweep" => ts_sweep::run(&args[1..]),
        "dead-commands" => dead_commands::run(&args[1..]),
        // The scope manifest: the writer half of the pair the packet lint checks (rule 8 parses these rows).
        // Sync, like the other two harness gates: it reads the tree and git, and touches no database.
        "manifest" => manifest::run(&args[1..]),
        "protected-files" => protected_files::run(&args[1..]),
        "test-section" => test_section::run(&args[1..]),
        // The operator reads. Async because they answer from the live control plane; the two harness gates
        // above are pure file checks and stay synchronous.
        "board" | "story-show" | "story:show" | "batch-status" => read_tools::run(args).await,
        "doctor" => doctor::run(args).await,
        // The session rollup: read-only, one query, and it names the database it read (a cost figure
        // without its environment is a figure someone will quote about the wrong one).
        "roi" => roi::run(args).await,
        // The writer. Its guard rails (PROD only, --force, a positive stale window) are checked before
        // anything touches the database.
        "reset" | "recover" | "clean" => reset::run(&forward_reset_args(args)).await,
        // The read path for a question nobody has a tool for yet: one read-only query, against a database the
        // caller must name. It exists so an audit is a command instead of a throwaway script.
        "sql" => sql::run(&args[1..]).await,
        // The fail-safe's way back out: Smith's candidate code, as it was written into `forge_tool_artifact`
        // when the commit was stamped. Read-only, and it names the database it read for the same reason `sql`
        // does — a candidate recovered from the wrong control plane is worse than none.
        "salvage" => salvage::run(&args[1..]).await,
        other => Err(Failure::usage(format!(
            "unknown forge command `{other}`; usage: forge <harness-lint|guard-lint|sync-agents|manifest|protected-files|test-section|board|story-show|batch-status|doctor|roi|sql|salvage|reset|recover|clean> [options]"
        ))),
    }
}

/// Normalize a `reset` / `recover` / `clean` invocation into the shape [`reset::resolve_reset_config`] documents.
///
/// The CLI leads with the mode (`forge recover <story-id> --force`), but the resolver reads the mode from the
/// SECOND positional — story first, mode second — and falls back to `Reset` when that slot is absent. Forwarding
/// the command token untouched therefore put the mode in the story slot and the story in the mode slot, so
/// `forge reset <story-id>` and `forge recover <story-id>` both died with `unknown mode "<story-id>"`, and
/// `recover` could not be reached at all: the one remedy for a stuck `reserved` engine claim was unreachable,
/// which is why a story blocked by a stale claim stayed blocked across sessions. Only `forge clean` worked, and
/// only because the resolver special-cases a leading `clean`.
///
/// Appending rather than inserting the mode token keeps every flag parse intact: `--force` is recognized
/// position-independently and `--stale-minutes` is stripped together with its value, so the appended token lands
/// in the mode slot either way.
fn forward_reset_args(args: &[String]) -> Vec<String> {
    let Some((mode, rest)) = args.split_first() else {
        return Vec::new();
    };
    let mut forwarded = rest.to_vec();
    forwarded.push(mode.clone());
    forwarded
}

/// The repository root these gates scan. `git rev-parse --show-toplevel` first, because a gate must
/// check the same tree the commit will land in even when it is run from a subdirectory; the working
/// directory is the fallback the TypeScript used, and it is a fallback rather than the rule.
pub fn repo_root() -> PathBuf {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// The process pool for every Forge command that answers from the control plane, resolved from `APP_ENV` /
/// `VERCEL_ENV`. One copy for four callers, because the message is the load-bearing part.
///
/// `DbFailure`'s `Display` is `"{kind:?} during {operation} (incident …)"` — the `detail` field, the sentence
/// that says whether a URL was missing or a password was rejected, is not in it. Four copies of this function
/// printed "cannot resolve or reach" for both cases, so a missing `.env.local` and a suspended database looked
/// identical to the reader. The detail is printed here.
pub async fn connect() -> Result<Database, Failure> {
    Database::connect_from_env()
        .await
        .map_err(|error| Failure::configuration(connect_failure_message(&error)))
}

/// Split out of `connect` so the message can be asserted on without a database.
fn connect_failure_message(error: &DbFailure) -> String {
    format!(
        "cannot reach the control-plane database: {error}\n  \
         {}\n  \
         the URL comes from DATABASE_URL_DEV / DATABASE_URL_PROD in this repository's .env.local (loaded from the \
         repository root, not the working directory); the environment comes from APP_ENV / VERCEL_ENV",
        error
            .detail
            .as_deref()
            .unwrap_or("the driver reported no further detail")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this test exists for: a real failure reported without its reason. `DatabaseUnavailable during
    /// db.connect` was the entire output of a missing `.env.local`, and it sent a reader to the Neon console
    /// looking for an outage.
    #[test]
    fn a_connect_failure_prints_the_reason_and_where_the_url_comes_from() {
        let error = DbFailure::configuration(
            "db.connect",
            "DATABASE_URL_DEV is not configured; Rust DB refuses to fall back to another environment",
        );

        let message = connect_failure_message(&error);

        assert!(
            message.contains("DATABASE_URL_DEV is not configured"),
            "{message}"
        );
        assert!(message.contains(".env.local"), "{message}");
        assert_eq!(Failure::configuration(message).exit_code(), 2);
    }

    /// Even a driver error with nothing to add still names the file the URL is supposed to come from, so the
    /// reader has somewhere to look.
    #[test]
    fn a_connect_failure_without_a_detail_still_names_the_env_file() {
        let mut error = DbFailure::configuration("db.connect", "placeholder");
        error.detail = None;

        let message = connect_failure_message(&error);

        assert!(message.contains("no further detail"), "{message}");
        assert!(message.contains("DATABASE_URL_DEV"), "{message}");
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    /// Resolve the way `dispatch` does, so the assertion covers the forwarding and the resolver together.
    fn resolve(list: &[&str]) -> reset::ResetConfig {
        reset::resolve_reset_config(&forward_reset_args(&args(list)), Some("prod"))
            .expect("a forced PROD invocation is exactly what this tool is for")
    }

    /// The bug this test exists for: `forge recover <story-id>` was unreachable, and the recover path is the CLI's
    /// only remedy for a stuck `reserved` engine claim. With the token forwarded in the story slot, `recover`
    /// resolved to a RESET of a story literally named "recover".
    #[test]
    fn a_leading_mode_token_reaches_the_resolver_in_its_mode_slot() {
        let config = resolve(&["recover", "TST-WF-COMMAND-006", "--force"]);
        assert_eq!(config.story, "TST-WF-COMMAND-006");
        assert_eq!(config.mode, reset::ResetMode::Recover);
    }

    /// The modes that already worked must keep working, including `clean`'s valued flag.
    #[test]
    fn a_reset_and_a_clean_still_resolve_to_their_own_modes() {
        let reset_config = resolve(&["reset", "TST-WF-COMMAND-006", "--force"]);
        assert_eq!(reset_config.story, "TST-WF-COMMAND-006");
        assert_eq!(reset_config.mode, reset::ResetMode::Reset);

        let clean = resolve(&["clean", "--stale-minutes", "30", "--force"]);
        assert!(clean.story.is_empty(), "clean is not story-scoped");
        assert_eq!(clean.mode, reset::ResetMode::Clean);
        assert_eq!(clean.stale_minutes, 30);
    }
}
