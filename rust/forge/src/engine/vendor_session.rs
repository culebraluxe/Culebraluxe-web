//! Shared Forge database session and vendor-session repository access.
//!
//! This module owns process-lifetime access to the one Rust Database pool. It does
//! not expose raw SQL; Forge persistence verbs live in db repositories.

use db::{Database, ForgeEngineDao};
use std::sync::OnceLock;
use tokio::runtime::Runtime;

struct SharedDb {
    db: Database,
    rt: Runtime,
}

static SHARED: OnceLock<Result<SharedDb, String>> = OnceLock::new();

fn shared() -> Result<&'static SharedDb, String> {
    let slot = SHARED.get_or_init(|| {
        let rt = Runtime::new().map_err(|error| format!("tokio: {error}"))?;
        let db = rt
            .block_on(async {
                match db::shared::get() {
                    Some(db) => Ok(db),
                    None => db::shared::get_or_connect().await,
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(SharedDb { db, rt })
    });
    slot.as_ref().map_err(Clone::clone)
}

pub fn with_shared<R>(f: impl FnOnce(&Database, &Runtime) -> R) -> Result<R, String> {
    let shared = shared()?;
    Ok(f(&shared.db, &shared.rt))
}

/// Target URL presence for operator diagnostics only. No connection is opened here.
pub fn database_url() -> Option<String> {
    ["DATABASE_URL_PROD", "DATABASE_URL_DEV", "DATABASE_URL"]
        .iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

pub fn read_vendor_session_id(story_id: &str, lane: &str) -> Result<Option<String>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.vendor_session_id(story_id, lane)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn write_vendor_session_id(
    story_id: &str,
    lane: &str,
    session_id: Option<&str>,
) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.write_vendor_session_id(story_id, lane, session_id)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}
