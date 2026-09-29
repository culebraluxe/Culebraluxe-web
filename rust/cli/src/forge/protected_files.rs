use std::fs;
use std::path::Path;

use super::{repo_root, Failure};

struct ProtectedFile {
    path: &'static str,
    marker: &'static str,
    why: &'static str,
}

struct ProtectedDirectory {
    dir: &'static str,
    marker: &'static str,
    why: &'static str,
}

const PROTECTED_FILES: &[ProtectedFile] = &[
    ProtectedFile { path: "AGENTS.md", marker: "# CulebraLuxe Agent Operating Context", why: "the always-read handbook every lane is handed; a lane that rewrites it rewrites its own rules" },
    ProtectedFile { path: "docs/agent/ORIENTATION.md", marker: "# ORIENTATION — the map (start here)", why: "the entry point that says where everything else lives" },
    ProtectedFile { path: "docs/agent/MEMORY.md", marker: "# Decision log", why: "the decision log: history is appended, never rewritten (a decision edited away is a decision we will make again)" },
    ProtectedFile { path: "docs/agent/CURRENT.md", marker: "# Current Machine — Forge SDLC", why: "the current-state claim the workshop reads first" },
    ProtectedFile { path: "docs/agent/releases.md", marker: "# Releases — what was actually built, deployed and probed", why: "the release receipts: the only durable evidence of what PROD served and when" },
    ProtectedFile { path: "docs/agent/COLUMN-WRITER-AUDIT.md", marker: "# Column writer audit", why: "the classified column table that was nearly overwritten by the wrong generator" },
    ProtectedFile { path: "docs/agent/typesafe-failure-triage.md", marker: "# TypeSafe failure-triage pilot", why: "the recorded boundaries that keep the outside judgment source advisory only" },
    ProtectedFile { path: "docs/agent/PERIMETER.md", marker: "# The perimeter: instruments, what they found, and the traps in running them", why: "the credential-exposure record and measured instrument findings are expensive to re-derive" },
    ProtectedFile { path: "docs/agent/SECRET-ROTATION-CHECKLIST.md", marker: "# Credential rotation checklist", why: "the provider-by-provider credential exposure checklist must not be mechanically replaced" },
];

const PROTECTED_DIRECTORIES: &[ProtectedDirectory] = &[ProtectedDirectory {
    dir: "docs/agent/manifest",
    marker: "# Scope manifest",
    why: "every markdown file here is a generated manifest and must retain the manifest header",
}];

fn marker_refusal(path: &str, content: Option<&str>, marker: &str) -> Option<String> {
    let Some(content) = content else {
        return Some(format!(
            "{path} is MISSING — it is protected, so its absence is a break, not a cleanup"
        ));
    };
    if content.starts_with(marker) {
        return None;
    }
    let first_line = content.lines().next().unwrap_or_default();
    Some(format!(
        "{path} no longer starts with \"{marker}\" (first line: {:?}) — it was written by something that did not know what it was",
        first_line.chars().take(80).collect::<String>()
    ))
}

fn protection_refusals(root: &Path) -> Vec<String> {
    let mut refusals = Vec::new();
    for file in PROTECTED_FILES {
        let full = root.join(file.path);
        let content = fs::read_to_string(&full).ok();
        if let Some(refusal) = marker_refusal(file.path, content.as_deref(), file.marker) {
            refusals.push(format!("{refusal}\n  why it is protected: {}", file.why));
        }
    }
    for dir in PROTECTED_DIRECTORIES {
        let full = root.join(dir.dir);
        let Ok(entries) = fs::read_dir(&full) else {
            refusals.push(format!("{} is MISSING — {}", dir.dir, dir.why));
            continue;
        };
        let mut paths = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md"))
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = fs::read_to_string(&path).ok();
            if let Some(refusal) = marker_refusal(&rel, content.as_deref(), dir.marker) {
                refusals.push(format!("{refusal}\n  why it matters here: {}", dir.why));
            }
        }
    }
    refusals
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    if !args.is_empty() {
        return Err(Failure::usage("usage: forge protected-files"));
    }
    let root = repo_root();
    let refusals = protection_refusals(&root);
    if refusals.is_empty() {
        println!("protected-files — PASS: every protected marker is intact");
        return Ok(0);
    }
    for refusal in &refusals {
        eprintln!("{refusal}");
    }
    Err(Failure::failed(format!(
        "protected-files — {} refusal(s)",
        refusals.len()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_protected_file_is_refused() {
        let refusal = marker_refusal("docs/agent/releases.md", None, "# Releases")
            .expect("missing protected file must refuse");
        assert!(refusal.contains("MISSING"));
    }

    #[test]
    fn wrong_generator_header_is_refused_and_reported() {
        let refusal = marker_refusal(
            "docs/agent/COLUMN-WRITER-AUDIT.md",
            Some("# Scope manifest — COLUMN-WRITER-AUDIT\n"),
            "# Column writer audit",
        )
        .expect("wrong marker must refuse");
        assert!(refusal.contains("Scope manifest — COLUMN-WRITER-AUDIT"));
    }

    #[test]
    fn intact_marker_allows_following_content() {
        let with_body = marker_refusal(
            "x.md",
            Some("# Column writer audit\n\n| table |\n"),
            "# Column writer audit",
        );
        assert!(with_body.is_none());
        let single_line = marker_refusal("x.md", Some("stray no newline"), "stray");
        assert!(single_line.is_none());
    }

    #[test]
    fn every_protection_explains_itself() {
        assert!(PROTECTED_FILES
            .iter()
            .all(|file| !file.marker.is_empty() && file.why.trim().len() > 20));
        assert!(PROTECTED_DIRECTORIES
            .iter()
            .all(|dir| !dir.marker.is_empty() && dir.why.trim().len() > 20));
    }

    #[test]
    fn real_repository_protected_markers_are_intact() {
        let refusals = protection_refusals(&repo_root());
        assert!(refusals.is_empty(), "{refusals:#?}");
    }
}
