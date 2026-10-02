//! The spend meter: what a model turn cost, read from the harness's own session store.
//!
//! Port of `agent-runtime/harness-usage.ts`, which the Rust port dropped. Without it nothing wrote
//! `storyboard_story_run.tokens_input`/`tokens_output`/`cost_usd`: every run read `cost_source='none'` and the burn
//! was invisible (`forge:roi` — "cost captured on 0/717").
//!
//! OpenCode keeps per-session totals in its SQLite store (`session.tokens_input`, `tokens_output`, `cost`). Those
//! are the vendor's readings as the harness received them — never a model's self-report. They are read with the
//! system `sqlite3` in `-readonly` mode, so the store is never written and no SQLite crate enters the build.
//!
//! What one turn spent:
//! - a NEW session is the first one created in this lane's directory at or after the launch instant. Matching the
//!   directory as well as the time is what keeps another tool's session (Cline writes to the same store) from
//!   being claimed, and "at or after" is what kept the previous lane's session from being double-counted
//!   (2026-09-13, legacy test "a session that started BEFORE the launch is never claimed").
//! - a RESUMED session (`--session <id>` or `--continue`) is charged the DIFFERENCE across the turn, because its
//!   totals are cumulative for the whole generation.
//! - child sessions (sub-agents, `parent_id`) are part of their root's spend.
//!
//! Every failure to read is `None` — unmeasured — never a fabricated zero.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct HarnessUsage {
    pub session_id: String,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost_usd: f64,
}

impl HarnessUsage {
    /// One run is several attempts; their spend is the sum.
    pub fn absorb(&mut self, other: &HarnessUsage) {
        self.tokens_input += other.tokens_input;
        self.tokens_output += other.tokens_output;
        self.cost_usd += other.cost_usd;
    }
}

/// The store OpenCode writes: `FORGE_OPENCODE_DB` when set, else `$XDG_DATA_HOME/opencode/opencode.db`, else
/// `~/.local/share/opencode/opencode.db`.
pub fn opencode_db_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FORGE_OPENCODE_DB").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(data).join("opencode").join("opencode.db"));
    }
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db")
    })
}

/// A resumed session's own spend across one turn. Saturating: a session rewritten underneath us (compaction,
/// revert) can never report negative spend. No baseline means the measured totals stand.
pub fn usage_delta(after: &HarnessUsage, before: Option<&HarnessUsage>) -> HarnessUsage {
    let Some(before) = before else {
        return after.clone();
    };
    HarnessUsage {
        session_id: after.session_id.clone(),
        tokens_input: (after.tokens_input - before.tokens_input).max(0),
        tokens_output: (after.tokens_output - before.tokens_output).max(0),
        cost_usd: (after.cost_usd - before.cost_usd).max(0.0),
    }
}

/// The directory spellings OpenCode may have stored for this lane's working directory (as given, and resolved:
/// `/var/…` is `/private/var/…` on macOS).
pub fn directory_spellings(cwd: &str) -> Vec<String> {
    let mut out = vec![cwd.trim_end_matches('/').to_string()];
    if let Ok(real) = std::fs::canonicalize(cwd) {
        let real = real.to_string_lossy().trim_end_matches('/').to_string();
        if !out.contains(&real) {
            out.push(real);
        }
    }
    out
}

/// A session id is interpolated into SQL, so only the shape OpenCode issues is accepted.
fn is_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// A SQL string literal. Only ever given paths this engine chose, never model-authored text.
fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn query(db: &Path, sql: &str) -> Option<Vec<serde_json::Value>> {
    if !db.exists() {
        return None;
    }
    let out = Command::new("sqlite3")
        .arg("-readonly")
        .arg("-json")
        .arg(db)
        .arg(sql)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if text.trim().is_empty() {
        // sqlite3 prints nothing at all for an empty result.
        return Some(vec![]);
    }
    serde_json::from_str::<Vec<serde_json::Value>>(&text).ok()
}

