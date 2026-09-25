use crate::{Database, DbFailure, DbResult};
use serde_json::Value;

#[derive(Clone)]
pub struct SecurityAuditDao {
    db: Database,
}

impl SecurityAuditDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn record(
        &self,
        app_user_id: Option<&str>,
        event_type: &str,
        authentication_method: &str,
        metadata: &Value,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into security_audit_event (
                app_user_id,
                event_type,
                authentication_method,
                metadata
            ) values ($1::uuid, $2, $3, $4)
            "#,
        )
        .bind(app_user_id)
        .bind(event_type)
        .bind(authentication_method)
        .bind(metadata)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("security_audit.record", &error))?;
        Ok(())
    }
}
