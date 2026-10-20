//! Forge operator CLI.
//!
//! The Forge gates, read tools and launchd bridge used to be reachable only through the app CLI as
//! `cli forge …` / `cli launchd …`. They are now a separate binary that takes the Forge subcommand
//! directly: `forge-cli harness-lint`, `forge-cli board`, `forge-cli launchd agent-worker …`.
//! Exit codes are unchanged (0 clean/reported-not-blocking, 1 refused or drift, 2 configuration/usage).

mod env;
mod forge;
mod launchd;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    // The launchd bridge still owns its nested `launchd <agent> <action>` shape; everything else is
    // the direct Forge subcommand (no leading `forge`).
    if args.first().map(String::as_str) == Some("launchd") {
        return match launchd::run(&args[1..]) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("launchd: {error}");
                ExitCode::from(error.exit_code())
            }
        };
    }

    match forge::dispatch(&args).await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("forge: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
