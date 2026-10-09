//! `forge reset` / `forge recover` / `forge clean` — the writer, and the guards around it.
//!
//! Rust replacement for `scripts/forge-story-reset.ts` + `scripts/forge-story-reset-config.ts`, which import
//! `legacy/db/*` (deleted in `4cf98110`) and so exited `ERR_MODULE_NOT_FOUND`. `pnpm forge:clean` is named in
//! the handbook as pre-run hygiene and could not run at all.
//!
//! THE GUARDS ARE THE POINT OF THIS FILE, and they are a pure function so they can be tested:
//!
//! * the database target is NOT a choice here — the pool declares it, and an attempt to choose the environment
//!   is refused with a message that says who owns the decision;
//! * this tool only touches PROD state, so anything that resolves to DEV is refused rather than quietly run;
//! * PROD needs `--force`, because a destructive act should be deliberate;
//! * `--stale-minutes` must be a positive number, and a claim younger than it is treated as LIVE — `clean`
//!   must never cancel a running peer.
//!
//! Usage:
//!   cargo run -p cli -- forge reset <story-id> [--force] [--plan]
//!   cargo run -p cli -- forge recover <story-id> [--force] [--plan]
//!   cargo run -p cli -- forge clean [--stale-minutes N] [--force] [--plan]
//!
//! `--plan` is the dry run: it resolves the invocation exactly as a real one — same PROD-only
//! refusal, same mode and story parsing — prints the scope the run would touch, and exits 0 before
//! anything connects. No DAO call, no mutation. It is itself the explicit request, so it does not
//! need `--force`; a mutating run still requires `--force` exactly as before.

use super::{connect, Failure};
use db::{resolve_declared_target, DbTarget, ForgeResetDao, ResetReport};
use forge::engine::constants::FORGE_DEFAULT_STALE_MINUTES;
use std::env;

/// A claim younger than this is treated as LIVE: `clean` must never cancel a running peer.
pub const DEFAULT_CLEAN_STALE_MINUTES: i64 = FORGE_DEFAULT_STALE_MINUTES;

pub const USAGE: &str = "usage: forge reset <story-id> [--force] [--plan] | forge recover <story-id> [--force] \
[--plan] | forge clean [--stale-minutes N] [--force] [--plan] (the database target is PROD and is decided by \
the pool; PROD requires --force, or --plan for a scope preview that mutates nothing)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Reset,
    Recover,
    Clean,
}

impl ResetMode {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "reset" => Some(Self::Reset),
            "recover" => Some(Self::Recover),
            "clean" => Some(Self::Clean),
            _ => None,
        }
    }

    /// The label the steps print with.
    pub fn label(self) -> &'static str {
        match self {
            Self::Reset => "reset",
            Self::Recover => "recover",
            Self::Clean => "clean",
        }
    }
}

/// A resolved invocation. Reaching this value IS the permission check: every refusal happens in
/// [`resolve_reset_config`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetConfig {
    /// Empty for `clean`, which sweeps every story: hygiene is not story-scoped.
    pub story: String,
    pub mode: ResetMode,
    pub stale_minutes: i64,
    /// `--plan`: print the scope and exit 0 before anything connects — no DAO call, no mutation.
    pub plan: bool,
}

