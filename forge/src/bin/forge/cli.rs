//! The forge command line: how a launch is read, refused and defaulted, and the subcommands that answer
//! without an engine. Moved out of `main.rs` on 2026-10-14 (a 1400-line bin root) — move-only apart from
//! `pub(super)` on what the run half and the CLI contract tests read, and `use super::*;` for the parent's
//! imports. `parse_forge_stop_after`'s import travels with it: it was only ever read here. The old `main`
//! came too, renamed `entry`: it is the front end of the parse, and the crate root keeps exactly one `main`.
//!
//! `main.rs` keeps the entry point and `run`: the composition root, where the release handle is built and the
//! publish switch is read, and the one place the story queue and the role-job queue meet.

use forge::engine::executor::parse_forge_stop_after;

use super::*;

pub(super) fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

/// A claim whose launch configuration the engine refuses: terminalize it now, as `Error`.
///
/// Leaving it `Claimed` would hand the row to stale recovery ten minutes later, which reports a dead worker rather
/// than a configuration that was never runnable. This is `reject_agent_work_configuration`'s first caller — the DAO
/// that could not run until 2026-09-29, because it wrote the illegal state `Failed`.
pub(super) fn reject_configuration(work_item: Option<&str>, reason: &str) {
    let Some(item) = work_item else {
        return;
    };
    match agent_work::reject_agent_work_configuration(item, reason) {
        Ok(()) => eprintln!("work_item={item} state=Error ({reason})"),
        Err(error) => eprintln!("work_item={item} could not be rejected: {error}"),
    }
}

