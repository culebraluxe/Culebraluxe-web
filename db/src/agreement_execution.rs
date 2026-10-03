use crate::{Database, DbFailure, DbResult};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct IssuedAgreementDocumentRow {
    pub id: String,
    pub contract_id: Option<String>,
    pub template_id: Option<String>,
    pub issued_version: Option<i32>,
}

#[derive(Clone)]
pub struct AgreementExecutionDao {
    db: Database,
}

impl AgreementExecutionDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn load_issued_document(
        &self,
        document_id: &str,
    ) -> DbResult<Option<IssuedAgreementDocumentRow>> {
        sqlx::query_as::<_, IssuedAgreementDocumentRow>(
            r#"
            select id::text as id,
                   contract_id::text as contract_id,
                   template_id,
                   issued_version
            from transaction_document
            where id=$1::uuid
            limit 1
            "#,
        )
        .bind(document_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("agreement_execution.load_document", &error))
    }

    pub async fn marker_matches(
        &self,
        document_id: &str,
        issued_version: i32,
        event_id: &str,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                from agreement_execution
                where document_id=$1::uuid
                  and issued_version=$2
                  and event_id=$3
            )
            "#,
        )
        .bind(document_id)
        .bind(issued_version)
        .bind(event_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("agreement_execution.marker_matches", &error))
    }
}
