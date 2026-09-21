//! The process-wide database pool.
//!
//! WHY THIS EXISTS. `Database` is cheap to clone - it is a pool handle, not a pool - so the right number of pools in a
//! process is one, owned by whoever composes the process. Without a shared slot, every consumer that could not be
//! handed the pool on a constructor connected its own: the engine's store, the Forge vendor-session helper, the error
//! capture sink. Three pools against the same database, each paying its own TLS handshake and authentication, and each
//! with its own idea of how many connections are in flight.
//!
//! That was not a theoretical inefficiency. An engine command used to build a store, and therefore a pool, per call:
//! measured against the dev database, ten sequential engine commands took 23699ms - about 2.4 seconds each, flat, with
//! no warmup improvement, because every one of them connected from scratch.
//!
//! The composition root installs its pool here at boot. Anything else asks for it. The slot is first-install-wins,
//! which is the right bias for a process: the root gets there first, and a component that arrives later cannot replace
//! the composition root's pool with one of its own.

use std::sync::OnceLock;

use crate::error::DbResult;
use crate::pool::Database;

static SHARED: OnceLock<Database> = OnceLock::new();

/// Install the pool this process will use. Returns false if one was already installed, which is not an error: the
/// composition root is first, and a second install means someone built a pool they did not need.
pub fn install(db: Database) -> bool {
    SHARED.set(db).is_ok()
}

/// The installed pool, if the composition root has run.
pub fn get() -> Option<Database> {
    SHARED.get().cloned()
}

pub fn is_installed() -> bool {
    SHARED.get().is_some()
}

/// The installed pool, or connect one and install it.
///
/// The fallback exists for binaries that are not the server - the CLI and the task tools - which legitimately have no
/// composition root to install anything. Two threads racing here could both connect, and the loser's pool is dropped;
/// that is a wasted connection rather than a correctness problem, and `OnceLock` has no fallible initializer on stable.
pub async fn get_or_connect() -> DbResult<Database> {
    if let Some(db) = get() {
        return Ok(db);
    }
    let db = Database::connect_from_env().await?;
    let _ = SHARED.set(db.clone());
    Ok(db)
}