/// A session's cumulative spend, its child sessions included. `None` when the session is not in the store.
pub fn session_usage(db: &Path, session_id: &str) -> Option<HarnessUsage> {
    if !is_session_id(session_id) {
        return None;
    }
    let id = literal(session_id);
    let rows = query(
        db,
        &format!(
            "with recursive tree(id) as (select id from session where id = {id} \
             union all select s.id from session s join tree t on s.parent_id = t.id) \
             select count(*) as n, coalesce(sum(tokens_input), 0) as tokens_input, \
             coalesce(sum(tokens_output), 0) as tokens_output, coalesce(sum(cost), 0) as cost \
             from session where id in (select id from tree)"
        ),
    )?;
    let row = rows.first()?;
    if row["n"].as_i64().unwrap_or(0) == 0 {
        return None;
    }
    Some(HarnessUsage {
        session_id: session_id.to_string(),
        tokens_input: row["tokens_input"].as_i64().unwrap_or(0),
        tokens_output: row["tokens_output"].as_i64().unwrap_or(0),
        cost_usd: row["cost"].as_f64().unwrap_or(0.0),
    })
}

/// The session `--continue` will resume in this directory: the most recently updated root session.
pub fn latest_session_in(db: &Path, directories: &[String]) -> Option<String> {
    session_id_where(db, directories, "1 = 1", "time_updated desc")
}

/// The first root session created in this directory at or after `launch_ms` — the session a fresh turn opened.
pub fn first_session_since(db: &Path, directories: &[String], launch_ms: i64) -> Option<String> {
    session_id_where(
        db,
        directories,
        &format!("time_created >= {launch_ms}"),
        "time_created asc",
    )
}

fn session_id_where(
    db: &Path,
    directories: &[String],
    filter: &str,
    order: &str,
) -> Option<String> {
    if directories.is_empty() {
        return None;
    }
    let dirs = directories
        .iter()
        .map(|dir| literal(dir))
        .collect::<Vec<_>>()
        .join(", ");
    let rows = query(
        db,
        &format!(
            "select id from session where parent_id is null and directory in ({dirs}) and {filter} \
             order by {order} limit 1"
        ),
    )?;
    rows.first()?["id"].as_str().map(str::to_string)
}

/// What one OpenCode turn is about to resume, read BEFORE the turn so its spend can be taken as a difference.
pub struct UsageBaseline {
    db: Option<PathBuf>,
    directories: Vec<String>,
    launch_ms: i64,
    resumed: Option<(String, Option<HarnessUsage>)>,
}