/// The claim this process is executing under, read from the launch (migration 278).
///
/// `--work-item` says WHICH claim; `--claim-owner` and `--claim-generation` say WHOSE. The worker passes all three,
/// because it is the process that took the claim. The pair is required together: a run that names a work item but
/// not the authority it executes under cannot be fenced, and an unfenceable run is exactly what lets a superseded
/// execution begin, beat and settle a claim its successor now owns.
///
/// A missing generation is the pre-278 authority (0), which is the authority of an item claimed by a worker that
/// does not know about generations — that is what makes a mixed-version drain safe (`FORGE-B1` §10.8).
pub(super) fn claim_fence_from_args(
    args: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    work_item: &str,
) -> Result<db::ClaimFence, String> {
    let owner = flag(args, "--claim-owner")
        .or_else(|| env("FORGE_CLAIM_OWNER"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let Some(owner) = owner else {
        return Err(format!(
            "--work-item {work_item} without --claim-owner cannot be fenced (migration 278).\n\
             A claim's authority is (owner, generation) and only the process that took the claim has it.\n\
             Read the item row's claimed_by and claim_generation columns — `pnpm forge:doctor` prints the owner,\n\
             and psql against the control-plane database prints both —\n\
             then pass --claim-owner <claimed_by> --claim-generation <claim_generation>.\n\
             To run a story with no queue claim at all, drop --work-item."
        ));
    };
    let generation = match flag(args, "--claim-generation")
        .or_else(|| env("FORGE_CLAIM_GENERATION"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(raw) => raw
            .parse::<i64>()
            .map_err(|error| format!("invalid --claim-generation {raw}: {error}"))?,
        None => 0,
    };
    Ok(db::ClaimFence::new(owner, generation))
}

/// The run's own terminal write, on the way out of `main`: the item **and its story**, decided together in the
/// database (`db::settlement_pair`).
///
/// The result is returned, not only printed. A terminal write that fails has to change the exit status: the worker
/// settles a child only when it exits non-zero, and stale recovery then requeues whatever is left — which is how a
/// claim whose work had already landed could be handed to a second run. A verdict this process failed to write must
/// not look like a verdict it wrote.
///
/// The `Option` is the pair the DATABASE holds after the call, not a claim that this call wrote it: the name of the
/// answer (`settled`, `duplicate`, `released`, `conflict`, `refused_ownership`, `not_found`) is printed with it,
/// and only `settled` means the row moved here.
pub(super) fn settle_work_item(
    work_item: Option<&str>,
    fence: Option<&db::ClaimFence>,
    outcome: AgentWorkOutcome,
    reason: Option<&str>,
) -> Result<Option<AgentWorkSettlement>, String> {
    let (Some(item), Some(fence)) = (work_item, fence) else {
        return Ok(None);
    };
    match agent_work::finish_agent_work_run(item, fence, outcome, reason) {
        Ok(answer) => {
            // The answer is printed by NAME. The old code printed one message for four different facts, and
            // "already had a verdict" covered "this claim is not yours" — the sentence that let a superseded run
            // read as a settled one.
            let pair = answer.settlement().cloned();
            match pair.as_ref() {
                Some(pair) => eprintln!(
                    "work_item={item} {}: item={} story={}",
                    answer.name(),
                    pair.item_state,
                    pair.story_status.as_deref().unwrap_or("unchanged")
                ),
                None => eprintln!("work_item={item} {}", answer.name()),
            }
            if let Some(refusal) = pair.as_ref().and_then(|pair| pair.reason.as_deref()) {
                eprintln!("work_item={item} {refusal}");
            }
            Ok(pair)
        }
        Err(error) => {
            eprintln!("work_item={item} could not be settled: {error}");
            Err(error)
        }
    }
}

/// A refusal to launch: the message that goes to stderr, and the status that goes with it.
///
/// Parsing used to print and `exit(2)` from five different places inside `main`, so "the flags were read" and
/// "the process ends" were the same statement and the exit codes were only discoverable by reading `main` top to
/// bottom. A refusal is a value now: parsing returns it, `main` maps it, and a test can assert on it.
#[derive(Debug)]
pub(super) struct CliRefusal {
    pub(super) message: String,
    pub(super) exit_code: i32,
}

impl CliRefusal {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }
}

/// The work types a launch may declare. A type that is not here is refused, never widened to the default.
pub(super) const ALLOWED_WORK_TYPES: &[&str] =
    &["FEATURE", "FAST", "BUG", "HOTFIX", "RESEARCH", "MIGRATION"];

/// The launch configuration of one `forge` run: which story, under which claim, and how far.
///
/// Read once, here. Every field takes the flag first and the environment variable it mirrors as the fallback — the
/// precedence the hand-rolled `flag()`/`env::var()` pairs had, in one place instead of spread through `main`. What
/// the LAUNCH said lives in here; what the MACHINE says (the worktree, the base ref, the store, the budgets) stays
/// in the environment and is read where it is used.
#[derive(Debug)]
pub(super) struct Cli {
    /// The story to drive (`--story` / `FORGE_STORY_ID`). Required.
    pub(super) story: String,
    /// The chain to run (`--work-type` / `FORGE_WORK_TYPE`). Defaults to `FEATURE`.
    pub(super) work_type: String,
    /// The queue claim this process owns (`--work-item` / `FORGE_WORK_ITEM_ID`), when the worker launched it.
    pub(super) work_item: Option<String>,
    /// The authority that claim is held under (`--claim-owner`/`--claim-generation`), required with `work_item`.
    pub(super) claim_fence: Option<db::ClaimFence>,
    /// How far the chain may go (`--stop-after` / `FORGE_STOP_AFTER`), when the dispatch capped it.
    pub(super) stop_after: Option<ForgeStopTarget>,
}

impl Cli {
    /// Parse this process's launch, reading the environment for the fallbacks.
    pub(super) fn from_args(args: &[String]) -> Result<Self, CliRefusal> {
        Self::from_args_with(args, &|name| env::var(name).ok())
    }

    /// Parse a launch against an explicit environment reader.
    ///
    /// The reader is a parameter so the CLI contract test can state both halves of the precedence — the flag wins,
    /// the variable fills in what the flag omitted — without mutating the process environment that every other test
    /// in this binary shares.
    pub(super) fn from_args_with(
        args: &[String],
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, CliRefusal> {
        // The claim this run was launched against, if any. A claimed run **owns** its queue row: it opens it as
        // `Running` before the first role turn and settles it on the way out. An operator invocation without the
        // flag stays legal and owns nothing — but nothing that dispatches may launch without it (`engine::worker`
        // claims first and passes this id).
        let work_item = flag(args, "--work-item")
            .or_else(|| env("FORGE_WORK_ITEM_ID"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        // The authority this run executes under. Fail fast, before any claim is moved or any model turn is paid
        // for: a run that cannot be fenced must not start (migration 278, and `FORGE-B1` §10.4).
        let claim_fence = match work_item.as_deref() {
            Some(item) => Some(
                claim_fence_from_args(args, env, item)
                    .map_err(|refusal| CliRefusal::new(format!("forge: {refusal}")))?,
            ),
            None => None,
        };
        let story = flag(args, "--story")
            .or_else(|| env("FORGE_STORY_ID"))
            .unwrap_or_default();
        if story.trim().is_empty() {
            reject_configuration(work_item.as_deref(), "no --story");
            return Err(CliRefusal::new(
                "usage: forge --story <id> [--work-type FEATURE|FAST|BUG|HOTFIX|RESEARCH|MIGRATION] [--work-item <uuid>]",
            ));
        }
        let work_type = flag(args, "--work-type")
            .or_else(|| env("FORGE_WORK_TYPE"))
            .unwrap_or_else(|| "FEATURE".into());
        if !ALLOWED_WORK_TYPES.contains(&work_type.as_str()) {
            reject_configuration(
                work_item.as_deref(),
                &format!("invalid --work-type {work_type}"),
            );
            return Err(CliRefusal::new(format!("invalid --work-type {work_type}")));
        }
        // The dispatch cap the worker carried off the claimed row (migration 167). An unrecognised value is
        // refused rather than widened: a cap that cannot be read must not become "run the whole chain".
        let stop_after = match flag(args, "--stop-after")
            .or_else(|| env("FORGE_STOP_AFTER"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            None => None,
            Some(raw) => match parse_forge_stop_after(&raw) {
                Some(target) => Some(target),
                None => {
                    reject_configuration(
                        work_item.as_deref(),
                        &format!("invalid --stop-after {raw}"),
                    );
                    return Err(CliRefusal::new(format!(
                        "invalid --stop-after {raw}: expected scout|architect|lead"
                    )));
                }
            },
        };
        Ok(Self {
            story,
            work_type,
            work_item,
            claim_fence,
            stop_after,
        })
    }
}

/// The subcommands that print and stop: they name no story and drive nothing, so they are answered before a launch
/// is parsed. `Some(status)` means this process is done, and that is the status to exit with.
pub(super) fn subcommand(args: &[String]) -> Option<i32> {
    match args.get(1).map(|value| value.as_str())? {
        // `forge learn --dry-run`: what the learn pass would do, without touching the control plane.
        "learn" if args.iter().any(|value| value == "--dry-run") => Some(learn_dry_run()),
        // `forge profile-compare [limit]`: print what the recorded turns say each harness profile costs per
        // ACCEPTED candidate, read from the run trajectory the engine writes (293). Read-only and
        // environment-declared, because it reports measurement rather than announcing a verdict; the limit keeps
        // the read bounded (default 2000 turns).
        "profile-compare" => {
            let limit = args
                .get(2)
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(2000);
            Some(match forge::engine::profile_report::run(limit) {
                Ok(report) => {
                    println!("{report}");
                    0
                }
                Err(error) => {
                    eprintln!("forge profile-compare: {error}");
                    1
                }
            })
        }
        // `forge harness-info`: print the resolved vendor identity and exit — the proof line for the A/B/C
        // acceptance criteria without parsing a turn log.
        "harness-info" => Some(print_harness_info()),
        _ => None,
    }
}

/// `forge learn --dry-run`: the learn report, or the reason there is none.
pub(super) fn learn_dry_run() -> i32 {
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("forge learn --dry-run: cannot resolve current directory: {error}");
            return 1;
        }
    };
    match forge::engine::run_learn_dry_run(&root) {
        Ok(report) => {
            println!("{report}");
            0
        }
        Err(error) => {
            eprintln!("forge learn --dry-run: {error}");
            1
        }
    }
}

/// `forge harness-info`: the vendor this process would drive, said out loud. Exit 2 when the vendor cannot be
/// resolved, because a lane that cannot name its vendor has nothing to report.
pub(super) fn print_harness_info() -> i32 {
    match forge::engine::harness::HarnessBackend::from_env() {
        Ok(harness) => {
            eprintln!("{}", harness_identity_line(harness));
            0
        }
        Err(e) => {
            eprintln!("harness=error {e}");
            2
        }
    }
}

/// The `harness=` line `harness-info` prints: which vendor, which model or agent, which binary, from where.
pub(super) fn harness_identity_line(harness: forge::engine::harness::HarnessBackend) -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into());
    match harness {
        forge::engine::harness::HarnessBackend::OpenCode => {
            let bin = forge::engine::opencode::default_cli_bin();
            let model = forge::engine::opencode::resolve_opencode_model(None)
                .unwrap_or_else(|e| format!("error: {e}"));
            format!("harness=opencode model={model} bin={bin} cwd={cwd}")
        }
        forge::engine::harness::HarnessBackend::Maestro => {
            let bin = forge::engine::maestro::MaestroHarness::default_cli_bin()
                .unwrap_or_else(|e| format!("error: {e}"));
            let selection = forge::engine::harness::ModelSelection::from_parts(
                std::env::var("FORGE_MODEL_POLICY").ok().as_deref(),
                None,
            );
            let agent = forge::engine::maestro::resolve_maestro_agent(
                &selection,
                std::env::var("MAESTRO_AGENT").ok().as_deref(),
                std::env::var("MAESTRO_CHEAP_AGENT").ok().as_deref(),
                std::env::var("MAESTRO_JUDGMENT_AGENT").ok().as_deref(),
                std::env::var("MAESTRO_DEFAULT_AGENT").ok().as_deref(),
            )
            .unwrap_or_else(|e| format!("error: {e}"));
            format!("harness=maestro agent={agent} bin={bin} cwd={cwd}")
        }
        forge::engine::harness::HarnessBackend::Dsh => {
            let cli = forge::engine::dsh::default_cli()
                .map(|cli| cli.display())
                .unwrap_or_else(|e| format!("error: {e}"));
            let model = forge::engine::dsh::resolve_model(None)
                .ok()
                .flatten()
                .unwrap_or_else(|| "profile-default".into());
            format!("harness=dsh model={model} bin={cli} cwd={cwd}")
        }
    }
}

pub(super) fn entry() {
    // First, before the parse: a refused launch still reports why through the engine's events, and an event with no
    // subscriber behind it is an event nobody reads.
    forge::observability::init();
    let args: Vec<String> = env::args().collect();
    if let Some(status) = subcommand(&args) {
        std::process::exit(status);
    }
    // The engine's own database budgets, and the rule behind them: `forge::engine::db_budget`. Both binaries that
    // talk to the control plane install these, because the process that was dying was the worker. Before the parse,
    // deliberately: a refused launch still settles the claim it was refused for.
    let budget = forge::engine::db_budget::install_engine_db_budget();
    let cli = match Cli::from_args(&args) {
        Ok(cli) => cli,
        Err(refusal) => {
            eprintln!("{}", refusal.message);
            std::process::exit(refusal.exit_code);
        }
    };
    run(cli, budget);
}
