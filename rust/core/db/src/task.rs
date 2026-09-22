use crate::{Database, DbFailure, DbResult};
use domain::TaskCompletion;

#[derive(Clone)]
pub struct TaskDao {
    db: Database,
}

impl TaskDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn complete(&self, task_id: &str) -> DbResult<Option<TaskCompletion>> {
        let id = sqlx::query_scalar::<_, String>(
            r#"
            update task
            set status = 'completed',
                completed_at = now(),
                updated_at = now()
            where id = $1::uuid
              and status = 'open'
            returning id::text
            "#,
        )
        .bind(task_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("task.complete", &error))?;

        Ok(id.map(|task_id| TaskCompletion {
            task_id,
            status: "completed".into(),
        }))
    }
}
