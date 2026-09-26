use crate::{DbFailure, DbResult};
use sqlx::{PgConnection, Postgres, Transaction};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::OwnedMutexGuard;

pub struct DbTransaction {
    inner: Option<Transaction<'static, Postgres>>,
    scoped: Option<OwnedMutexGuard<Option<DbTransaction>>>,
    rollback_only: Option<Arc<AtomicBool>>,
    operation: &'static str,
}

impl DbTransaction {
    pub(crate) fn new(inner: Transaction<'static, Postgres>, operation: &'static str) -> Self {
        Self {
            inner: Some(inner),
            scoped: None,
            rollback_only: None,
            operation,
        }
    }

    pub(crate) fn scoped(
        scoped: OwnedMutexGuard<Option<DbTransaction>>,
        rollback_only: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner: None,
            scoped: Some(scoped),
            rollback_only: Some(rollback_only),
            operation: "service.mutation.nested",
        }
    }

    /// Workflow kernel (and other core crates) run SQL on the shared transaction.
    pub fn connection(&mut self) -> &mut PgConnection {
        match &mut self.scoped {
            Some(scoped) => scoped
                .as_mut()
                .expect("active mutation transaction")
                .connection_ref(),
            None => self
                .inner
                .as_mut()
                .expect("database transaction already finalized")
                .as_mut(),
        }
    }

    pub(crate) fn connection_ref(&mut self) -> &mut PgConnection {
        self.inner
            .as_mut()
            .expect("database transaction already finalized")
            .as_mut()
    }

    pub(crate) fn connection_const(&self) -> &PgConnection {
        &**self
            .inner
            .as_ref()
            .expect("database transaction already finalized")
    }

    pub async fn commit(mut self) -> DbResult<()> {
        if self.scoped.take().is_some() {
            self.rollback_only.take();
            return Ok(());
        }
        let transaction = self
            .inner
            .take()
            .expect("database transaction already finalized");
        transaction
            .commit()
            .await
            .map_err(|error| DbFailure::from_sqlx(self.operation, &error))
    }

    pub async fn rollback(mut self) -> DbResult<()> {
        if self.scoped.take().is_some() {
            if let Some(rollback_only) = self.rollback_only.take() {
                rollback_only.store(true, Ordering::Release);
            }
            return Ok(());
        }
        let transaction = self
            .inner
            .take()
            .expect("database transaction already finalized");
        transaction
            .rollback()
            .await
            .map_err(|error| DbFailure::from_sqlx(self.operation, &error))
    }
}

impl Drop for DbTransaction {
    fn drop(&mut self) {
        if self.scoped.is_some() {
            if let Some(rollback_only) = &self.rollback_only {
                rollback_only.store(true, Ordering::Release);
            }
        }
    }
}
