//! Blocking access to the app's one shared database pool for the synchronous workflow kernel.
//!
//! This is the re-host equivalent of Forge's `vendor_session::with_shared`, without the Forge
//! vendor-session repository: the store is synchronous and `block_on`s on its own runtime, so the
//! caller's thread must not already be driving a runtime.

use std::sync::OnceLock;

use db::Database;
use tokio::runtime::Runtime;

struct SharedDb {
    db: Database,
    rt: Runtime,
}

static SHARED: OnceLock<Result<SharedDb, String>> = OnceLock::new();

fn shared() -> Result<&'static SharedDb, String> {
    SHARED
        .get_or_init(|| {
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
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn with_shared<R>(f: impl FnOnce(&Database, &Runtime) -> R) -> Result<R, String> {
    let shared = shared()?;
    Ok(f(&shared.db, &shared.rt))
}