/// Resolve one invocation without touching the process — the tested half.
///
/// `declared_target` is `None` when the environment is undeclared, which is a refusal: a tool that cannot name
/// its database must not guess one.
pub fn resolve_reset_config(
    args: &[String],
    declared_target: Option<&str>,
) -> Result<ResetConfig, String> {
    // `--force` and `--plan` are recognized position-independently and never occupy a positional slot, so
    // they may appear before, between or after the positionals.
    let force = args.iter().any(|arg| arg == "--force");
    let plan = args.iter().any(|arg| arg == "--plan");
    let stale_minutes = read_stale_minutes(args)?;

    let positional = strip_valued_flags(args, "--stale-minutes")
        .into_iter()
        .filter(|arg| arg != "--force" && arg != "--plan")
        .collect::<Vec<_>>();

    // `clean` may lead (hygiene reads naturally as its own verb) or follow a story id. When it leads there is
    // no story, because the sweep is about the control plane's leftovers, not about one story's chain.
    let clean_leads = positional
        .first()
        .map(|value| value.eq_ignore_ascii_case("clean"))
        .unwrap_or(false);
    let story = if clean_leads {
        String::new()
    } else {
        positional.first().cloned().unwrap_or_default()
    };
    if !clean_leads && story.trim().is_empty() {
        return Err(USAGE.to_string());
    }

    let raw_mode = if clean_leads {
        "clean".to_string()
    } else {
        positional
            .get(1)
            .map(|value| value.to_lowercase())
            .unwrap_or_else(|| "reset".to_string())
    };
    let Some(mode) = ResetMode::parse(&raw_mode) else {
        return Err(format!(
            "unknown mode {raw_mode:?} (expected reset|recover|clean)"
        ));
    };

    // Any leftover positional is an attempt to choose the environment. Say so, and say who owns that
    // decision, rather than silently ignoring it.
    let consumed = if clean_leads { 1 } else { 2 };
    if let Some(extra) = positional.get(consumed) {
        return Err(format!(
            "unexpected argument {extra:?}: the database target is not a choice here — the pool declares the \
             environment. {USAGE}"
        ));
    }

    let Some(declared) = declared_target else {
        return Err("the database environment is not declared (set APP_ENV)".to_string());
    };
    if declared != "prod" {
        return Err(format!(
            "refusing to run against {}: this tool only resets PROD state, and the environment is the pool's \
             decision, not the caller's. Declare APP_ENV=production.",
            declared.to_uppercase()
        ));
    }
    // `--plan` is the dry run: it prints the scope and mutates nothing, so it is itself the explicit
    // request and does not need `--force`. A mutating run still requires `--force`, unchanged.
    if !force && !plan {
        return Err(
            "PROD reset/recover/clean requires --force (refusing a destructive act without explicit \
             confirmation; --plan previews the scope and mutates nothing)"
                .to_string(),
        );
    }

    Ok(ResetConfig {
        story,
        mode,
        stale_minutes,
        plan,
    })
}

/// Read `--stale-minutes N`: an error when present but not a positive number, the default when absent.
fn read_stale_minutes(args: &[String]) -> Result<i64, String> {
    let Some(index) = args.iter().position(|arg| arg == "--stale-minutes") else {
        return Ok(DEFAULT_CLEAN_STALE_MINUTES);
    };
    let Some(raw) = args.get(index + 1) else {
        return Err(format!("--stale-minutes needs a positive number. {USAGE}"));
    };
    match raw.parse::<f64>() {
        Ok(value) if value.is_finite() && value > 0.0 => Ok(value as i64),
        _ => Err(format!("--stale-minutes needs a positive number. {USAGE}")),
    }
}

/// Drop a flag together with its value, so the value is never mistaken for a positional.
fn strip_valued_flags(args: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            index += 2;
            continue;
        }
        out.push(args[index].clone());
        index += 1;
    }
    out
}

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD.
    crate::apple_sync::load_env();

    // The declaration is read from the environment exactly as the pool reads it, so this tool and the pool can
    // never disagree about which database a run would touch.
    let declared = resolve_declared_target(
        env::var("VERCEL_ENV").ok().as_deref(),
        env::var("APP_ENV").ok().as_deref(),
    )
    .ok()
    .map(DbTarget::as_str);

    run_with_declared_target(args, declared).await
}

/// The body of [`run`] with the declared target already resolved, so the `--plan` path — which returns
/// before anything connects — can be asserted on without a process environment or a database.
async fn run_with_declared_target(args: &[String], declared: Option<&str>) -> Result<u8, Failure> {
    let config = resolve_reset_config(args, declared).map_err(Failure::usage)?;

    // `--plan` is the dry run: report the scope and exit before anything connects. No DAO call, no
    // mutation — the preview is a pure function of the resolved config.
    if config.plan {
        print_plan(&config);
        return Ok(0);
    }

    let database = connect().await?;
    let dao = ForgeResetDao::new(database);

    let report = match config.mode {
        ResetMode::Reset => dao.reset_story(&config.story).await,
        ResetMode::Recover => dao.recover_story(&config.story).await,
        ResetMode::Clean => dao.clean(config.stale_minutes).await,
    }
    .map_err(|error| {
        Failure::failed(format!(
            "{} failed against PROD: {error}",
            config.mode.label()
        ))
    })?;

    print_report(&config, &report);
    Ok(0)
}

