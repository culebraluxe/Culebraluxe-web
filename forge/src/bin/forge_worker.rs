//! Scheduled production worker. Replaces scripts/agent-work-entry.ts.
//!
//! ONE RULE, ONE PLACE. This binary used to carry its own `APP_ENV != production` check while the engine child
//! carried `execution_target`'s — the same fact with two authors, so a change to the rule could land in one and
//! not the other. `db::resolve_forge_target` is the single authority now (PROD, or refuse), and the line it
//! prints says which database the run is actually on, because a run that does not name its target is a run a
//! reader has to guess about.
fn main() {
    match db::resolve_forge_target(
        std::env::var("VERCEL_ENV").ok().as_deref(),
        std::env::var("APP_ENV").ok().as_deref(),
    ) {
        Ok(target) => eprintln!("forge-worker target={}", target.as_str()),
        Err(error) => {
            eprintln!("forge-worker: {error}");
            std::process::exit(2);
        }
    }
    // The worker is the process that died on `db.connect` with nothing claimed (2026-09-29), so it installs the
    // engine's database budgets exactly as the runner does — see `forge::engine::db_budget`.
    let budget = forge::engine::db_budget::install_engine_db_budget();
    eprintln!(
        "statement_ceiling_ms={} connect_budget_ms={}",
        budget.statement_timeout_ms, budget.connect_timeout_ms
    );
    let forge = forge::ForgeService::new();
    match forge.run_scheduled_pass() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
