use crate::{DbFailure, DbResult};
use sqlx::{PgConnection, Postgres, Transaction};

pub struct DbTransaction {
    inner: Option<Transaction<'static, Postgres>>,
    operation: &'static str,
}

impl DbTransaction {
    pub(crate) fn new(inner: Transaction<'static, Postgres>, operation: &'static str) -> Self {
        Self {
            inner: Some(inner),
            operation,
        }
    }

    /// Workflow kernel (and other core crates) run SQL on the shared transaction.
    pub fn connection(&mut self) -> &mut PgConnection {
        self.inner
            .as_mut()
            .expect("database transaction already finalized")
            .as_mut()
    }

    pub async fn commit(mut self) -> DbResult<()> {
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
