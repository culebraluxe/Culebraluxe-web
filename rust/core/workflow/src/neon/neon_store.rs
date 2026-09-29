//! Moved from `neon.rs` (move only): NeonStore, shared_runtime, from_database, with_tx, NeonTx, conn, exec_q, fetch_all_q, fetch_one_q, fetch_optional_q.

#[allow(unused_imports)]
use super::*;

pub struct NeonStore {
    pub(super) db: Database,
    pub(super) rt: &'static tokio::runtime::Runtime,
}

/// The runtime every store shares.
///
/// This used to be built PER STORE with `worker_threads(1)`, which is why the engine could not overlap database work:
/// every call in the process funnelled through a single worker thread. A shared multi-threaded runtime is the smallest
/// change that removes that funnel without touching the synchronous `Store` trait - the trait still blocks its caller,
/// but the database work itself now spreads across workers instead of queueing behind one.
///
/// The larger fix is making `Store`/`TxStore` async (45 methods) so nothing blocks at all; the engine being a
/// synchronous process per call is the second half of the same problem, and the shape for that is a long-running
/// engine service rather than a spawned binary.
pub(super) fn shared_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("workflow runtime")
    })
}

impl NeonStore {
    pub fn from_database(db: Database) -> Result<Self> {
        Ok(Self {
            db,
            rt: shared_runtime(),
        })
    }

    pub fn connect_from_env() -> Result<Self> {
        let rt = shared_runtime();
        // The composition root's pool when there is one, otherwise the single pool this process connects for itself.
        // Either way this happens once: a store per engine command used to mean a POOL per engine command, which cost
        // about 2.4 seconds a call against the dev database - all of it connecting, none of it doing work.
        let db = rt
            .block_on(db::shared::get_or_connect())
            .map_err(|e| WorkflowError::generic(e.to_string()))?;
        Ok(Self { db, rt })
    }

    pub fn ping(&self) -> Result<()> {
        self.rt
            .block_on(self.db.ping())
            .map_err(|e| WorkflowError::generic(e.to_string()))
    }

    pub fn database(&self) -> &Database {
        &self.db
    }
}

impl TxStore for NeonStore {
    fn with_tx<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Store) -> Result<R>,
    {
        let mut tx = self
            .rt
            .block_on(self.db.begin("workflow.step"))
            .map_err(|e| WorkflowError::generic(e.to_string()))?;
        let result = {
            let mut store = NeonTx {
                tx: &mut tx,
                handle: self.rt.handle().clone(),
            };
            f(&mut store)
        };
        match result {
            Ok(v) => {
                self.rt
                    .block_on(tx.commit())
                    .map_err(|e| WorkflowError::generic(e.to_string()))?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.rt.block_on(tx.rollback());
                Err(e)
            }
        }
    }
}

pub(super) struct NeonTx<'a> {
    pub(super) tx: &'a mut DbTransaction,
    pub(super) handle: tokio::runtime::Handle,
}

impl NeonTx<'_> {
    pub(super) fn conn(&mut self) -> &mut PgConnection {
        self.tx.connection()
    }
}

pub(super) fn exec_q<'q>(tx: &mut NeonTx<'_>, q: Query<'q, Postgres, PgArguments>) -> Result<u64> {
    let handle = tx.handle.clone();
    let conn = tx.conn();
    handle
        .block_on(q.execute(conn))
        .map(|r| r.rows_affected())
        .map_err(|e| WorkflowError::generic(e.to_string()))
}

pub(super) fn fetch_all_q<'q>(
    tx: &mut NeonTx<'_>,
    q: Query<'q, Postgres, PgArguments>,
) -> Result<Vec<PgRow>> {
    let handle = tx.handle.clone();
    let conn = tx.conn();
    handle
        .block_on(q.fetch_all(conn))
        .map_err(|e| WorkflowError::generic(e.to_string()))
}

pub(super) fn fetch_one_q<'q>(
    tx: &mut NeonTx<'_>,
    q: Query<'q, Postgres, PgArguments>,
) -> Result<PgRow> {
    let handle = tx.handle.clone();
    let conn = tx.conn();
    handle
        .block_on(q.fetch_one(conn))
        .map_err(|e| WorkflowError::generic(e.to_string()))
}

pub(super) fn fetch_optional_q<'q>(
    tx: &mut NeonTx<'_>,
    q: Query<'q, Postgres, PgArguments>,
) -> Result<Option<PgRow>> {
    let handle = tx.handle.clone();
    let conn = tx.conn();
    handle
        .block_on(q.fetch_optional(conn))
        .map_err(|e| WorkflowError::generic(e.to_string()))
}
