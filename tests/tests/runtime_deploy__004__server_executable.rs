//! RUNTIME.DEPLOY — server executable (TST-RUNTIME-DEPLOY-004).
//!
//! CONTRACT. The production server binary is built from the `web` crate (the `web` binary target). The binary
//! must:
//!
//!   1. Compile successfully for the host target (not wasm).
//!   2. Require a valid database connection to start (fails fast with clear error if not available).
//!   3. Start and listen on a socket when invoked with valid configuration.
//!   4. Respond to a basic health check or root request.
//!   5. Shut down cleanly on SIGTERM.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The test exercises the actual `web` crate binary through the
//! cargo build and process execution, not a re-implementation.
//!
//! NEGATIVE CASES. A test that only checked compilation could not distinguish a binary that builds but crashes
//! on start. So the test also exercises:
//! - A binary built without the required environment variables (must fail fast with a clear error).
//! - A binary started without a valid database URL (must fail with a clear database connection error).
//! - A binary sent SIGTERM during startup (must shut down cleanly).
//!
//! POSITIVE CASE. With a valid database URL, the server starts and responds to requests. This requires a live
//! database and is tested separately in integration environments.
//!
//! This test compiles the binary and runs it as a subprocess. Level: L3 Composition, no special harness.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test runtime_deploy__004__server_executable

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// How long a case that asserts the server REFUSES to start may take to exit. Bounded and generous, and paid
/// only when the answer is wrong: the fixed 500 ms this replaced both flaked (a slow first connect is not a
/// failure) and hid a hang (the case is about a server that must not keep running).
const REFUSAL_WAIT: Duration = Duration::from_secs(30);

/// How long the SIGTERM case may take to exit.
const SIGTERM_WAIT: Duration = Duration::from_secs(5);

/// Wait for the child to exit, on a bounded poll rather than a fixed sleep.
fn wait_for_exit(
    child: &mut std::process::Child,
    within: Duration,
) -> Option<std::process::ExitStatus> {
    let step = Duration::from_millis(50);
    let mut waited = Duration::ZERO;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if waited >= within {
                    return None;
                }
                thread::sleep(step);
                waited += step;
            }
            Err(e) => panic!("failed to check process status: {e}"),
        }
    }
}

/// A child whose environment is the CASE rather than an inheritance.
///
/// `env_clear()` is the point, not decoration: without it the child inherits whatever the test process has,
/// every lane has `.env.local` sourced, `APP_ENV` arrived from the parent, the server started and served, and
/// the case failed on the harness instead of on the server. Nothing in `web/` or `db/` loads `.env.local`
/// itself (measured: no dotenv in either), so with the child cleared the server decides on the variables this
/// test hands it — which is the condition each case means to exercise.
fn refusable_command(binary: &std::path::Path) -> Command {
    let mut command = Command::new(binary);
    command.env_clear();
    // PATH is not a database setting: keep it, so the binary is started the way a shell starts it.
    if let Ok(path) = std::env::var("PATH") {
        command.env("PATH", path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

/// The refused child's stderr, for the assertion that says what the refusal has to name.
fn stderr_of(child: &mut std::process::Child) -> String {
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    stderr
}

/// Where cargo actually puts artifacts: `CARGO_TARGET_DIR` if the environment names one (it beats the config),
/// otherwise what cargo's own configuration resolves to (`cargo metadata`'s `target_directory`, which honours
/// each lane's `.cargo/config.toml`), otherwise cargo's default `<root>/target`.
///
/// This test used to fall back to `<root>/build/rust`, the shared target dir retired on 2026-10-07: with
/// `CARGO_TARGET_DIR` unset — every lane, whose target dir lives in its own `.cargo/config.toml` — it built a
/// release binary INSIDE the checkout, at a path no `.gitignore` covers, and a `git add -A` running while that
/// build still wrote died on a `.rmeta` under it.
fn cargo_target_dir(root: &std::path::Path) -> String {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        if !dir.trim().is_empty() {
            return dir;
        }
    }
    if let Ok(out) = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
    {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(dir) = json
                    .get("target_directory")
                    .and_then(|value| value.as_str())
                {
                    return dir.to_string();
                }
            }
        }
    }
    root.join("target").to_string_lossy().to_string()
}

