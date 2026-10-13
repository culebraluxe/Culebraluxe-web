//! Pinned, bounded Git input for the learning scanner.

use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use wait_timeout::ChildExt;

const GIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_GIT_OUTPUT: usize = 8 * 1024 * 1024;
pub(super) const MAX_SOURCE_BYTES: usize = 256 * 1024;

struct GitOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    truncated: bool,
}

fn read_limited<R: Read>(mut reader: R, limit: usize) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(size) => {
                let remaining = limit.saturating_sub(output.len());
                output.extend_from_slice(&buffer[..size.min(remaining)]);
                truncated |= size > remaining;
            }
        }
    }
    (output, truncated)
}

fn run_git(root: &Path, args: &[String], output_limit: usize) -> Result<GitOutput, String> {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start bounded Git command: {error}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_reader = thread::spawn(move || read_limited(stdout, output_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, 16 * 1024));
    let status = match child
        .wait_timeout(GIT_TIMEOUT)
        .map_err(|error| format!("could not wait for Git command: {error}"))?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(format!(
                "Git command exceeded {} second time limit",
                GIT_TIMEOUT.as_secs()
            ));
        }
    };
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| "Git stdout reader panicked".to_string())?;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| "Git stderr reader panicked".to_string())?;
    Ok(GitOutput {
        status,
        stdout,
        stderr,
        truncated: stdout_truncated || stderr_truncated,
    })
}