impl UsageBaseline {
    pub fn before_turn(cwd: &str, session: Option<&str>, continue_session: bool) -> Self {
        let db = opencode_db_path();
        let directories = directory_spellings(cwd);
        let launch_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0);
        let resumed_id = match (db.as_deref(), session) {
            (_, Some(id)) => Some(id.to_string()),
            (Some(db), None) if continue_session => latest_session_in(db, &directories),
            _ => None,
        };
        let resumed = resumed_id.map(|id| {
            let before = db.as_deref().and_then(|db| session_usage(db, &id));
            (id, before)
        });
        Self {
            db,
            directories,
            launch_ms,
            resumed,
        }
    }

    /// What the turn spent. `None` is "unmeasured", which the run row keeps as `cost_source='none'`.
    pub fn after_turn(&self) -> Option<HarnessUsage> {
        let db = self.db.as_deref()?;
        match &self.resumed {
            Some((id, before)) => {
                session_usage(db, id).map(|after| usage_delta(&after, before.as_ref()))
            }
            None => {
                let id = first_session_since(db, &self.directories, self.launch_ms)?;
                session_usage(db, &id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Ported from `legacy/agent-runtime/harness-usage.test.ts`, against a REAL temp SQLite file through the real
    //! `sqlite3` reader, because the query is the thing most likely to break.
    use super::*;

    fn store(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-usage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let db = dir.join("opencode.db");
        let sql = "create table session (id text primary key, parent_id text, directory text not null, \
                   time_created integer not null, time_updated integer not null, \
                   tokens_input integer default 0 not null, tokens_output integer default 0 not null, \
                   cost real default 0 not null);";
        let ok = Command::new("sqlite3")
            .arg(&db)
            .arg(sql)
            .status()
            .expect("sqlite3 runs")
            .success();
        assert!(ok, "fixture store");
        db
    }

    fn insert(db: &Path, rows: &str) {
        let ok = Command::new("sqlite3")
            .arg(db)
            .arg(format!(
                "insert into session (id, parent_id, directory, time_created, time_updated, tokens_input, \
                 tokens_output, cost) values {rows};"
            ))
            .status()
            .expect("sqlite3 runs")
            .success();
        assert!(ok, "fixture rows");
    }

    #[test]
    fn a_fresh_turn_is_the_first_session_in_its_own_directory_after_launch() {
        let db = store("fresh");
        insert(
            &db,
            "('ses_previous', null, '/work/lane', 900, 900, 5000, 500, 9.0), \
             ('ses_cline', null, '/elsewhere', 1100, 1100, 7000, 700, 7.0), \
             ('ses_target', null, '/work/lane', 1200, 1300, 26714, 701, 0.005352), \
             ('ses_child', 'ses_target', '/work/lane', 1250, 1250, 100, 10, 0.001), \
             ('ses_later', null, '/work/lane', 5000, 5000, 1, 1, 1.0)",
        );
        let dirs = vec!["/work/lane".to_string()];
        let id = first_session_since(&db, &dirs, 1000).expect("the turn's own session");
        assert_eq!(
            id, "ses_target",
            "not the previous lane's, not another tool's"
        );
        let usage = session_usage(&db, &id).expect("measured");
        assert_eq!(
            usage.tokens_input, 26_814,
            "the sub-agent's session is part of the turn"
        );
        assert_eq!(usage.tokens_output, 711);
        assert!((usage.cost_usd - 0.006352).abs() < 1e-9);
        assert_eq!(
            first_session_since(&db, &dirs, 6000),
            None,
            "nothing after: unmeasured"
        );
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }

    #[test]
    fn a_resumed_session_is_charged_its_delta_not_its_lifetime() {
        let before = HarnessUsage {
            session_id: "ses_gen".into(),
            tokens_input: 100,
            tokens_output: 20,
            cost_usd: 0.01,
        };
        let after = HarnessUsage {
            session_id: "ses_gen".into(),
            tokens_input: 260,
            tokens_output: 55,
            cost_usd: 0.031,
        };
        let delta = usage_delta(&after, Some(&before));
        assert_eq!(delta.tokens_input, 160);
        assert_eq!(delta.tokens_output, 35);
        assert!((delta.cost_usd - 0.021).abs() < 1e-9);
        let rewritten = usage_delta(&before, Some(&after));
        assert_eq!(
            (rewritten.tokens_input, rewritten.cost_usd),
            (0, 0.0),
            "never negative"
        );
        assert_eq!(
            usage_delta(&after, None),
            after,
            "no baseline: the totals stand"
        );
    }

    #[test]
    fn a_missing_store_or_a_hostile_id_is_unmeasured() {
        let missing = Path::new("/definitely/not/here/opencode.db");
        assert_eq!(session_usage(missing, "ses_x"), None);
        let db = store("hostile");
        insert(&db, "('ses_a', null, '/w', 1, 1, 1, 1, 1.0)");
        assert_eq!(session_usage(&db, "ses_a' or '1'='1"), None);
        assert_eq!(
            session_usage(&db, "ses_absent"),
            None,
            "absent is unmeasured, not zero"
        );
        let _ = std::fs::remove_dir_all(db.parent().unwrap());
    }
}
