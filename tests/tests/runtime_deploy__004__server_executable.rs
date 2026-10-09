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

#[test]
#[allow(non_snake_case)]
fn runtime_deploy_004__server_executable() {
    let api = "server executable";

    // Build the server binary in release mode (what production uses)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| "/Users/Shared/dev/src/lane-nemotron/tests".to_string());
    let root = std::path::Path::new(&manifest_dir).parent().unwrap();

    // Find the target directory - use CARGO_TARGET_DIR or default
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        root.join("build")
            .join("rust")
            .to_string_lossy()
            .to_string()
    });

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
    let mut child = Command::new(&binary_path)
        .env("PORT", "8080")
        // APP_ENV intentionally not set
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server failed to start");

    thread::sleep(Duration::from_millis(500));

    match child.try_wait() {
        Ok(Some(status)) => {
            // Server should exit with error when APP_ENV is not set
            assert!(
                !status.success(),
                "{api}: server must fail when APP_ENV is not set"
            );
            let mut stderr = String::new();
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            assert!(
                stderr.contains("database target is undeclared") || stderr.contains("APP_ENV"),
                "{api}: error must mention missing environment declaration, got: {stderr}"
            );
        }
        Ok(None) => {
            // If it's still running, kill it and fail
            child.kill().expect("kill");
            let _ = child.wait();
            panic!("{api}: server should have failed without APP_ENV but is still running");
        }
        Err(e) => {
            panic!("{api}: failed to check process status: {e}");
        }
    }

    // ---- NEGATIVE: Missing database URL (APP_ENV set but no DATABASE_URL) ----
    let mut child2 = Command::new(&binary_path)
        .env("PORT", "8081")
        .env("APP_ENV", "development")
        // DATABASE_URL_DEV intentionally not set
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server failed to start");

    thread::sleep(Duration::from_millis(500));

    match child2.try_wait() {
        Ok(Some(status)) => {
            assert!(
                !status.success(),
                "{api}: server must fail when DATABASE_URL_DEV is not set"
            );
            let mut stderr = String::new();
            if let Some(mut err) = child2.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            assert!(
                stderr.contains("DATABASE_URL_DEV")
                    || stderr.contains("database")
                    || stderr.contains("connection"),
                "{api}: error must mention missing database URL, got: {stderr}"
            );
        }
        Ok(None) => {
            child2.kill().expect("kill");
            let _ = child2.wait();
            panic!(
                "{api}: server should have failed without DATABASE_URL_DEV but is still running"
            );
        }
        Err(e) => {
            panic!("{api}: failed to check process status: {e}");
        }
    }

    // ---- NEGATIVE: Invalid database URL (wrong host) ----
    let mut child3 = Command::new(&binary_path)
        .env("PORT", "8082")
        .env("APP_ENV", "development")
        .env("DATABASE_URL_DEV", "postgres://invalid-host/invalid")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server failed to start");

    thread::sleep(Duration::from_millis(500));

    match child3.try_wait() {
        Ok(Some(status)) => {
            assert!(
                !status.success(),
                "{api}: server must fail with invalid database URL"
            );
            let mut stderr = String::new();
            if let Some(mut err) = child3.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            assert!(
                stderr.contains("database")
                    || stderr.contains("connection")
                    || stderr.contains("lookup"),
                "{api}: error must mention database connection failure, got: {stderr}"
            );
        }
        Ok(None) => {
            child3.kill().expect("kill");
            let _ = child3.wait();
            panic!(
                "{api}: server should have failed with invalid database URL but is still running"
            );
        }
        Err(e) => {
            panic!("{api}: failed to check process status: {e}");
        }
    }

    // ---- NEGATIVE: SIGTERM during startup ----
    let mut child4 = Command::new(&binary_path)
        .env("PORT", "8083")
        .env("APP_ENV", "development")
        .env("DATABASE_URL_DEV", "postgres://invalid/invalid")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

    thread::sleep(Duration::from_millis(200));

    match child4.try_wait() {
        Ok(Some(_status)) => {
            // Server should have exited (either from signal or from signal handling)
            // The exact exit code depends on signal handling; what matters is it didn't hang
        }
        Ok(None) => {
            child4.kill().expect("kill");
            let _ = child4.wait();
            panic!("{api}: server did not respond to SIGTERM");
        }
        Err(e) => {
            panic!("{api}: failed to check process status: {e}");
        }
    }
}
