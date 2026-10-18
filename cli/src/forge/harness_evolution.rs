//! `forge harness-evolution` — the operator surface for the Harness Evolution subsystem.
//!
//! Read-only. It answers the questions a human actually asks between experiments:
//! which harness revision is active per domain, what revisions exist, what experiments ran, and
//! what lessons are retrievable. It never probes, adopts, rolls back, or writes; those stay in the
//! subsystem (or a later, explicitly-authorized command), not in a status viewer.
//!
//! Usage:
//!   cargo run -p cli -- forge harness-evolution status [--format json]
//!   cargo run -p cli -- forge harness-evolution revisions [--domain D] [--format json]
//!   cargo run -p cli -- forge harness-evolution experiments --domain D [--format json]
//!   cargo run -p cli -- forge harness-evolution experience --domain D [--executor E]
//!       [--mode parallel|sequential] [--seed N] [--limit N] [--format json]

use super::{connect, Failure};
use db::{ForgeHarnessDao, ForgeHarnessRevisionRow};
use serde_json::{json, Value};

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    crate::apple_sync::load_env();
    if args.iter().any(|value| value == "--help") {
        usage();
        return Ok(0);
    }
    let command = args.first().map(String::as_str).unwrap_or("status");
    let json_out = wants_json(args);
    let database = connect().await?;
    let target = database.declared_target().as_str().to_string();
    let dao = ForgeHarnessDao::new(database);

    match command {
        "status" => {
            let revisions = dao
                .list_all_revisions()
                .await
                .map_err(|error| Failure::failed(format!("cannot list revisions: {error}")))?;
            let mut domains: Vec<String> =
                revisions.iter().map(|row| row.domain.clone()).collect();
            domains.sort();
            domains.dedup();
            let mut active = Vec::new();
            for domain in &domains {
                let activation = dao
                    .current_activation(domain)
                    .await
                    .map_err(|error| Failure::failed(format!("cannot read activation: {error}")))?;
                active.push(json!({
                    "domain": domain,
                    "revision_id": activation.as_ref().map(|row| row.to_revision_id.clone()),
                    "activation_kind": activation.as_ref().map(|row| row.activation_kind.clone()),
                    "activated_at": activation.as_ref().map(|row| row.activated_at),
                    "activated_by": activation.as_ref().map(|row| row.activated_by.clone()),
                }));
            }
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "active": active,
                    "revision_count": revisions.len(),
                    "revisions": revisions.iter().map(revision_json).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution status — target={target} (read-only)");
                println!("REVISIONS {}", revisions.len());
                if active.is_empty() {
                    println!("ACTIVE    none (no harness domain has an active revision)");
                } else {
                    for item in &active {
                        let revision = item.get("revision_id").and_then(Value::as_str);
                        println!(
                            "ACTIVE    {}  {}",
                            item.get("domain").and_then(Value::as_str).unwrap_or("?"),
                            revision.unwrap_or("none")
                        );
                    }
                }
            }
            Ok(0)
        }
        "revisions" => {
            let domain = flag(args, "--domain");
            let rows = match domain.as_deref() {
                Some(domain) => dao.list_revisions(domain).await,
                None => dao.list_all_revisions().await,
            }
            .map_err(|error| Failure::failed(format!("cannot list revisions: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "revisions": rows.iter().map(revision_json).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution revisions — target={target} (read-only)");
                for row in &rows {
                    println!(
                        "  {}  {}  {}  {}  {}",
                        row.revision_id, row.domain, row.status, short(&row.manifest_hash, 12), row.created_by
                    );
                }
                if rows.is_empty() {
                    println!("  no revisions");
                }
            }
            Ok(0)
        }
        "experiments" => {
            let domain = required_flag(args, "--domain")?;
            let rows = dao
                .list_experiments(&domain)
                .await
                .map_err(|error| Failure::failed(format!("cannot list experiments: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "experiments": rows.iter().map(|row| json!({
                        "experiment_id": row.experiment_id,
                        "domain": row.domain,
                        "base_revision_id": row.base_revision_id,
                        "mode": row.mode,
                        "k": row.k,
                        "state": row.state,
                        "successor_revision_id": row.successor_revision_id,
                        "created_at": row.created_at,
                    })).collect::<Vec<_>>(),
                }))?;
            } else {
                println!("forge harness-evolution experiments — domain={domain} target={target} (read-only)");
                for row in &rows {
                    println!(
                        "  {}  {}  k={}  base={}  successor={}",
                        row.experiment_id,
                        row.state,
                        row.k,
                        row.base_revision_id,
                        row.successor_revision_id.as_deref().unwrap_or("none")
                    );
                }
                if rows.is_empty() {
                    println!("  no experiments");
                }
            }
            Ok(0)
        }
        "experience" => {
            let domain = required_flag(args, "--domain")?;
            let executor = flag(args, "--executor").unwrap_or_else(|| "opencode".to_string());
            let mode = flag(args, "--mode").unwrap_or_else(|| "parallel".to_string());
            let seed = flag(args, "--seed")
                .map(|value| value.parse::<i64>())
                .transpose()
                .map_err(|_| Failure::failed("--seed must be an integer"))?
                .unwrap_or(0);
            let limit = flag(args, "--limit")
                .map(|value| value.parse::<i64>())
                .transpose()
                .map_err(|_| Failure::failed("--limit must be an integer"))?
                .unwrap_or(20);
            let rows = dao
                .list_experiences(&domain, &executor, &mode, seed, limit)
                .await
                .map_err(|error| Failure::failed(format!("cannot list experience: {error}")))?;
            if json_out {
                print_json(&json!({
                    "target": target,
                    "read_only": true,
                    "scope": {"domain": domain, "executor": executor, "mode": mode, "seed": seed},
                    "experiences": rows.iter().map(|row| json!({
                        "experience_id": row.experience_id,
                        "lesson": row.lesson,
                        "confidence": row.confidence,
                        "manifest_hash": row.manifest_hash,
                        "created_at": row.created_at,
                    })).collect::<Vec<_>>(),
                }))?;
            } else {
                println!(
                    "forge harness-evolution experience — domain={domain} executor={executor} mode={mode} seed={seed} target={target} (read-only)"
                );
                for row in &rows {
                    println!(
                        "  {}  confidence {}  {}",
                        row.experience_id,
                        row.confidence,
                        short(&row.lesson, 100)
                    );
                }
                if rows.is_empty() {
                    println!("  no experience in this scope");
                }
            }
            Ok(0)
        }
        other => Err(Failure::failed(format!(
            "unknown harness-evolution command `{other}`; try status|revisions|experiments|experience"
        ))),
    }
}

