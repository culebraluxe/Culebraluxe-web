use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    AppleReminderCommandReceipt, AppleReminderLanding, AppleReminderUpsertRequest,
    CreateWbsItemRequest, SaveWbsItemRequest, WbsCategory, WbsDependency, WbsEntityLink,
    WbsEntityType, WbsItem, WbsStatus,
};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct WbsRow {
    id: String,
    project_id: Option<String>,
    parent_id: Option<String>,
    title: String,
    notes: String,
    category: String,
    status: String,
    due_at: Option<DateTime<Utc>>,
    planned_start: Option<NaiveDate>,
    planned_finish: Option<NaiveDate>,
    owner: Option<String>,
    sort_order: Option<i32>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DependencyRow {
    project_id: String,
    source_id: String,
    target_id: String,
    kind: String,
}

impl From<DependencyRow> for WbsDependency {
    fn from(value: DependencyRow) -> Self {
        Self {
            project_id: value.project_id,
            source_id: value.source_id,
            target_id: value.target_id,
            kind: value.kind,
        }
    }
}

fn map_row(row: WbsRow) -> DbResult<WbsItem> {
    let category = WbsCategory::try_from(row.category.as_str())
        .map_err(|error| DbFailure::schema_mismatch("wbs.map", error.to_string()))?;
    let status = WbsStatus::try_from(row.status.as_str())
        .map_err(|error| DbFailure::schema_mismatch("wbs.map", error))?;
    let entity = match (row.entity_type.as_deref(), row.entity_id) {
        (Some(entity_type), Some(id)) => Some(WbsEntityLink {
            entity_type: WbsEntityType::try_from(entity_type)
                .map_err(|error| DbFailure::schema_mismatch("wbs.map", error))?,
            id,
        }),
        _ => None,
    };

    Ok(WbsItem {
        id: row.id,
        title: row.title,
        notes: row.notes,
        category,
        status,
        project_id: row.project_id,
        parent_id: row.parent_id,
        due_at: row.due_at.map(|value| value.to_rfc3339()),
        planned_start: row.planned_start.map(|value| value.to_string()),
        planned_finish: row.planned_finish.map(|value| value.to_string()),
        owner: row.owner,
        order: row.sort_order,
        entity,
        created_at: Some(row.created_at.to_rfc3339()),
        updated_at: Some(row.updated_at.to_rfc3339()),
    })
}

macro_rules! wbs_sql {
    ($suffix:literal) => {
        concat!(
            "select id, project_id, parent_id, title, notes, category, status, ",
            "due_at, planned_start, planned_finish, owner, sort_order, entity_type, entity_id, created_at, updated_at ",
            "from wbs_item ",
            $suffix
        )
    };
}

#[derive(Clone)]
pub struct WbsDao {
    db: Database,
}

impl WbsDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn list_dependencies(&self, project_id: &str) -> DbResult<Vec<WbsDependency>> {
        let rows = sqlx::query_as::<_, DependencyRow>(
            "select project_id, source_id, target_id, kind from wbs_dependency \
             where project_id = $1 order by source_id, target_id",
        )
        .bind(project_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.dependencies.list", &error))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Called within WbsService's mutation. The lock serializes graph checks
    /// against concurrent links in this project until that mutation commits.
    pub async fn lock_dependency_project(&self, project_id: &str) -> DbResult<()> {
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 69022))")
            .bind(project_id)
            .execute(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("wbs.dependencies.lock", &error))?;
        Ok(())
    }

    pub async fn insert_dependency(&self, edge: &WbsDependency) -> DbResult<WbsDependency> {
        let row = sqlx::query_as::<_, DependencyRow>(
            "insert into wbs_dependency(project_id, source_id, target_id, kind) \
             values($1,$2,$3,$4) \
             returning project_id, source_id, target_id, kind",
        )
        .bind(&edge.project_id)
        .bind(&edge.source_id)
        .bind(&edge.target_id)
        .bind(&edge.kind)
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.dependencies.insert", &error))?;
        Ok(row.into())
    }

    pub async fn delete_dependency(&self, project_id: &str, source_id: &str, target_id: &str) -> DbResult<bool> {
        let result = sqlx::query(
            "delete from wbs_dependency where project_id=$1 and source_id=$2 and target_id=$3",
        )
        .bind(project_id)
        .bind(source_id)
        .bind(target_id)
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.dependencies.delete", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get(&self, id: &str) -> DbResult<Option<WbsItem>> {
        let row = sqlx::query_as::<_, WbsRow>(wbs_sql!("where id = $1 limit 1"))
            .bind(id)
            .fetch_optional(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("wbs.get", &error))?;
        row.map(map_row).transpose()
    }

    pub async fn list_due(&self, category: Option<WbsCategory>) -> DbResult<Vec<WbsItem>> {
        let rows = sqlx::query_as::<_, WbsRow>(wbs_sql!(
            "where status in ('open','doing')
             and ($1::text is null or category = $1)
             order by due_at nulls last, id"
        ))
        .bind(category.as_ref().map(WbsCategory::as_str))
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.list_due", &error))?;
        rows.into_iter().map(map_row).collect()
    }

    pub async fn list_project_items(&self) -> DbResult<Vec<WbsItem>> {
        let rows = sqlx::query_as::<_, WbsRow>(wbs_sql!(
            "where project_id is not null
             order by project_id, parent_id nulls first, sort_order nulls last,
                      due_at nulls last, id"
        ))
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.list_project_items", &error))?;
        rows.into_iter().map(map_row).collect()
    }

    pub async fn list_for_entity(
        &self,
        entity_type: WbsEntityType,
        id: &str,
    ) -> DbResult<Vec<WbsItem>> {
        let rows = sqlx::query_as::<_, WbsRow>(wbs_sql!(
            "where entity_type = $1 and entity_id = $2
             order by due_at nulls last, sort_order nulls last, id"
        ))
        .bind(entity_type.as_str())
        .bind(id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.list_for_entity", &error))?;
        rows.into_iter().map(map_row).collect()
    }

    pub async fn create(&self, request: &CreateWbsItemRequest) -> DbResult<WbsItem> {
        let row = sqlx::query_as::<_, WbsRow>(
            r#"
            insert into wbs_item (
                id, project_id, parent_id, title, notes, category, status,
                due_at, planned_start, planned_finish, owner, sort_order, entity_type, entity_id
            )
            values ($1,$2,$3,$4,$5,$6,'open',$7::timestamptz,$8::date,$9::date,$10,$11,$12,$13)
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, planned_start, planned_finish, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(&request.id)
        .bind(request.project_id.as_deref())
        .bind(request.parent_id.as_deref())
        .bind(request.title.trim())
        .bind(request.notes.as_deref().unwrap_or(""))
        .bind(request.category.as_str())
        .bind(request.due_at.as_deref())
        .bind(request.planned_start.as_deref())
        .bind(request.planned_finish.as_deref())
        .bind(request.owner.as_deref())
        .bind(request.order)
        .bind(
            request
                .entity
                .as_ref()
                .map(|entity| entity.entity_type.as_str()),
        )
        .bind(request.entity.as_ref().map(|entity| entity.id.as_str()))
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.create", &error))?;
        map_row(row)
    }

    pub async fn save(&self, request: &SaveWbsItemRequest) -> DbResult<Option<WbsItem>> {
        let base = &request.create;
        let row = sqlx::query_as::<_, WbsRow>(
            r#"
            update wbs_item
            set title=$2, notes=$3, category=$4, status=coalesce($5,status),
                due_at=$6::timestamptz, planned_start=$7::date, planned_finish=$8::date,
                owner=$9, sort_order=$10,
                entity_type=$11, entity_id=$12, project_id=$13, parent_id=$14,
                updated_at=now()
            where id=$1
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, planned_start, planned_finish, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(&base.id)
        .bind(base.title.trim())
        .bind(base.notes.as_deref().unwrap_or(""))
        .bind(base.category.as_str())
        .bind(request.status.as_ref().map(WbsStatus::as_str))
        .bind(base.due_at.as_deref())
        .bind(base.planned_start.as_deref())
        .bind(base.planned_finish.as_deref())
        .bind(base.owner.as_deref())
        .bind(base.order)
        .bind(
            base.entity
                .as_ref()
                .map(|entity| entity.entity_type.as_str()),
        )
        .bind(base.entity.as_ref().map(|entity| entity.id.as_str()))
        .bind(base.project_id.as_deref())
        .bind(base.parent_id.as_deref())
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.save", &error))?;
        row.map(map_row).transpose()
    }

    pub async fn upsert_apple_reminder_landing(
        &self,
        reminder: &AppleReminderLanding,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into l_reminder (
                source_account, source_message_id, external_id, list_name, title, notes,
                starts_at, due_at, completed, completed_at, priority, raw,
                first_seen_at, last_seen_at
            )
            values (
                $1,$2,$3,$4,$5,$6,$7::timestamptz,$8::timestamptz,$9,
                $10::timestamptz,$11,$12,now(),now()
            )
            on conflict ((coalesce(source_account, '')), source_message_id)
            do update set
                external_id=excluded.external_id,
                list_name=excluded.list_name,
                title=excluded.title,
                notes=excluded.notes,
                starts_at=excluded.starts_at,
                due_at=excluded.due_at,
                completed=excluded.completed,
                completed_at=excluded.completed_at,
                priority=excluded.priority,
                raw=excluded.raw,
                last_seen_at=now()
            "#,
        )
        .bind(&reminder.source_account)
        .bind(&reminder.source_message_id)
        .bind(reminder.external_id.as_deref())
        .bind(reminder.list_name.as_deref())
        .bind(reminder.title.as_deref())
        .bind(reminder.notes.as_deref())
        .bind(reminder.start_at.as_deref())
        .bind(reminder.due_at.as_deref())
        .bind(reminder.completed)
        .bind(reminder.completed_at.as_deref())
        .bind(reminder.priority)
        .bind(&reminder.raw)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("reminder.landing.upsert", &error))?;
        Ok(())
    }

    pub async fn queue_apple_reminder(
        &self,
        request: &AppleReminderUpsertRequest,
        actor_app_user_id: Option<&str>,
        correlation_id: &str,
    ) -> DbResult<AppleReminderCommandReceipt> {
        let command_id = Uuid::new_v4().to_string();
        let payload = json!({
            "wbsId": request.wbs_id.clone(),
            "title": request.title.clone(),
            "dueAt": request.due_at.clone(),
            "completed": request.completed,
            "notes": request.notes.clone(),
            "alert": request.alert,
        });
        sqlx::query(
            r#"
            insert into outbox_message (
                id, event_type, aggregate_type, aggregate_id,
                correlation_id, actor_app_user_id, occurred_at, payload
            )
            values ($1::uuid,'apple.reminder.upsert.requested','task',$2,$3,$4,now(),$5)
            on conflict (id) do nothing
            "#,
        )
        .bind(&command_id)
        .bind(&request.wbs_id)
        .bind(correlation_id)
        .bind(actor_app_user_id)
        .bind(payload)
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.queue_apple_reminder", &error))?;

        Ok(AppleReminderCommandReceipt {
            command_id,
            state: "queued".into(),
        })
    }

    pub async fn set_status(&self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>> {
        let row = sqlx::query_as::<_, WbsRow>(
            r#"
            update wbs_item
            set status=$2, updated_at=now()
            where id=$1
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, planned_start, planned_finish, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.set_status", &error))?;
        row.map(map_row).transpose()
    }
}
