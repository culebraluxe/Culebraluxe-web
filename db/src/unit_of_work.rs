//! Explicit service mutation scope. DAO connection leases and nested repository
//! transactions join one transaction; only the outer service may commit it.
//! This scope is task-local and deliberately does not escape into spawned tasks.
use crate::{Database, DbFailure, DbResult, DbTransaction};
use sqlx::{pool::PoolConnection, PgConnection, Postgres};
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone)]
pub(crate) struct MutationScope {
    pub database_id: Arc<()>,
    pub transaction: Arc<Mutex<Option<DbTransaction>>>,
    pub rollback_only: Arc<AtomicBool>,
}

tokio::task_local! { static MUTATION: MutationScope; }

pub(crate) fn current(database_id: &Arc<()>) -> Option<MutationScope> {
    MUTATION
        .try_with(|scope| Arc::ptr_eq(&scope.database_id, database_id).then(|| scope.clone()))
        .ok()
        .flatten()
}

pub enum DbConnection {
    Pool(PoolConnection<Postgres>),
    Scoped(OwnedMutexGuard<Option<DbTransaction>>),
}
impl Deref for DbConnection {
    type Target = PgConnection;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Pool(connection) => connection,
            Self::Scoped(tx) => tx
                .as_ref()
                .expect("active mutation transaction")
                .connection_const(),
        }
    }
}
impl DerefMut for DbConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Pool(connection) => connection,
            Self::Scoped(tx) => tx
                .as_mut()
                .expect("active mutation transaction")
                .connection(),
        }
    }
}

impl Database {
    /// Lease the active mutation's connection, otherwise use the shared pool.
    pub async fn connection(&self) -> DbResult<DbConnection> {
        if let Some(scope) = current(&self.identity) {
            let guard = scope.transaction.lock_owned().await;
            if guard.is_none() || scope.rollback_only.load(Ordering::Acquire) {
                return Err(DbFailure::configuration(
                    "service.transaction",
                    "Mutation transaction has been rolled back.",
                ));
            }
            return Ok(DbConnection::Scoped(guard));
        }
        self.pool()
            .acquire()
            .await
            .map(DbConnection::Pool)
            .map_err(|e| DbFailure::from_sqlx("db.acquire", &e))
    }

    pub fn in_mutation(&self) -> bool {
        current(&self.identity).is_some()
    }

    pub async fn mutation<T, E, F>(&self, work: F) -> Result<T, E>
    where
        E: From<DbFailure>,
        F: Future<Output = Result<T, E>>,
    {
        if let Some(scope) = current(&self.identity) {
            let result = work.await;
            if result.is_err() {
                scope.rollback_only.store(true, Ordering::Release);
            }
            return result;
        }
        let transaction = self.begin("service.mutation").await?;
        let scope = MutationScope {
            database_id: self.identity.clone(),
            transaction: Arc::new(Mutex::new(Some(transaction))),
            rollback_only: Arc::new(AtomicBool::new(false)),
        };
        let result = MUTATION.scope(scope.clone(), work).await;
        let transaction = scope.transaction.lock().await.take();
        match result {
            Ok(value) if !scope.rollback_only.load(Ordering::Acquire) => {
                let tx = transaction.ok_or_else(|| {
                    DbFailure::configuration(
                        "service.mutation",
                        "Mutation transaction was already rolled back.",
                    )
                })?;
                tx.commit().await?;
                Ok(value)
            }
            result => {
                if let Some(tx) = transaction {
                    tx.rollback().await?;
                }
                match result {
                    Err(error) => Err(error),
                    Ok(_) => Err(DbFailure::configuration(
                        "service.mutation",
                        "Nested operation failed; mutation was rolled back.",
                    )
                    .into()),
                }
            }
        }
    }
}

/// Test repositories have no database. Production repositories always supply it.
pub async fn service_mutation<T, E, F>(database: Option<Database>, work: F) -> Result<T, E>
where
    E: From<DbFailure>,
    F: Future<Output = Result<T, E>>,
{
    match database {
        Some(database) => database.mutation(work).await,
        None => work.await,
    }
}
