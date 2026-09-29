//! Scheduled production worker. Replaces scripts/agent-work-entry.ts.
fn main() {
    if std::env::var("APP_ENV").unwrap_or_else(|_| "development".into()) != "production" {
        eprintln!("forge-worker must target production; set APP_ENV=production");
        std::process::exit(2);
    }
    // The worker is the process that died on `db.connect` with nothing claimed (2026-09-29), so it installs the
    // engine's database budgets exactly as the runner does — see `forge::engine::db_budget`.
    let budget = forge::engine::db_budget::install_engine_db_budget();
    eprintln!(
        "statement_ceiling_ms={} connect_budget_ms={}",
        budget.statement_timeout_ms, budget.connect_timeout_ms
    );
    match forge::engine::worker::run_worker_pass() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
