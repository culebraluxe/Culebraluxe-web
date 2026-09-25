use crate::{Database, DbFailure, DbResult};
use domain::{IssueQueueRow, IssuesPage};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct IssueRow {
    id: String,
    issue_type: String,
    severity: String,
    state: String,
    title: String,
    detail: Option<String>,
    domain_type: String,
    domain_id: String,
    detected_at: String,
    resolved_at: Option<String>,
    related_deal_id: Option<String>,
    property_name: Option<String>,
    client_name: Option<String>,
    closing_date: Option<String>,
    deal_stage: Option<String>,
    task_title: Option<String>,
    task_due_at: Option<String>,
    total: i64,
}

#[derive(Clone)]
pub struct IssueDao {
    db: Database,
}

impl IssueDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn page(
        &self,
        scope: &str,
        state: &str,
        page: i64,
        page_size: i64,
    ) -> DbResult<IssuesPage> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 50);
        let state = if state == "RESOLVED" { "RESOLVED" } else { "OPEN" };
        let scope = if scope == "SUPPORT_EXCEPTION" {
            "SUPPORT_EXCEPTION"
        } else {
            "OPERATIONS_EXCEPTION"
        };

        if scope == "SUPPORT_EXCEPTION" {
            return Ok(IssuesPage {
                rows: Vec::new(),
                total: 0,
                page,
                page_size,
                scope: scope.into(),
                state: state.into(),
            });
        }

        let types = vec![
            "MISSING_EXECUTED_PS",
            "APPRAISAL_OVERDUE",
            "CLOSING_DATE_AT_RISK",
            "OVERDUE_DEAL_TASK",
        ];
        let offset = (page - 1) * page_size;

        let rows = sqlx::query_as::<_, IssueRow>(
            r#"
            with base as (
              select
                i.id::text as id,
                i.type as issue_type,
                i.severity,
                i.state,
                i.title,
                i.detail,
                i.domain_type,
                i.domain_id::text as domain_id,
                i.detected_at::text as detected_at,
                i.resolved_at::text as resolved_at,
                case when i.domain_type = 'deal'
                  then i.domain_id::text
                  else t.deal_id::text
                end as related_deal_id,
                case when i.domain_type = 'task' then t.title else null end as task_title,
                case when i.domain_type = 'task' then t.due_at::text else null end as task_due_at
              from issue i
              left join task t
                on i.domain_type = 'task'
               and t.id = i.domain_id
              where i.state = $1
                and i.type = any($2::text[])
            ),
            joined as (
              select
                b.*,
                d.closing_date::text as closing_date,
                d.stage as deal_stage,
                p.name as property_name,
                pe.display_name as client_name
              from base b
              left join deal d on d.id::text = b.related_deal_id
              left join property p on p.id = d.property_id
              left join person pe on pe.id = d.client_person_id
            )
            select j.*, count(*) over()::bigint as total
            from joined j
            order by
              case j.severity when 'RED' then 0 when 'YELLOW' then 1 else 2 end,
              j.detected_at asc,
              j.id asc
            limit $3 offset $4
            "#,
        )
        .bind(state)
        .bind(&types)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("issue.page", &error))?;

        let total = rows.first().map(|row| row.total).unwrap_or(0);
        let rows = rows
            .into_iter()
            .map(|row| IssueQueueRow {
                id: row.id,
                issue_type: row.issue_type,
                severity: row.severity,
                state: row.state,
                title: row.title,
                detail: row.detail,
                domain_type: row.domain_type,
                domain_id: row.domain_id,
                detected_at: row.detected_at,
                resolved_at: row.resolved_at,
                related_deal_id: row.related_deal_id,
                property_name: row.property_name,
                client_name: row.client_name,
                closing_date: row.closing_date,
                deal_stage: row.deal_stage,
                task_title: row.task_title,
                task_due_at: row.task_due_at,
            })
            .collect();

        Ok(IssuesPage {
            rows,
            total,
            page,
            page_size,
            scope: scope.into(),
            state: state.into(),
        })
    }
}
