use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    CreateWbsItemRequest, SaveWbsItemRequest, WbsCategory, WbsEntityLink, WbsEntityType, WbsItem,
    WbsStatus,
};
use sqlx::FromRow;

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
    owner: Option<String>,
    sort_order: Option<i32>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
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
            "due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at ",
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

    pub async fn get(&self, id: &str) -> DbResult<Option<WbsItem>> {
        let row = sqlx::query_as::<_, WbsRow>(wbs_sql!("where id = $1 limit 1"))
            .bind(id)
            .fetch_optional(self.db.pool())
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
        .fetch_all(self.db.pool())
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
        .fetch_all(self.db.pool())
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
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.list_for_entity", &error))?;
        rows.into_iter().map(map_row).collect()
    }

    pub async fn create(&self, request: &CreateWbsItemRequest) -> DbResult<WbsItem> {
        let row = sqlx::query_as::<_, WbsRow>(
            r#"
            insert into wbs_item (
                id, project_id, parent_id, title, notes, category, status,
                due_at, owner, sort_order, entity_type, entity_id
            )
            values ($1,$2,$3,$4,$5,$6,'open',$7::timestamptz,$8,$9,$10,$11)
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(&request.id)
        .bind(request.project_id.as_deref())
        .bind(request.parent_id.as_deref())
        .bind(request.title.trim())
        .bind(request.notes.as_deref().unwrap_or(""))
        .bind(request.category.as_str())
        .bind(request.due_at.as_deref())
        .bind(request.owner.as_deref())
        .bind(request.order)
        .bind(
            request
                .entity
                .as_ref()
                .map(|entity| entity.entity_type.as_str()),
        )
        .bind(request.entity.as_ref().map(|entity| entity.id.as_str()))
        .fetch_one(self.db.pool())
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
                due_at=$6::timestamptz, owner=$7, sort_order=$8,
                entity_type=$9, entity_id=$10, project_id=$11, parent_id=$12,
                updated_at=now()
            where id=$1
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(&base.id)
        .bind(base.title.trim())
        .bind(base.notes.as_deref().unwrap_or(""))
        .bind(base.category.as_str())
        .bind(request.status.as_ref().map(WbsStatus::as_str))
        .bind(base.due_at.as_deref())
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
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.save", &error))?;
        row.map(map_row).transpose()
    }

    pub async fn set_status(&self, id: &str, status: WbsStatus) -> DbResult<Option<WbsItem>> {
        let row = sqlx::query_as::<_, WbsRow>(
            r#"
            update wbs_item
            set status=$2, updated_at=now()
            where id=$1
            returning id, project_id, parent_id, title, notes, category, status,
                      due_at, owner, sort_order, entity_type, entity_id, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("wbs.set_status", &error))?;
        row.map(map_row).transpose()
    }
}
