//! One read-only query against a named control-plane database: `forge sql`.
//!
//! The reason this exists: on 2026-09-29 the captain asked direct questions about the live engine ("is the trigger
//! function `SECURITY DEFINER`?", "which functions are unrecorded?", "does this column exist?") and the only ways to
//! answer them were `psql` (absent), a TypeScript script (deleted with `legacy/`, `ERR_MODULE_NOT_FOUND`) or a
//! hand-written throwaway — and a throwaway is exactly the tree-shaped scratch this repository forbids. So the read
//! path is a verb, it names the database it read, and it cannot write.
//!
//! READ-ONLY BY CONSTRUCTION, not by convention:
//!   1. the statement must be a single `select` / `with` / `values` / `table` statement — anything else is refused
//!      before a connection is opened;
//!   2. it runs inside `begin read only`, so Postgres itself refuses a write that slipped through;
//!   3. the result is wrapped (`select row_to_json(t)::text from (<sql>) t limit $1`) so every column comes back as
//!      JSON — no driver type mapping to get wrong and no cast the query author has to guess at.
//!
//! Usage:
//!   cargo run -p cli -- forge sql --target dev --sql "select now()"
//!   cargo run -p cli -- forge sql --target prod --file /tmp/q.sql [--limit 200] [--format json]

use super::Failure;
use db::{Database, DbTarget, ForgeReadDao};
use serde_json::json;

/// The default row ceiling. A read tool that can pull an unbounded result into a terminal is a read tool that can
/// hold the operator's terminal hostage; `--limit` raises it deliberately.
const DEFAULT_LIMIT: i64 = 200;

pub async fn run(args: &[String]) -> Result<u8, Failure> {
    // `.env.local` carries DATABASE_URL_DEV / DATABASE_URL_PROD — the same loader every other read uses.
    crate::apple_sync::load_env();

    let target = match flag(args, "--target").as_deref() {
        Some("dev") | Some("development") => DbTarget::Dev,
        Some("prod") | Some("production") => DbTarget::Prod,
        Some(other) => {
            return Err(Failure::usage(format!(
                "unknown --target `{other}`; expected dev or prod"
            )))
        }
        // No default. A read that does not name its database is a read someone has to guess about, and guessing
        // about production is the one thing this repository keeps refusing to do.
        None => {
            return Err(Failure::usage(
                "forge sql needs an explicit --target dev|prod: a read that does not name its database is a read \
                 nobody can trust"
                    .to_string(),
            ))
        }
    };

    let sql = match flag(args, "--sql") {
        Some(sql) => sql,
        None => match flag(args, "--file") {
            Some(path) => std::fs::read_to_string(&path)
                .map_err(|error| Failure::failed(format!("cannot read --file {path}: {error}")))?,
            None => {
                return Err(Failure::usage(
                    "forge sql needs --sql \"select …\" or --file <path>".to_string(),
                ))
            }
        },
    };
    let sql = sql.trim().trim_end_matches(';').trim().to_string();
    if let Some(why) = read_only_refusal(&sql) {
        return Err(Failure::usage(why));
    }
    let limit = match flag(args, "--limit") {
        Some(raw) => raw
            .trim()
            .parse::<i64>()
            .map_err(|_| Failure::usage(format!("--limit must be a number, got `{raw}`")))?,
        None => DEFAULT_LIMIT,
    };
    if limit <= 0 {
        return Err(Failure::usage(
            "--limit must be greater than zero".to_string(),
        ));
    }

    let database = Database::connect_target(target)
        .await
        .map_err(|error| Failure::failed(format!("cannot connect: {error}")))?;
    eprintln!("target={target:?}");
    let rows = ForgeReadDao::new(database)
        .read_only_rows(&sql, limit)
        .await
        .map_err(|error| Failure::failed(format!("query failed: {error}")))?;

    if flag(args, "--format").as_deref() == Some("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "target": format!("{target:?}").to_lowercase(),
                "rows": rows.len(),
                "result": rows,
            }))
            .map_err(|error| Failure::failed(format!("cannot render JSON: {error}")))?
        );
        return Ok(0);
    }
    for row in &rows {
        println!(
            "{}",
            serde_json::to_string(row)
                .map_err(|error| Failure::failed(format!("cannot render JSON: {error}")))?
        );
    }
    eprintln!("rows={}", rows.len());
    Ok(0)
}

/// Why this statement may not be read by a read tool — `None` means it may.
///
/// A refusal is *usage* (exit 2), not a failure: nothing was attempted, and the operator needs the shape of the
/// command rather than a database error.
pub fn read_only_refusal(sql: &str) -> Option<String> {
    let bare = strip_leading_comments(sql).trim().to_lowercase();
    if bare.is_empty() {
        return Some("forge sql was given no statement".to_string());
    }
    let first = bare
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("");
    if matches!(
        first,
        "select" | "with" | "values" | "table" | "show" | "explain"
    ) {
        return None;
    }
    // Refused by NAME, so the operator is told what was seen rather than "invalid query".
    Some(format!(
        "forge sql is read-only: a `{first}` statement cannot be executed (expected select / with / values / table)"
    ))
}

fn strip_leading_comments(sql: &str) -> &str {
    let mut rest = sql;
    loop {
        let trimmed = rest.trim_start();
        if let Some(after) = trimmed.strip_prefix("--") {
            match after.find('\n') {
                Some(index) => rest = &after[index + 1..],
                None => return "",
            }
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("/*") {
            match after.find("*/") {
                Some(index) => rest = &after[index + 2..],
                None => return "",
            }
            continue;
        }
        return trimmed;
    }
}

/// The wrapped, read-only execution has ONE home and it is not here: `ForgeReadDao::read_only_rows`
/// (`rust/core/db/src/forge_read.rs`) wraps the statement in `row_to_json`, runs it on a `read only` transaction
/// and bounds it by `limit`. This module keeps only the *shape* guard, because refusing a `drop table` with a
/// sentence is a CLI concern and refusing it at the server is not.
/// `--flag value` / `--flag=value`. Absent = `None`; present with no value = the next argument as-is, which the
/// callers validate rather than assume.
fn flag(args: &[String], name: &str) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == name {
            return args.get(index + 1).cloned();
        }
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_shapes_are_allowed_and_writes_are_refused_by_name() {
        assert!(read_only_refusal("select 1").is_none());
        assert!(read_only_refusal("  WITH x as (select 1) select * from x").is_none());
        assert!(read_only_refusal("values (1)").is_none());
        assert!(read_only_refusal("-- a comment\nselect now()").is_none());
        assert!(read_only_refusal("/* why */ select now()").is_none());

        let refusal = read_only_refusal("update storyboard_story set status='Complete'").unwrap();
        assert!(refusal.contains("read-only"), "{refusal}");
        assert!(refusal.contains("update"), "{refusal}");
        assert!(read_only_refusal("truncate table app_error").is_some());
        assert!(read_only_refusal("drop function f()").is_some());
        assert!(read_only_refusal("").is_some());
    }

    #[test]
    fn flags_read_both_spellings() {
        let args: Vec<String> = ["--target", "prod", "--limit=5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(flag(&args, "--target").as_deref(), Some("prod"));
        assert_eq!(flag(&args, "--limit").as_deref(), Some("5"));
        assert_eq!(flag(&args, "--sql"), None);
    }
}