/// `--plan`: print the scope of the run instead of the run. Pure function of the config — no
/// connection, no DAO, no mutation.
fn print_plan(config: &ResetConfig) {
    println!("{}", plan_report(config));
}

/// The scope report `--plan` prints: what the run would touch, in the order the run would touch it.
fn plan_report(config: &ResetConfig) -> String {
    let scope = if config.story.is_empty() {
        "control plane".to_string()
    } else {
        config.story.clone()
    };
    let mut lines = vec![format!(
        "[plan] {} {} (stale window {} min) — nothing will be mutated",
        config.mode.label(),
        scope,
        config.stale_minutes
    )];
    lines.extend(
        plan_steps(config)
            .into_iter()
            .map(|step| format!("  would {step}")),
    );
    lines.join("\n")
}

/// What the run would do, per mode — the same steps [`ForgeResetDao`] performs, as scope lines.
fn plan_steps(config: &ResetConfig) -> Vec<&'static str> {
    match config.mode {
        ResetMode::Reset => vec![
            "abort the story's active engine instances",
            "obsolete their open tasks",
            "cancel the story's open work items",
            "interrupt the engine claims the reset kills",
            "return the story to Planned",
        ],
        ResetMode::Recover => vec![
            "release the story's reserved and in-progress tasks to ready",
            "cancel the story's stale running work items",
        ],
        ResetMode::Clean => vec![
            "cancel stale open work items and hold their stories",
            "interrupt engine claims older than the stale window",
            "abort engine instances older than the stale window",
            "obsolete open tasks under terminal instances",
            "mop up orphaned engine claims",
        ],
    }
}

