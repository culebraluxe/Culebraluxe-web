//! THE UNATTENDED FORGE WORKER'S LAUNCHAGENT — `com.culebraluxe.agent-worker`.
//!
//! Rust replacement for `scripts/agent-scheduler.mjs` (behind `pnpm agent:scheduler:*`). It owns NO queue
//! logic: Neon and `pnpm agent:work` own Ready discovery, hydration, claiming, Smith, Assay, publication and
//! story state. This installs and reports the timer that repeats clean Forge passes every three minutes, and
//! runs the *exact deployed wrapper* when an operator wants to see one pass by hand — which is why `run`
//! refuses outright when the deployed wrapper differs from the repository's.
//!
//! Usage:
//!   cargo run -p cli -- launchd agent-worker <render|install|status|run|stop|uninstall>

use crate::forge::Failure;
use crate::launchd::{
    env_var, launchctl, plutil_lint, render_plist, sha256_file, short_sha, write_with_mode, Machine,
};
use chrono::{SecondsFormat, Utc};
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub const LABEL: &str = "com.culebraluxe.agent-worker";
pub const CADENCE_SECONDS: u32 = 180;

const USAGE: &str = "launchd agent-worker <render|install|status|run|stop|uninstall>";

struct Paths {
    plist: PathBuf,
    log_dir: PathBuf,
    invocation_log: PathBuf,
    lock_dir: PathBuf,
    wrapper: PathBuf,
    deployed_wrapper: PathBuf,
}

/// The worker's log directory: the plist, the wrapper and the invocation log all agree on it, and
/// `AGENT_WORKER_LOG_DIR` is the one place an operator may move it. The fallback is the machine's shared
/// log tree (`Machine::log_dir`), which is outside every checkout — not a directory of this job's own.
pub fn log_dir(machine: &Machine) -> PathBuf {
    env_var("AGENT_WORKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| machine.log_dir.clone())
}

fn paths(machine: &Machine) -> Paths {
    let log_dir = log_dir(machine);
    Paths {
        plist: machine.plist_path(LABEL),
        invocation_log: log_dir.join("agent-worker.invocations.log"),
        lock_dir: log_dir.join("agent-worker.lock"),
        wrapper: machine.repo.join("scripts").join("agent-worker-once.sh"),
        deployed_wrapper: machine.support_dir.join("agent-worker-once.sh"),
        log_dir,
    }
}

pub fn template_path(machine: &Machine) -> PathBuf {
    machine
        .repo
        .join("scripts")
        .join(format!("{LABEL}.plist.template"))
}

/// The plist body, without touching the machine. Pure, so the byte-diff against the installed plist can be a
/// test as well as a command.
pub fn render(template: &str, machine: &Machine, log_dir: &Path) -> String {
    render_plist(
        template,
        &[
            ("LABEL", LABEL.to_string()),
            ("REPO_ROOT", machine.repo.display().to_string()),
            ("SUPPORT_DIR", machine.support_dir.display().to_string()),
            ("HOME", machine.home.display().to_string()),
            ("LOG_DIR", log_dir.display().to_string()),
            ("CADENCE_SECONDS", CADENCE_SECONDS.to_string()),
        ],
    )
}

struct Integrity {
    repo_sha: Option<String>,
    deployed_sha: Option<String>,
    synced: bool,
}

fn wrapper_integrity(repo_wrapper: &Path, deployed_wrapper: &Path) -> Integrity {
    let repo_sha = sha256_file(repo_wrapper);
    let deployed_sha = sha256_file(deployed_wrapper);
    let synced = matches!((&repo_sha, &deployed_sha), (Some(a), Some(b)) if a == b);
    Integrity {
        repo_sha,
        deployed_sha,
        synced,
    }
}

fn disabled_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(r#""{LABEL}"\s*=>\s*(disabled|enabled)"#)).expect("pattern is valid")
    })
}

fn is_loaded(machine: &Machine) -> bool {
    launchctl(&["print", &machine.job(LABEL)]).status.success()
}

/// `launchctl print-disabled` lists every label with its state; anything else (including a target that cannot
/// be read) answers "not disabled", which is what the TypeScript did.
fn is_disabled(machine: &Machine) -> bool {
    let target = machine.target();
    let output = launchctl(&["print-disabled", &target]);
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    match disabled_re().captures(&stdout) {
        Some(captures) => captures.get(1).map(|state| state.as_str()) == Some("disabled"),
        None => false,
    }
}

