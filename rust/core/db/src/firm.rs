use crate::{Database, DbFailure, DbResult};
use domain::{Firm, UpsertFirmRequest};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct FirmRow {
    id: String,
    name: String,
    legal_name: Option<String>,
    kind: Option<String>,
    status: String,
}

fn compact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn map_firm(row: FirmRow) -> Firm {
    Firm {
        id: row.id,
        name: row.name,
        legal_name: compact(row.legal_name.as_deref()),
        kind: compact(row.kind.as_deref()),
        status: row.status,
    }
}

#[derive(Clone)]
pub struct FirmDao {
    db: Database,
}

impl FirmDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, firm_id: &str) -> DbResult<Option<Firm>> {
        let row = sqlx::query_as::<_, FirmRow>(
            "select id::text as id, name, legal_name, kind, status from firm where id = $1::uuid limit 1",
        )
        .bind(firm_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("firm.get", &error))?;
        Ok(row.map(map_firm))
    }

    pub async fn find_by_name(&self, name: &str) -> DbResult<Option<Firm>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, FirmRow>(
            r#"
            select id::text as id, name, legal_name, kind, status
            from firm
            where lower(trim(name)) = lower(trim($1))
               or lower(trim(coalesce(legal_name, ''))) = lower(trim($1))
            order by case when lower(trim(coalesce(legal_name, ''))) = lower(trim($1)) then 0 else 1 end,
                     id
            limit 2
            "#,
        )
        .bind(name)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("firm.find_by_name", &error))?;

        if rows.len() > 1 && rows[0].id != rows[1].id {
            return Err(DbFailure::schema_mismatch(
                "firm.find_by_name",
                format!("ambiguous Firm name: {name}"),
            ));
        }
        Ok(rows.into_iter().next().map(map_firm))
    }

    pub async fn upsert(&self, request: &UpsertFirmRequest) -> DbResult<Firm> {
        let name = request.name.trim();
        let existing = if let Some(id) = request.firm_id.as_deref() {
            self.get(id).await?
        } else {
            self.find_by_name(name).await?
        };

        let legal_name = request
            .legal_name
            .as_deref()
            .and_then(|value| compact(Some(value)))
            .or_else(|| existing.as_ref().and_then(|firm| firm.legal_name.clone()));
        let kind = request
            .kind
            .as_deref()
            .and_then(|value| compact(Some(value)))
            .or_else(|| existing.as_ref().and_then(|firm| firm.kind.clone()));
        let status = request
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| existing.as_ref().map(|firm| firm.status.clone()))
            .unwrap_or_else(|| "active".into());

        let row = if let Some(existing) = existing {
            sqlx::query_as::<_, FirmRow>(
                r#"
                update firm
                set name = $2, legal_name = $3, kind = $4, status = $5, updated_at = now()
                where id = $1::uuid
                returning id::text as id, name, legal_name, kind, status
                "#,
            )
            .bind(&existing.id)
            .bind(name)
            .bind(legal_name)
            .bind(kind)
            .bind(status)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("firm.upsert.update", &error))?
        } else {
            sqlx::query_as::<_, FirmRow>(
                r#"
                insert into firm (name, legal_name, kind, status)
                values ($1, $2, $3, $4)
                returning id::text as id, name, legal_name, kind, status
                "#,
            )
            .bind(name)
            .bind(legal_name)
            .bind(kind)
            .bind(status)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("firm.upsert.insert", &error))?
        };

        Ok(map_firm(row))
    }
}
