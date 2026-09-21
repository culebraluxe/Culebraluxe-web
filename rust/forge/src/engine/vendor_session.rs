//! `forge_vendor_session` — existing Neon table. Same SQL as `db/forge-vendor-session.ts`.
//! Queries go through `db::Database` (the one Rust pool). The `psql` CLI is gone.

use db::Database;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub const READ_VENDOR_SESSION: &str = "\
SELECT session_id FROM forge_vendor_session
 WHERE story_id = $1 AND worker_id = $2
 LIMIT 1";

pub const WRITE_VENDOR_SESSION: &str = "\
INSERT INTO forge_vendor_session (story_id, worker_id, session_id, updated_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (story_id, worker_id)
DO UPDATE SET session_id = excluded.session_id, updated_at = now()";

pub const CLEAR_VENDOR_SESSION: &str = "\
INSERT INTO forge_vendor_session (story_id, worker_id, session_id, updated_at)
VALUES ($1, $2, NULL, now())
ON CONFLICT (story_id, worker_id)
DO UPDATE SET session_id = NULL, updated_at = now()";

struct SharedDb {
    db: Database,
    rt: Runtime,
}

static SHARED: OnceLock<Result<SharedDb, String>> = OnceLock::new();

fn shared() -> Result<&'static SharedDb, String> {
    let slot = SHARED.get_or_init(|| {
        let rt = Runtime::new().map_err(|e| format!("tokio: {e}"))?;
        let db = rt
            .block_on(Database::connect_from_env())
            .map_err(|e| e.to_string())?;
        Ok(SharedDb { db, rt })
    });
    slot.as_ref().map_err(|e| e.clone())
}

/// Borrow the shared database and runtime.
///
/// The shared `OnceLock` session exists so a Forge step does not open a pool per call. Anything that used to go
/// through `psql_query` needs the same guarantee, and needs it WITH BINDS: the old helper took a finished SQL string,
/// which is why every caller built one with `format!` and escaped it by hand with `sql_literal`. Passing a closure
/// keeps the session behavior identical while letting the caller use `$1`-style binds and let the driver do the
/// escaping.
pub fn with_shared<R>(f: impl FnOnce(&Database, &Runtime) -> R) -> Result<R, String> {
    let shared = shared()?;
    Ok(f(&shared.db, &shared.rt))
}

/// Target URL presence for logs. Does not open a second client.
pub fn database_url() -> Option<String> {
    ["DATABASE_URL_PROD", "DATABASE_URL_DEV", "DATABASE_URL"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|s| !s.trim().is_empty()))
}

pub fn psql_query(sql: &str) -> Result<String, String> {
    let s = shared()?;
    s.rt.block_on(s.db.run_text(sql)).map_err(|e| e.to_string())
}

pub fn sql_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn read_vendor_session_id(story_id: &str, lane: &str) -> Result<Option<String>, String> {
    let sql = format!(
        "SELECT session_id FROM forge_vendor_session WHERE story_id = {} AND worker_id = {} LIMIT 1",
        sql_literal(story_id),
        sql_literal(lane)
    );
    let raw = psql_query(&sql)?;
    if raw.is_empty() || raw == "\\N" {
        Ok(None)
    } else {
        Ok(Some(raw))
    }
}

pub fn write_vendor_session_id(
    story_id: &str,
    lane: &str,
    session_id: Option<&str>,
) -> Result<(), String> {
    let sid = match session_id {
        Some(id) => sql_literal(id),
        None => "NULL".into(),
    };
    let sql = format!(
        "INSERT INTO forge_vendor_session (story_id, worker_id, session_id, updated_at) \
         VALUES ({}, {}, {sid}, now()) \
         ON CONFLICT (story_id, worker_id) \
         DO UPDATE SET session_id = excluded.session_id, updated_at = now()",
        sql_literal(story_id),
        sql_literal(lane)
    );
    let _ = psql_query(&sql)?;
    Ok(())
}