/// The running worker, read from the wrapper's own lock directory — a pid that is not alive is not a worker.
fn current_worker(paths: &Paths) -> Option<String> {
    if !paths.lock_dir.exists() {
        return None;
    }
    let pid = fs::read_to_string(paths.lock_dir.join("pid"))
        .ok()?
        .trim()
        .to_string();
    if pid.is_empty() {
        return None;
    }
    let alive = Command::new("/bin/kill")
        .args(["-0", &pid])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if alive {
        Some(pid)
    } else {
        None
    }
}

pub fn last_invocations(path: &Path, count: usize) -> String {
    if !path.exists() {
        return "(no invocations logged yet)".to_string();
    }
    let Ok(text) = fs::read_to_string(path) else {
        return "(invocation log unreadable)".to_string();
    };
    let lines: Vec<&str> = text.trim().split('\n').collect();
    if lines.is_empty() {
        return "(no invocations logged yet)".to_string();
    }
    let from = lines.len().saturating_sub(count);
    lines[from..].join("\n")
}

fn print_status(machine: &Machine) {
    let p = paths(machine);
    let plist_present = p.plist.exists();
    let integrity = wrapper_integrity(&p.wrapper, &p.deployed_wrapper);

    let summary = if !plist_present {
        "not installed (plist missing)"
    } else if is_loaded(machine) {
        "installed + loaded"
    } else {
        "installed (plist present, not loaded)"
    };

    let worker = current_worker(&p);
    println!("CulebraLuxe Forge worker scheduler");
    println!("  status:     {summary}");
    println!("  label:      {LABEL}");
    println!("  cadence:    every {CADENCE_SECONDS}s (3 minutes)");
    println!(
        "  plist:      {}{}",
        p.plist.display(),
        if plist_present { "" } else { " (missing)" }
    );
    let disabled = if !plist_present {
        "n/a".to_string()
    } else if is_disabled(machine) {
        "yes".to_string()
    } else {
        "no".to_string()
    };
    println!("  disabled:   {disabled}");
    println!(
        "  deployed:   {}{}",
        p.deployed_wrapper.display(),
        if p.deployed_wrapper.exists() {
            ""
        } else {
            " (missing)"
        }
    );
    println!(
        "  repo:       {}/scripts/agent-worker-once.sh",
        machine.repo.display()
    );
    println!(
        "  wrapper:    {}",
        if integrity.synced {
            format!("synced sha256={}", short_sha(integrity.repo_sha.as_deref()))
        } else {
            format!(
                "MISMATCH repo={} deployed={}",
                short_sha(integrity.repo_sha.as_deref()),
                short_sha(integrity.deployed_sha.as_deref())
            )
        }
    );
    println!(
        "  running:    {}",
        match &worker {
            Some(pid) => format!("yes (pid {pid})"),
            None => "no".to_string(),
        }
    );
    println!(
        "  logs:       {}/agent-worker.{{out,err,invocations}}.log",
        p.log_dir.display()
    );
    println!("  last invocations:");
    for line in last_invocations(&p.invocation_log, 4).split('\n') {
        println!("    {line}");
    }
}

