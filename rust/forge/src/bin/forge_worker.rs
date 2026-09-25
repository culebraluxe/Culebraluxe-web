//! Scheduled production worker. Replaces scripts/agent-work-entry.ts.
fn main() {
    if std::env::var("APP_ENV").unwrap_or_else(|_| "development".into()) != "production" {
        eprintln!("forge-worker must target production; set APP_ENV=production");
        std::process::exit(2);
    }
    match forge::engine::worker::run_worker_pass() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