#[test]
#[allow(non_snake_case)]
fn runtime_deploy_004__server_executable() {
    let api = "server executable";

    // Build the server binary in release mode (what production uses)
    // CARGO_MANIFEST_DIR is always set for an integration test, so the fallback is compile-time rather than a
    // path into somebody's lane.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
    let root = std::path::Path::new(&manifest_dir).parent().unwrap();

    // WHERE CARGO PUTS ARTIFACTS, asked rather than guessed (TECH-DEBT row 7) — see `cargo_target_dir`.
    let target_dir = cargo_target_dir(root);

    let binary_path = std::path::Path::new(&target_dir)
        .join("release")
        .join("web");

    // If binary doesn't exist, build it
    if !binary_path.exists() {
        let output = Command::new("cargo")
            .args([
                "build",
                "--manifest-path",
                &root.join("Cargo.toml").to_string_lossy(),
                "--bin",
                "web",
                "--release",
                "--target-dir",
                &target_dir,
            ])
            .output()
            .expect("cargo build failed to run");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("{api}: server binary failed to compile:\n{stderr}");
        }
    }

    assert!(
        binary_path.exists(),
        "{api}: server binary must exist at {:?}",
        binary_path
    );

    // ---- NEGATIVE: Missing required environment (APP_ENV) ----
    // The child's environment is set by `refusable_command`, not inherited: that is what makes
    // "APP_ENV intentionally not set" true of the process the server sees.
    let mut child = refusable_command(&binary_path)
        .env("PORT", "8080")
        // APP_ENV intentionally not set
        .spawn()
        .expect("server failed to start");

    match wait_for_exit(&mut child, REFUSAL_WAIT) {
        Some(status) => {
            // Server should exit with error when APP_ENV is not set
            assert!(
                !status.success(),
                "{api}: server must fail when APP_ENV is not set"
            );
            let stderr = stderr_of(&mut child);
            assert!(
                stderr.contains("database target is undeclared") || stderr.contains("APP_ENV"),
                "{api}: error must mention missing environment declaration, got: {stderr}"
            );
        }
        None => {
            // If it's still running, kill it and fail
            child.kill().expect("kill");
            let _ = child.wait();
            panic!("{api}: server should have failed without APP_ENV but is still running");
        }
    }

    // ---- NEGATIVE: Missing database URL (APP_ENV set but no DATABASE_URL) ----
    let mut child2 = refusable_command(&binary_path)
        .env("PORT", "8081")
        .env("APP_ENV", "development")
        // DATABASE_URL_DEV intentionally not set
        .spawn()
        .expect("server failed to start");

    match wait_for_exit(&mut child2, REFUSAL_WAIT) {
        Some(status) => {
            assert!(
                !status.success(),
                "{api}: server must fail when DATABASE_URL_DEV is not set"
            );
            let stderr = stderr_of(&mut child2);
            assert!(
                stderr.contains("DATABASE_URL_DEV")
                    || stderr.contains("database")
                    || stderr.contains("connection"),
                "{api}: error must mention missing database URL, got: {stderr}"
            );
        }
        None => {
            child2.kill().expect("kill");
            let _ = child2.wait();
            panic!(
                "{api}: server should have failed without DATABASE_URL_DEV but is still running"
            );
        }
    }

    // ---- NEGATIVE: Invalid database URL (wrong host) ----
    let mut child3 = refusable_command(&binary_path)
        .env("PORT", "8082")
        .env("APP_ENV", "development")
        .env("DATABASE_URL_DEV", "postgres://invalid-host/invalid")
        .spawn()
        .expect("server failed to start");

    match wait_for_exit(&mut child3, REFUSAL_WAIT) {
        Some(status) => {
            assert!(
                !status.success(),
                "{api}: server must fail with invalid database URL"
            );
            let stderr = stderr_of(&mut child3);
            assert!(
                stderr.contains("database")
                    || stderr.contains("connection")
                    || stderr.contains("lookup"),
                "{api}: error must mention database connection failure, got: {stderr}"
            );
        }
        None => {
            child3.kill().expect("kill");
            let _ = child3.wait();
            panic!(
                "{api}: server should have failed with invalid database URL but is still running"
            );
        }
    }

    // ---- NEGATIVE: SIGTERM during startup ----
    let mut child4 = refusable_command(&binary_path)
        .env("PORT", "8083")
        .env("APP_ENV", "development")
        .env("DATABASE_URL_DEV", "postgres://invalid/invalid")
        .spawn()
        .expect("server failed to start");

    // Send SIGTERM immediately using the standard library
    #[cfg(unix)]
    {
        extern crate libc;
        unsafe {
            libc::kill(child4.id() as i32, libc::SIGTERM);
        }
    }
    #[cfg(windows)]
    {
        child4.kill().expect("kill on windows");
    }

    match wait_for_exit(&mut child4, SIGTERM_WAIT) {
        Some(_status) => {
            // Server should have exited (either from signal or from signal handling)
            // The exact exit code depends on signal handling; what matters is it didn't hang
        }
        None => {
            child4.kill().expect("kill");
            let _ = child4.wait();
            panic!("{api}: server did not respond to SIGTERM");
        }
    }
}