fn install(machine: &Machine) -> Result<u8, Failure> {
    let p = paths(machine);
    for directory in [&machine.launch_agents_dir, &machine.support_dir, &p.log_dir] {
        if let Err(error) = fs::create_dir_all(directory) {
            eprintln!("cannot create {}: {error}", directory.display());
            return Ok(1);
        }
    }

    let source = match fs::read(&p.wrapper) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("cannot read {}: {error}", p.wrapper.display());
            return Ok(1);
        }
    };
    if let Err(error) = write_with_mode(&p.deployed_wrapper, &source, 0o755) {
        eprintln!("cannot deploy {}: {error}", p.deployed_wrapper.display());
        return Ok(1);
    }

    let integrity = wrapper_integrity(&p.wrapper, &p.deployed_wrapper);
    if !integrity.synced {
        eprintln!(
            "agent-worker install integrity failure: repo={} deployed={}",
            short_sha(integrity.repo_sha.as_deref()),
            short_sha(integrity.deployed_sha.as_deref())
        );
        return Ok(1);
    }

    let stamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let log_line = format!(
        "{stamp} installed: deployed-wrapper sha256={}\n",
        short_sha(integrity.deployed_sha.as_deref())
    );
    if let Err(error) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p.invocation_log)
        .and_then(|mut log| log.write_all(log_line.as_bytes()))
    {
        eprintln!("cannot append to {}: {error}", p.invocation_log.display());
        return Ok(1);
    }

    let template_file = template_path(machine);
    let template = match fs::read_to_string(&template_file) {
        Ok(template) => template,
        Err(error) => {
            eprintln!("cannot read {}: {error}", template_file.display());
            return Ok(1);
        }
    };
    let plist = render(&template, machine, &p.log_dir);
    if let Err(error) = write_with_mode(&p.plist, plist.as_bytes(), 0o644) {
        eprintln!("cannot write {}: {error}", p.plist.display());
        return Ok(1);
    }

    let lint = plutil_lint(&p.plist);
    if !lint.status.success() {
        eprintln!(
            "plutil rejected generated plist:\n{}{}",
            String::from_utf8_lossy(&lint.stdout),
            String::from_utf8_lossy(&lint.stderr)
        );
        return Ok(1);
    }

    // Idempotent (re)enable: a job that is not loaded makes `bootout` fail, and that is not an error here.
    let job = machine.job(LABEL);
    let target = machine.target();
    launchctl(&["bootout", &job]);
    launchctl(&["enable", &job]);
    let boot = launchctl(&["bootstrap", &target, &p.plist.display().to_string()]);
    if !boot.status.success() {
        let stderr = String::from_utf8_lossy(&boot.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&boot.stdout).trim().to_string();
        eprintln!(
            "launchctl bootstrap failed:\n{}",
            if stderr.is_empty() { stdout } else { stderr }
        );
        return Ok(1);
    }

    println!("installed + enabled:");
    print_status(machine);
    Ok(0)
}

fn run_worker(machine: &Machine) -> Result<u8, Failure> {
    let p = paths(machine);
    if !p.deployed_wrapper.exists() {
        eprintln!(
            "deployed agent-worker wrapper missing: {}",
            p.deployed_wrapper.display()
        );
        eprintln!("run `pnpm agent:scheduler:install` first");
        return Ok(1);
    }
    let integrity = wrapper_integrity(&p.wrapper, &p.deployed_wrapper);
    if !integrity.synced {
        eprintln!(
            "refusing diagnostic run: deployed wrapper differs from repo wrapper (repo={} deployed={})",
            short_sha(integrity.repo_sha.as_deref()),
            short_sha(integrity.deployed_sha.as_deref())
        );
        eprintln!("run `pnpm agent:scheduler:install` first");
        return Ok(2);
    }

    // THE PATH AND THE ENVIRONMENT ARE PART OF THE FIDELITY: this is the file launchd executes, given what
    // the plist gives it. Without `AGENT_WORKER_REPO` the wrapper falls back to a folder with no repository
    // and no `.env.local`, and the diagnostic run dies at a different place than the scheduled one — which is
    // how a manual run once "proved" a job healthy while the timer was dying at its branch check.
    let worker_id = env_var("AGENT_WORKER_ID").unwrap_or_else(|| "scheduler".to_string());
    let status = Command::new("/bin/bash")
        .arg(&p.deployed_wrapper)
        .env("HOME", &machine.home)
        .env("AGENT_WORKER_REPO", &machine.repo)
        .env("AGENT_WORKER_LOG_DIR", &p.log_dir)
        .env("AGENT_WORKER_ID", worker_id)
        .status();
    match status {
        Ok(status) => Ok(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("cannot run {}: {error}", p.deployed_wrapper.display());
            Ok(1)
        }
    }
}

fn stop(machine: &Machine) -> Result<u8, Failure> {
    let p = paths(machine);
    let job = machine.job(LABEL);
    launchctl(&["bootout", &job]);
    launchctl(&["disable", &job]);
    println!(
        "stopped: no future scheduled invocations (plist kept at {}).",
        p.plist.display()
    );
    println!("Story Board data untouched. Re-enable with `pnpm agent:scheduler:install`.");
    Ok(0)
}

