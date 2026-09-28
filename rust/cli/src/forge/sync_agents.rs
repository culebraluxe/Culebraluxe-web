//! forge:sync-agents — regenerate the managed block in every vendor pointer file.
//!
//! Rust replacement for the retired `scripts/forge-sync-agents.ts`, which imported the deleted
//! `lib/agent-vendor-block.ts` and therefore could not run at all.
//!
//!   cargo run -p cli -- forge sync-agents           # rewrite the block where it exists, no-op if identical
//!   cargo run -p cli -- forge sync-agents --check   # exit 1 when a block is missing or has drifted
//!
//! `docs/agent/VENDOR-ADAPTERS.md` says vendor files are one-line pointers and never a second source of
//! truth. This is the tool that keeps them pointers: the content is generated from the constants in
//! `forge::vendor_block`, every guardrail is anchored to a sentence that must still exist in `AGENTS.md`,
//! and we never create a vendor file that was not already there. `forge harness-lint` fails when a block
//! drifts, so handbook-first survives a hand edit.

use crate::forge::vendor_block::{
    block_drifted, has_block, orphaned_guardrails, render_block, upsert_block, GUARDRAILS,
    MANAGED_VENDOR_FILES,
};
use crate::forge::{repo_root, Failure};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unchanged,
    WouldWrite,
    Drifted,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Unchanged => "unchanged",
            Status::WouldWrite => "would-write",
            Status::Drifted => "drifted",
        }
    }
}

pub struct VendorFile {
    pub path: String,
    pub status: Status,
    pub has_block: bool,
}

pub struct Plan {
    pub files: Vec<VendorFile>,
    pub absent: Vec<String>,
    pub orphaned: Vec<String>,
    pub writes: Vec<(String, String)>,
}