pub fn usage() {
    eprintln!("usage: cargo run -p cli -- forge harness-evolution <status|revisions|experiments|experience> [options]");
    eprintln!();
    eprintln!("  Read-only operator surface for Harness Evolution (migration 294 tables).");
    eprintln!("  status      active harness revision per domain + revision count");
    eprintln!("  revisions   [--domain D] immutable revisions");
    eprintln!("  experiments --domain D  propose/probe/compose experiments");
    eprintln!("  experience  --domain D [--executor E] [--mode parallel|sequential] [--seed N] [--limit N]");
    eprintln!("  All: [--format json]. APP_ENV picks the database; the header names it.");
}

fn revision_json(row: &ForgeHarnessRevisionRow) -> Value {
    json!({
        "revision_id": row.revision_id,
        "parent_revision_id": row.parent_revision_id,
        "domain": row.domain,
        "status": row.status,
        "manifest_hash": row.manifest_hash,
        "created_at": row.created_at,
        "created_by": row.created_by,
    })
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|value| value == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn required_flag(args: &[String], name: &str) -> Result<String, Failure> {
    flag(args, name).ok_or_else(|| Failure::failed(format!("missing required {name}")))
}

fn wants_json(args: &[String]) -> bool {
    args.iter().any(|value| value == "--format")
        && args
            .iter()
            .position(|value| value == "--format")
            .and_then(|index| args.get(index + 1))
            .map(|value| value == "json")
            .unwrap_or(false)
}

fn short(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        text.chars().take(limit.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn print_json(value: &Value) -> Result<(), Failure> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| Failure::failed(format!("cannot serialize JSON: {error}")))?
    );
    Ok(())
}