fn uninstall(machine: &Machine) -> Result<u8, Failure> {
    let p = paths(machine);
    let job = machine.job(LABEL);
    launchctl(&["bootout", &job]);
    launchctl(&["disable", &job]);
    // The removal IS the intent here, so a failure to remove is reported by the next `status` rather than
    // swallowed: the file's absence is what this verb promises.
    if p.plist.exists() {
        let _ = fs::remove_file(&p.plist);
    }
    if p.deployed_wrapper.exists() {
        let _ = fs::remove_file(&p.deployed_wrapper);
    }
    println!("uninstalled: launchd job removed, plist + deployed wrapper deleted.");
    println!(
        "Story Board data untouched. Logs kept at {}",
        p.log_dir.display()
    );
    Ok(0)
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let machine = Machine::detect();
    match args.first().map(String::as_str).unwrap_or_default() {
        // `render` touches nothing: it prints what `install` would write, which is how a port of this file is
        // proved — by diffing its output against the plist the TypeScript already installed.
        "render" => {
            let file = template_path(&machine);
            let template = fs::read_to_string(&file).map_err(|error| {
                Failure::configuration(format!("cannot read {}: {error}", file.display()))
            })?;
            print!("{}", render(&template, &machine, &log_dir(&machine)));
            Ok(0)
        }
        "install" => install(&machine),
        "status" => {
            print_status(&machine);
            Ok(0)
        }
        "run" => run_worker(&machine),
        "stop" => stop(&machine),
        "uninstall" => uninstall(&machine),
        "help" | "--help" | "-h" => {
            println!("usage: cargo run -p cli -- {USAGE}");
            println!("commands:");
            println!("  render      print the plist `install` would write (the port's own proof)");
            println!(
                "  install     render + install + bootstrap the 3-minute LaunchAgent (idempotent)"
            );
            println!("  status      show loaded/enabled state, wrapper integrity, running worker, last invocations");
            println!("  run         run the exact deployed unattended Forge wrapper now");
            println!("  stop        kill switch: boot out + persist disabled");
            println!("  uninstall   stop + delete the plist");
            Ok(0)
        }
        other => Err(Failure::usage(format!(
            "unknown command `{other}`; usage: {USAGE}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch inside the test, removed inside the test: nothing outlives the run.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("culebraluxe-launchd-{name}"));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch directory");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn machine(repo: &Path, home: &Path) -> Machine {
        Machine {
            repo: repo.to_path_buf(),
            home: home.to_path_buf(),
            launch_agents_dir: home.join("Library/LaunchAgents"),
            support_dir: home.join("Library/Application Support/CulebraLuxe"),
            log_dir: home.join("logs"),
            uid: 501,
        }
    }

    #[test]
    fn the_render_fills_every_placeholder_the_template_carries() {
        let scratch = Scratch::new("render");
        let log_dir = scratch.path().join("Logs/CulebraLuxe");
        let machine = machine(scratch.path(), scratch.path());
        let template =
            "{{LABEL}}|{{REPO_ROOT}}|{{SUPPORT_DIR}}|{{HOME}}|{{LOG_DIR}}|{{CADENCE_SECONDS}}";

        let rendered = render(template, &machine, &log_dir);

        assert_eq!(
            rendered,
            format!(
                "{LABEL}|{repo}|{support}|{home}|{logs}|{CADENCE_SECONDS}",
                repo = scratch.path().display(),
                support = machine.support_dir.display(),
                home = scratch.path().display(),
                logs = log_dir.display(),
            )
        );
    }

    /// The tracked template is the interface: if it grows a seventh placeholder, this test is the one that says
    /// so instead of a plist that quietly keeps the token.
    #[test]
    fn the_tracked_template_has_no_placeholder_left_after_a_render() {
        let scratch = Scratch::new("template");
        let machine = machine(&crate::forge::repo_root(), scratch.path());
        let template = fs::read_to_string(template_path(&machine)).expect("the tracked template");

        let rendered = render(&template, &machine, &scratch.path().join("logs"));

        assert!(
            !rendered.contains("{{"),
            "a placeholder survived the render: {rendered}"
        );
        assert!(rendered.contains(&format!("<integer>{CADENCE_SECONDS}</integer>")));
        assert!(rendered.contains(&format!("<string>{LABEL}</string>")));
    }

    #[test]
    fn the_wrapper_rule_is_synced_only_when_both_hashes_match() {
        let scratch = Scratch::new("integrity");
        let repo_wrapper = scratch.path().join("scripts/agent-worker-once.sh");
        let deployed = scratch.path().join("support/agent-worker-once.sh");
        fs::create_dir_all(repo_wrapper.parent().expect("parent")).expect("scratch scripts");
        fs::write(&repo_wrapper, "#!/bin/bash\necho hi\n").expect("scratch wrapper");

        assert!(
            !wrapper_integrity(&repo_wrapper, &deployed).synced,
            "a deployed copy that is not there is not synced"
        );

        fs::create_dir_all(deployed.parent().expect("parent")).expect("scratch support");
        fs::write(&deployed, "#!/bin/bash\necho hi\n").expect("deployed wrapper");
        assert!(wrapper_integrity(&repo_wrapper, &deployed).synced);

        fs::write(&deployed, "#!/bin/bash\necho bye\n").expect("drifted wrapper");
        let drifted = wrapper_integrity(&repo_wrapper, &deployed);
        assert!(!drifted.synced);
        assert_ne!(drifted.repo_sha, drifted.deployed_sha);
    }

    /// The wrapper must be able to start the worker it names in a launchd environment — and every way it
    /// could not has happened, each one silently, because a tick that dies leaves one line in a log nobody
    /// reads: the invocation was the retired `pnpm agent:work` shim (2026-09-25), it then declared no
    /// `APP_ENV` while `forge/src/bin/forge_worker.rs:3` refuses anything but `production` (exit 2,
    /// every tick, four days), and it never supplied `DATABASE_URL_PROD`, the one value `forge-worker`
    /// cannot load for itself because it is not the CLI (2026-09-29, `DatabaseUnavailable` before a single
    /// work item was claimed). Read from the real file rather than a fixture: a fixture would have passed
    /// through all three of those changes.
    #[test]
    fn the_scheduled_wrapper_can_start_the_worker_it_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cli sits one level below the repository root");
        let wrapper = root.join("scripts/agent-worker-once.sh");
        let text = fs::read_to_string(&wrapper)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", wrapper.display()));

        assert!(
            text.contains(r#"APP_ENV="${APP_ENV:-production}""#),
            "forge-worker exits 2 without a production target, so the wrapper must declare one"
        );
        assert!(
            text.contains("DATABASE_URL_PROD"),
            "the worker's own pool has no connection string without DATABASE_URL_PROD"
        );
        assert!(
            text.contains("-p forge --bin forge-worker"),
            "the scheduled command must be the Rust worker"
        );
        let executes_retired_shim = text
            .lines()
            .any(|line| line.trim_start().starts_with("pnpm agent:work"));
        assert!(
            !executes_retired_shim,
            "the retired TypeScript shim must not be the scheduled command"
        );
    }

    #[test]
    fn the_invocation_tail_is_the_last_four_lines_or_the_reason_there_are_none() {
        let scratch = Scratch::new("tail");
        let log = scratch.path().join("agent-worker.invocations.log");

        assert_eq!(last_invocations(&log, 4), "(no invocations logged yet)");

        let body = (1..=6)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        fs::write(&log, body).expect("scratch log");
        assert_eq!(last_invocations(&log, 4), "line 3\nline 4\nline 5\nline 6");

        // The TypeScript said the same thing about a whitespace-only log: nothing, not a placeholder.
        fs::write(&log, "   \n").expect("scratch log");
        assert_eq!(last_invocations(&log, 4), "");
    }

    #[test]
    fn the_disabled_pattern_reads_the_state_launchctl_prints() {
        let sample = format!("\t\"{LABEL}\" => disabled\n\t\"other.label\" => enabled\n");

        let captures = disabled_re()
            .captures(&sample)
            .expect("the label is listed");

        assert_eq!(captures.get(1).expect("state").as_str(), "disabled");
        assert!(disabled_re()
            .captures("\t\"other.label\" => enabled\n")
            .is_none());
    }
}