/// Pure enough to test: takes the file contents in, returns what it would do. The caller is the only
/// thing that touches the disk.
pub fn plan_sync(present: &[(String, String)], absent: &[String], agents_md: &str) -> Plan {
    let block = render_block();
    let orphaned = orphaned_guardrails(agents_md, &GUARDRAILS)
        .iter()
        .map(|guardrail| guardrail.anchored_by.to_string())
        .collect();
    let mut files: Vec<VendorFile> = Vec::new();
    let mut writes: Vec<(String, String)> = Vec::new();

    for (path, current) in present {
        if !has_block(current) {
            files.push(VendorFile {
                path: path.clone(),
                status: Status::WouldWrite,
                has_block: false,
            });
            writes.push((path.clone(), upsert_block(Some(current), &block)));
            continue;
        }
        let next = upsert_block(Some(current), &block);
        if &next == current {
            files.push(VendorFile {
                path: path.clone(),
                status: Status::Unchanged,
                has_block: true,
            });
            continue;
        }
        files.push(VendorFile {
            path: path.clone(),
            status: if block_drifted(current, &block) {
                Status::Drifted
            } else {
                Status::WouldWrite
            },
            has_block: true,
        });
        writes.push((path.clone(), next));
    }

    Plan {
        files,
        absent: absent.to_vec(),
        orphaned,
        writes,
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut check = false;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => check = true,
            "--format" => {
                json = args.get(index + 1).map(String::as_str) == Some("json");
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge sync-agents [--check] [--format json]"
                )))
            }
        }
        index += 1;
    }

    let root = repo_root();
    let agents_md = fs::read_to_string(root.join("AGENTS.md")).map_err(|_| {
        Failure::failed("AGENTS.md not found — the handbook is the source of this block")
    })?;

    let mut present: Vec<(String, String)> = Vec::new();
    let mut absent: Vec<String> = Vec::new();
    for name in MANAGED_VENDOR_FILES {
        match fs::read_to_string(root.join(name)) {
            Ok(content) => present.push((name.to_string(), content)),
            Err(_) => absent.push(name.to_string()),
        }
    }

    let plan = plan_sync(&present, &absent, &agents_md);

    if !plan.orphaned.is_empty() {
        // Refuse to write a rule the handbook no longer contains. This is the anchor check doing the
        // one job that makes replication safe.
        eprintln!("FAIL  guardrail(s) no longer anchored in AGENTS.md:");
        for anchor in &plan.orphaned {
            eprintln!("      \"{anchor}\"");
        }
        eprintln!("      fix the handbook or the guardrail constant before syncing.");
        return Ok(1);
    }

    if !check {
        for (path, content) in &plan.writes {
            fs::write(root.join(path), content)
                .map_err(|error| Failure::failed(format!("{path} could not be written: {error}")))?;
        }
    }

    let drifted = plan
        .files
        .iter()
        .filter(|file| file.status == Status::Drifted)
        .count();
    let unblocked = plan.files.iter().filter(|file| !file.has_block).count();

    if json {
        let payload = serde_json::json!({
            "mode": if check { "check" } else { "write" },
            "files": plan
                .files
                .iter()
                .map(|file| serde_json::json!({
                    "path": file.path,
                    "status": if check && file.status != Status::Unchanged {
                        "drifted"
                    } else {
                        file.status.as_str()
                    },
                    "hasBlock": file.has_block,
                }))
                .collect::<Vec<_>>(),
            "absent": plan.absent,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(if check && (drifted > 0 || unblocked > 0) {
            1
        } else {
            0
        });
    }

    for file in &plan.files {
        let label = if check {
            match file.status {
                Status::Unchanged => "ok      ",
                Status::Drifted => "FAIL    ",
                Status::WouldWrite => "MISSING ",
            }
        } else if file.status == Status::Unchanged {
            "unchanged"
        } else {
            "wrote   "
        };
        println!(
            "{label}{}{}",
            file.path,
            if file.has_block {
                ""
            } else {
                " (no managed block yet)"
            }
        );
    }
    if plan.files.is_empty() {
        println!("no vendor pointer files present");
    }
    println!(
        "\nforge:sync-agents — {} vendor file(s), {} unchanged, {} {}, {} not present",
        plan.files.len(),
        plan.files.len().saturating_sub(plan.writes.len()),
        plan.writes.len(),
        if check { "would change" } else { "written" },
        plan.absent.len()
    );
    println!(
        "  not present (we do not create vendor files): {}",
        if plan.absent.is_empty() {
            "none".to_string()
        } else {
            plan.absent.join(", ")
        }
    );
    if check && (drifted > 0 || unblocked > 0) {
        println!("  run `pnpm forge:sync-agents` to regenerate.");
    }

    Ok(if check && (drifted > 0 || unblocked > 0) {
        1
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_present_file_without_a_block_is_a_write_not_a_drift() {
        let present = vec![("CLAUDE.md".to_string(), "# CLAUDE\n".to_string())];
        let plan = plan_sync(&present, &["WARP.md".to_string()], "AGENTS body");
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].status, Status::WouldWrite);
        assert!(!plan.files[0].has_block);
        assert_eq!(plan.absent, vec!["WARP.md".to_string()]);
        assert_eq!(plan.writes.len(), 1);
    }

    #[test]
    fn a_fresh_block_is_unchanged_and_a_hand_edit_is_drift() {
        let block = render_block();
        let fresh = upsert_block(Some("# CLAUDE\n"), &block);
        let unchanged = plan_sync(&[("CLAUDE.md".to_string(), fresh.clone())], &[], "AGENTS body");
        assert_eq!(unchanged.files[0].status, Status::Unchanged);
        assert!(unchanged.writes.is_empty());

        let edited = fresh.replace("Only the Builder role commits", "Anyone may commit");
        let drifted = plan_sync(&[("CLAUDE.md".to_string(), edited)], &[], "AGENTS body");
        assert_eq!(drifted.files[0].status, Status::Drifted);
        assert_eq!(drifted.writes.len(), 1);
    }

    #[test]
    fn an_orphaned_guardrail_is_refused_before_anything_is_written() {
        let plan = plan_sync(&[], &[], "a handbook with no anchors at all");
        assert_eq!(plan.orphaned.len(), 4);
    }
}