/// The post-condition is printed every time, because a sweep you cannot read the result of is a sweep you will
/// run twice and trust neither time.
fn print_report(config: &ResetConfig, report: &ResetReport) {
    let scope = if config.story.is_empty() {
        "control plane".to_string()
    } else {
        config.story.clone()
    };
    println!(
        "[{}] {} (stale window {} min)",
        config.mode.label(),
        scope,
        config.stale_minutes
    );
    for (label, count) in &report.steps {
        println!("  {label}: {count}");
    }
    println!(
        "  remaining: instances={} openTasks={} openWorkItems={} activeEngineClaims={}",
        report.remaining.instances,
        report.remaining.open_tasks,
        report.remaining.open_work_items,
        report.remaining.active_engine_claims
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn a_prod_reset_with_force_resolves_to_the_story_and_the_default_stale_window() {
        let config = resolve_reset_config(&args(&["ENG-FORGE-DOCTOR-01", "--force"]), Some("prod"))
            .expect("a forced prod reset is exactly what this tool is for");
        assert_eq!(config.story, "ENG-FORGE-DOCTOR-01");
        assert_eq!(config.mode, ResetMode::Reset);
        assert_eq!(config.stale_minutes, DEFAULT_CLEAN_STALE_MINUTES);
    }

    #[test]
    fn clean_may_lead_and_needs_no_story_because_hygiene_is_not_story_scoped() {
        let config = resolve_reset_config(&args(&["clean", "--force"]), Some("prod"))
            .expect("clean is the pre-test hygiene verb");
        assert!(config.story.is_empty());
        assert_eq!(config.mode, ResetMode::Clean);

        // …and it may also follow the story id, as the retired script allowed.
        let trailing =
            resolve_reset_config(&args(&["S-1", "clean", "--force"]), Some("prod")).unwrap();
        assert_eq!(trailing.mode, ResetMode::Clean);
    }

    #[test]
    fn a_dev_target_is_refused_and_the_refusal_names_the_environment() {
        let error = resolve_reset_config(&args(&["S-1", "--force"]), Some("dev"))
            .expect_err("this tool only resets PROD state");
        assert!(error.contains("refusing to run against DEV"), "{error}");
        assert!(error.contains("not the caller's"), "{error}");
    }

    #[test]
    fn an_undeclared_environment_is_a_refusal_not_a_guess() {
        let error = resolve_reset_config(&args(&["S-1", "--force"]), None).unwrap_err();
        assert!(error.contains("not declared"), "{error}");
    }

    #[test]
    fn prod_without_force_is_refused() {
        let error = resolve_reset_config(&args(&["S-1"]), Some("prod")).unwrap_err();
        assert!(error.contains("requires --force"), "{error}");
    }

    #[test]
    fn an_attempt_to_choose_the_environment_is_named_as_such() {
        let error = resolve_reset_config(&args(&["S-1", "reset", "prod", "--force"]), Some("prod"))
            .unwrap_err();
        assert!(error.contains("not a choice here"), "{error}");
        assert!(
            error.contains("the pool declares the environment"),
            "{error}"
        );
    }

    #[test]
    fn the_stale_window_must_be_a_positive_number_and_a_missing_value_is_not_a_zero() {
        assert_eq!(
            resolve_reset_config(
                &args(&["clean", "--stale-minutes", "5", "--force"]),
                Some("prod")
            )
            .unwrap()
            .stale_minutes,
            5
        );
        assert!(resolve_reset_config(
            &args(&["clean", "--stale-minutes", "0", "--force"]),
            Some("prod")
        )
        .is_err());
        assert!(resolve_reset_config(
            &args(&["clean", "--stale-minutes", "later", "--force"]),
            Some("prod")
        )
        .is_err());
        assert!(resolve_reset_config(
            &args(&["clean", "--stale-minutes", "--force"]),
            Some("prod")
        )
        .is_err());
    }

    #[test]
    fn an_unknown_mode_is_refused_by_name() {
        let error =
            resolve_reset_config(&args(&["S-1", "wipe", "--force"]), Some("prod")).unwrap_err();
        assert!(error.contains("unknown mode \"wipe\""), "{error}");
    }

    #[test]
    fn a_reset_with_no_story_prints_the_usage_rather_than_guessing_one() {
        let error = resolve_reset_config(&args(&["--force"]), Some("prod")).unwrap_err();
        assert_eq!(error, USAGE);
    }

    #[test]
    fn the_stale_minutes_value_is_never_mistaken_for_a_story_id() {
        let config = resolve_reset_config(
            &args(&["clean", "--stale-minutes", "30", "--force"]),
            Some("prod"),
        )
        .unwrap();
        assert!(
            config.story.is_empty(),
            "the value of a flag is not a positional"
        );
        assert_eq!(config.stale_minutes, 30);
    }

    /// FORGE-FIX-015: `--plan` is the dry run. It resolves like a real invocation — same PROD-only
    /// refusal, same mode and story parsing — but it is itself the explicit request, so it does not
    /// need `--force`: a preview mutates nothing.
    #[test]
    fn clean_with_plan_resolves_without_force_over_the_whole_control_plane() {
        let config = resolve_reset_config(&args(&["clean", "--plan"]), Some("prod"))
            .expect("a preview is the explicit request for exactly what it does");
        assert!(config.plan);
        assert_eq!(config.mode, ResetMode::Clean);
        assert!(config.story.is_empty(), "clean is not story-scoped");
        assert_eq!(config.stale_minutes, DEFAULT_CLEAN_STALE_MINUTES);
    }

    /// `--plan` keeps the story scoping of `reset` and `recover`.
    #[test]
    fn a_story_mode_with_plan_resolves_the_story_and_the_plan() {
        let config = resolve_reset_config(
            &args(&["ENG-FORGE-DOCTOR-01", "recover", "--plan"]),
            Some("prod"),
        )
        .expect("recover --plan is the dry run for one story");
        assert!(config.plan);
        assert_eq!(config.mode, ResetMode::Recover);
        assert_eq!(config.story, "ENG-FORGE-DOCTOR-01");
    }

    /// The mutating contract is unchanged: without `--force` and without `--plan`, PROD is refused.
    #[test]
    fn prod_with_neither_force_nor_plan_is_refused() {
        let error = resolve_reset_config(&args(&["clean"]), Some("prod")).unwrap_err();
        assert!(error.contains("requires --force"), "{error}");
    }

    /// `--plan` is a flag, never a story or a mode: it is stripped from the positionals, so the
    /// first positional remains the story exactly as it would be without the flag — and a leftover
    /// positional after story and mode is still refused, flag or not.
    #[test]
    fn a_plan_flag_is_never_mistaken_for_a_story_or_a_mode() {
        let config = resolve_reset_config(&args(&["reset", "--plan"]), Some("prod"))
            .expect("the plan flag is not a positional");
        assert!(config.plan);
        assert_eq!(config.story, "reset");
        assert_eq!(config.mode, ResetMode::Reset);

        let error =
            resolve_reset_config(&args(&["--plan", "clean", "extra"]), Some("prod")).unwrap_err();
        assert!(error.contains("unexpected argument"), "{error}");
    }

    /// The PROD-only refusal holds for the dry run too: this tool only ever touches PROD state.
    #[test]
    fn plan_on_a_dev_target_is_still_refused() {
        let error = resolve_reset_config(&args(&["clean", "--plan"]), Some("dev")).unwrap_err();
        assert!(error.contains("refusing to run against DEV"), "{error}");
    }

    /// `--plan` composes with the valued flags: the stale window is still parsed.
    #[test]
    fn plan_composes_with_the_stale_window() {
        let config = resolve_reset_config(
            &args(&["clean", "--stale-minutes", "30", "--plan"]),
            Some("prod"),
        )
        .expect("plan composes with --stale-minutes");
        assert!(config.plan);
        assert_eq!(config.stale_minutes, 30);
    }

    /// The scope report names the mode, the scope, the stale window, and the no-mutation guarantee.
    #[test]
    fn the_plan_report_names_the_scope_and_the_guarantee() {
        let config = resolve_reset_config(&args(&["clean", "--plan"]), Some("prod")).unwrap();
        let report = plan_report(&config);
        assert!(report.contains("[plan] clean control plane"), "{report}");
        assert!(
            report.contains(&format!("stale window {} min", DEFAULT_CLEAN_STALE_MINUTES)),
            "{report}"
        );
        assert!(report.contains("nothing will be mutated"), "{report}");
        assert!(
            report.contains("would cancel stale open work items"),
            "{report}"
        );
        assert!(
            report.contains("would mop up orphaned engine claims"),
            "{report}"
        );

        let story = resolve_reset_config(
            &args(&["ENG-FORGE-DOCTOR-01", "reset", "--plan"]),
            Some("prod"),
        )
        .unwrap();
        let report = plan_report(&story);
        assert!(
            report.contains("[plan] reset ENG-FORGE-DOCTOR-01"),
            "{report}"
        );
        assert!(
            report.contains("would return the story to Planned"),
            "{report}"
        );
    }

    /// The assay: `forge clean --plan` reports the scope and exits 0 with zero row changes. The plan
    /// path returns before `connect` is ever called, so no connection is possible and no row can
    /// move. `DATABASE_URL_PROD` is removed for the duration so that even a regression that fell
    /// through to the mutating path could not connect — the test would fail on the connection
    /// error instead of passing against a real database. No other test in this binary reads it.
    #[tokio::test]
    async fn clean_with_plan_exits_zero_without_a_connection() {
        let previous = std::env::var("DATABASE_URL_PROD").ok();
        std::env::remove_var("DATABASE_URL_PROD");

        let result = run_with_declared_target(&args(&["clean", "--plan"]), Some("prod")).await;

        match previous {
            Some(url) => std::env::set_var("DATABASE_URL_PROD", &url),
            None => std::env::remove_var("DATABASE_URL_PROD"),
        }
        assert_eq!(
            result.expect("--plan must exit 0 without touching a database"),
            0
        );
    }
}
