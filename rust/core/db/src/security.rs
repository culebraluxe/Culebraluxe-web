use crate::{Database, DbFailure, DbResult};
use domain::ActingUser;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct PrincipalRow {
    app_user_id: String,
    display_name: String,
    email: Option<String>,
    account_type: String,
    person_id: Option<String>,
    role_codes: Vec<String>,
    authority_codes: Vec<String>,
}

#[derive(Clone)]
pub struct SecurityDao {
    db: Database,
}

impl SecurityDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn resolve_provider_subject(&self, provider: &str, subject: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select app_user_id::text
            from auth_identity
            where provider = $1 and provider_subject = $2
            limit 1
            "#,
        )
        .bind(provider)
        .bind(subject)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("security.resolve_provider_subject", &error))
    }

    pub async fn get_principal(&self, app_user_id: &str) -> DbResult<Option<ActingUser>> {
        let row = sqlx::query_as::<_, PrincipalRow>(
            r#"
            select
              u.id::text as app_user_id,
              u.display_name,
              u.email,
              u.account_type,
              u.person_id::text as person_id,
              array(
                select distinct r.code
                from app_user_role aur
                join security_role r on r.id = aur.role_id and r.active = true
                where aur.app_user_id = u.id
                order by r.code
              ) as role_codes,
              array(
                select distinct a.code
                from app_user_role aur
                join security_role r on r.id = aur.role_id and r.active = true
                join role_authority ra on ra.role_id = r.id
                join authority a on a.id = ra.authority_id
                where aur.app_user_id = u.id
                order by a.code
              ) as authority_codes
            from app_user u
            where u.id = $1::uuid and u.active = true
            limit 1
            "#,
        )
        .bind(app_user_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("security.get_principal", &error))?;

        Ok(row.map(|row| ActingUser {
            app_user_id: row.app_user_id,
            display_name: row.display_name,
            email: row.email,
            account_type: row.account_type,
            role_codes: row.role_codes,
            authority_codes: row.authority_codes,
            person_id: row.person_id,
        }))
    }
}