fn git_stdout(root: &Path, args: &[String], limit: usize) -> Result<Vec<u8>, String> {
    let result = run_git(root, args, limit)?;
    if result.truncated {
        return Err("Git output exceeded the configured byte limit".into());
    }
    if !result.status.success() {
        return Err(format!(
            "Git command failed (exit {}): {}",
            result.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(result.stdout)
}

pub(super) fn source_revision(root: &Path) -> Result<String, String> {
    let bytes = git_stdout(
        root,
        &[
            "rev-parse".into(),
            "--verify".into(),
            "HEAD^{commit}".into(),
        ],
        256,
    )
    .map_err(|error| format!("origin remote is unavailable: {error}"))?;
    let revision = String::from_utf8_lossy(&bytes).trim().to_string();
    if revision.is_empty() {
        return Err("Git returned an empty source revision".into());
    }
    Ok(revision)
}

fn canonical_remote(remote: &str) -> String {
    if let Some((scheme, rest)) = remote.split_once("://") {
        let host_and_path = rest.rsplit_once('@').map_or(rest, |(_, value)| value);
        return format!("{}://{}", scheme.to_ascii_lowercase(), host_and_path);
    }
    if let Some((_, rest)) = remote.split_once('@') {
        if rest.contains(':') {
            return rest.to_string();
        }
    }
    remote.to_string()
}

pub(super) fn repository_key(root: &Path) -> Result<String, String> {
    let remote = git_stdout(
        root,
        &["config".into(), "--get".into(), "remote.origin.url".into()],
        4096,
    )
    .map_err(|error| format!("origin remote is unavailable: {error}"))?;
    let remote = String::from_utf8_lossy(&remote).trim().to_string();
    if remote.is_empty() {
        return Err("origin remote is required for durable cross-worker learning progress".into());
    }
    let identity = canonical_remote(&remote);
    let digest = Sha256::digest(identity.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to String");
    }
    Ok(format!("git:{hex}"))
}

pub(super) fn changed_paths(
    root: &Path,
    cursor: Option<&str>,
    revision: &str,
    since_unix: u64,
) -> Result<Vec<String>, String> {
    let mut args = if let Some(cursor) = cursor {
        vec![
            "diff".into(),
            "--diff-filter=ACMRT".into(),
            "--name-only".into(),
            "-z".into(),
            cursor.into(),
            revision.into(),
            "--".into(),
        ]
    } else {
        // `--since=@{unix}` is not "since that epoch" on git 2.54: `@0` resolves to NOW
        // (`git rev-parse --since=@0` -> `--max-age=1791861979`, the current second), so a pass anchored at
        // epoch 0 listed no commits at all - an empty window, and the anchor could never move. `--max-age`
        // takes the timestamp itself (`man git-rev-list`, commit limiting), so no argument is ever handed to
        // the date parser: measured, `--max-age=0` lists every commit where `--since=@0` lists none.
        vec![
            "log".into(),
            format!("--max-age={since_unix}"),
            "--diff-filter=ACMRT".into(),
            "--name-only".into(),
            "--pretty=format:".into(),
            "-z".into(),
            revision.into(),
            "--".into(),
        ]
    };
    // The pinned revision is the only tree input; never read the mutable working directory.
    args.shrink_to_fit();
    let bytes = git_stdout(root, &args, MAX_GIT_OUTPUT)?;
    let mut paths = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| {
            String::from_utf8(part.to_vec())
                .map_err(|_| "Git returned a non-UTF-8 source path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    paths.dedup();
    // A window can change a path and then delete it, and the scan reads every path AT the pin: there is no
    // source for a path the revision does not hold. The `--since` form lists the modifying commit's paths
    // even when a later commit in the same window deleted the file (the tree-to-tree form filters it as a D,
    // the per-commit form cannot), and asking for it anyway failed the whole pass — 2026-10-12,
    // `db/src/document_sign.rs` deleted inside a window whose anchor predated the deletion, so every tick
    // failed the same way and the learn anchor never moved. One listing of the pinned tree, not one probe per
    // path, says what is still there; what is not has nothing to observe.
    let present = pinned_paths(root, revision)?;
    paths.retain(|path| present.binary_search(path).is_ok());
    Ok(paths)
}

/// Every path the pinned revision holds, sorted, from one `ls-tree` listing.
fn pinned_paths(root: &Path, revision: &str) -> Result<Vec<String>, String> {
    let bytes = git_stdout(
        root,
        &[
            "ls-tree".into(),
            "-r".into(),
            "-z".into(),
            "--name-only".into(),
            revision.into(),
        ],
        MAX_GIT_OUTPUT,
    )?;
    let mut paths = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| {
            String::from_utf8(part.to_vec())
                .map_err(|_| "Git returned a non-UTF-8 source path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}

/// The pinned revision's copy of `path`, or `None` when the revision does not hold that path at all.
///
/// ABSENCE IS NOT A FAILURE (2026-10-12). A window that changed a path and then deleted it names the path in
/// the commit that changed it (`--since` cannot filter a later deletion), and the durable scan resumes from a
/// *stored* pending list, so the same path is asked for again on every tick. Failing the chunk there stopped
/// the whole pass and the scan never advanced: a single deletion in the window ended learning for good, with
/// the same `fatal: path 'db/src/document_sign.rs' does not exist in '987a7ffab'` every three minutes. A path
/// the pin does not have holds nothing to observe, so it reads as absent and the chunk moves on. Every other
/// git failure is still a failure.
pub(super) fn read_pinned_source(
    root: &Path,
    revision: &str,
    path: &str,
) -> Result<Option<String>, String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment == ".." || segment.is_empty())
    {
        return Err(format!(
            "Git returned an invalid repository-relative path: {path:?}"
        ));
    }
    let spec = format!("{revision}:{path}");
    let bytes = match git_stdout(root, &["show".into(), spec], MAX_SOURCE_BYTES + 1) {
        Ok(bytes) => bytes,
        Err(error) => {
            return if pinned_holds_path(root, revision, path)? {
                Err(error)
            } else {
                Ok(None)
            };
        }
    };
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "source file {path} exceeds the {} byte scan limit at {revision}",
            MAX_SOURCE_BYTES
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| format!("source file {path} is not valid UTF-8 at {revision}"))
}

/// Whether the pinned revision holds that exact path, asked of the tree rather than of the object database: a
/// path with no blob in the pin exits non-zero from `cat-file -e` for "absent" and for "bad spec" alike, while
/// `ls-tree -- <path>` answers with an empty list and exit 0 either way.
fn pinned_holds_path(root: &Path, revision: &str, path: &str) -> Result<bool, String> {
    let bytes = git_stdout(
        root,
        &[
            "ls-tree".into(),
            "-r".into(),
            "-z".into(),
            "--name-only".into(),
            revision.into(),
            "--".into(),
            path.into(),
        ],
        64 * 1024,
    )?;
    Ok(!bytes.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("git starts");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn changed_paths_and_source_reads_are_pinned_to_the_requested_revision() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Forge Test"]);
        git(
            root,
            &["config", "user.email", "forge-test@example.invalid"],
        );
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::write(
            root.join("src/lib.rs"),
            "pub fn state() { let _ = read(); }\n",
        )
        .expect("first source");
        git(root, &["add", "src/lib.rs"]);
        git(root, &["commit", "-qm", "initial"]);
        let first = git(root, &["rev-parse", "HEAD"]);

        fs::write(
            root.join("src/lib.rs"),
            "pub fn pinned() { let _ = read(); }\n",
        )
        .expect("second source");
        git(root, &["commit", "-qam", "update"]);
        let pinned = git(root, &["rev-parse", "HEAD"]);
        fs::write(root.join("src/lib.rs"), "pub fn mutable_worktree() {}\n")
            .expect("mutate working tree after commit");

        let paths = changed_paths(root, Some(&first), &pinned, 0).expect("revision diff");
        assert_eq!(paths, vec!["src/lib.rs"]);
        let source = read_pinned_source(root, &pinned, &paths[0])
            .expect("pinned file")
            .expect("the pin holds it");
        assert!(source.contains("pinned"));
        assert!(!source.contains("mutable_worktree"));
    }

    #[test]
    fn repository_identity_never_returns_the_remote_url() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        git(root, &["init", "-q"]);
        git(
            root,
            &[
                "remote",
                "add",
                "origin",
                "https://alice:secret@example.invalid/org/repo.git",
            ],
        );
        let identity = repository_key(root).expect("hashed repository identity");
        assert!(identity.starts_with("git:"));
        assert!(!identity.contains("secret"));
        assert!(!identity.contains("example.invalid"));
    }

    #[test]
    fn repository_identity_requires_a_stable_remote_scope() {
        let temp = tempfile::tempdir().expect("temporary repository");
        git(temp.path(), &["init", "-q"]);
        assert!(repository_key(temp.path()).is_err());
    }

    #[test]
    fn invalid_paths_fail_closed() {
        let temp = tempfile::tempdir().expect("temporary repository");
        assert!(read_pinned_source(temp.path(), "HEAD", "../secret.rs").is_err());
        assert!(read_pinned_source(temp.path(), "HEAD", "/etc/passwd").is_err());
    }

    /// The window is rebuilt from the same anchor on every tick, so a path it changed and then deleted is
    /// named again and again: the scan has to drop it, or one deletion inside the window keeps the learn
    /// anchor from ever advancing.
    #[test]
    fn changed_paths_drops_a_path_the_pinned_revision_no_longer_holds() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Forge Test"]);
        git(
            root,
            &["config", "user.email", "forge-test@example.invalid"],
        );
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::write(root.join("src/kept.rs"), "pub fn kept() {}\n").expect("kept source");
        fs::write(root.join("src/gone.rs"), "pub fn gone() {}\n").expect("doomed source");
        git(root, &["add", "src/kept.rs", "src/gone.rs"]);
        git(root, &["commit", "-qm", "initial"]);

        fs::write(root.join("src/kept.rs"), "pub fn kept_again() {}\n")
            .expect("kept source, again");
        git(root, &["rm", "-q", "src/gone.rs"]);
        git(root, &["commit", "-qam", "change one path, delete another"]);
        let pinned = git(root, &["rev-parse", "HEAD"]);

        // The premise, asserted so a flake names itself instead of showing an empty list: the per-commit
        // (`log`) form is the only form that can name a path the pin does not hold. Spelled `--max-age`, like
        // `changed_paths` below: `--since=@0` here resolved to "now", so this premise held only while the
        // fixture's commits and the query shared a second, and it failed once in three full-suite runs.
        let raw = git(
            root,
            &[
                "log",
                "--max-age=0",
                "--diff-filter=ACMRT",
                "--name-only",
                "--pretty=format:",
                "-z",
                &pinned,
                "--",
            ],
        );
        assert!(
            raw.contains("src/gone.rs"),
            "the window must name the deleted path, or this test proves nothing: {raw:?}"
        );

        let paths = changed_paths(root, None, &pinned, 0).expect("revision log");
        assert_eq!(paths, vec!["src/kept.rs"]);
        let source = read_pinned_source(root, &pinned, &paths[0])
            .expect("pinned file")
            .expect("the pin holds it");
        assert!(source.contains("kept_again"));
    }

    /// And the read itself, which is the call the live failure went through: the durable scan resumes from a
    /// pending list it stored earlier, so the deletion keeps arriving long after the window that named it.
    #[test]
    fn a_path_the_pinned_revision_does_not_hold_reads_as_absent() {
        let temp = tempfile::tempdir().expect("temporary repository");
        let root = temp.path();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Forge Test"]);
        git(
            root,
            &["config", "user.email", "forge-test@example.invalid"],
        );
        fs::write(root.join("kept.rs"), "pub fn kept() {}\n").expect("kept source");
        fs::write(root.join("gone.rs"), "pub fn gone() {}\n").expect("doomed source");
        git(root, &["add", "kept.rs", "gone.rs"]);
        git(root, &["commit", "-qm", "initial"]);
        git(root, &["rm", "-q", "gone.rs"]);
        git(root, &["commit", "-qam", "delete one path"]);
        let pinned = git(root, &["rev-parse", "HEAD"]);

        assert_eq!(
            read_pinned_source(root, &pinned, "gone.rs").expect("git answers"),
            None
        );
        let kept = read_pinned_source(root, &pinned, "kept.rs")
            .expect("git answers")
            .expect("the pin holds it");
        assert!(kept.contains("kept"));
    }
}
