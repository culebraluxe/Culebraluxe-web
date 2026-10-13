//! Maestro tests, grouped by the subject each block exercises.

use super::*;

/// Environment-mutating tests serialize here: Rust runs tests on threads sharing one process
/// environment, so two tests setting `FORGE_HARNESS` at once would read each other's values.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_fixture_script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("fixture script");
    writeln!(file, "#!/bin/sh\n{body}").expect("write fixture script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make fixture executable");
    path
}

fn run_fixture_with_outer_deadline(
    path: std::path::PathBuf,
    turn_ceiling: Duration,
    pid_file: Option<std::path::PathBuf>,
) -> Result<MaestroRunResult> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = run_maestro_streaming(
            path.to_str().expect("fixture path is UTF-8"),
            ".",
            &[],
            None,
            None,
            Some(turn_ceiling),
        );
        let _ = tx.send(result);
    });
    match rx.recv_timeout(turn_ceiling + Duration::from_secs(3)) {
        Ok(result) => result,
        Err(_) => {
            if let Some(pid_file) = pid_file {
                if let Ok(pid) = std::fs::read_to_string(pid_file)
                    .and_then(|pid| pid.trim().parse::<u32>().map_err(std::io::Error::other))
                {
                    let _ = crate::engine::opencode_client::RunningTurn { pid }.terminate();
                }
            }
            panic!("Maestro fixture exceeded its outer test deadline");
        }
    }
}

use crate::engine::harness::{HarnessBackend, ModelSelection};

mod args;
mod harness;
mod output;
mod session;
mod stream;

/// The whole Maestro module as text, for the structural guards: after the module was split into
/// files, a guard that read one file would have quietly stopped covering the rest.
pub(crate) fn maestro_module_source() -> String {
    [
        include_str!("../mod.rs"),
        include_str!("../args.rs"),
        include_str!("../harness.rs"),
        include_str!("../output.rs"),
        include_str!("../preflight.rs"),
        include_str!("../session.rs"),
        include_str!("../stream.rs"),
    ]
    .concat()
}
