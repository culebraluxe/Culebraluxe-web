use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    AttachPersonIdentityRequest, Person, PersonIdentity, PersonIdentityKind, PersonSearchResult,
    SearchPeopleRequest, SetPersonDisplayNameRequest, UpdatePersonAdminRequest,
};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct PersonRow {
    id: String,
    display_name: String,
    status: String,
    archived_at: Option<DateTime<Utc>>,
    company: Option<String>,
}

#[derive(Debug, FromRow)]
struct IdentityRow {
    person_id: String,
    identity_value: String,
    source_system: Option<String>,
    is_primary: bool,
}

#[derive(Debug, FromRow)]
struct SearchRow {
    id: String,
    display_name: String,
    role: String,
    status: String,
    location: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

fn map_person(row: PersonRow) -> Person {
    Person {
        id: row.id,
        display_name: row.display_name,
        status: row.status,
        archived_at: row.archived_at.map(|value| value.to_rfc3339()),
        company: row.company,
    }
}

fn semantic_phone(value: &str) -> String {
    let digits: String = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    if digits.len() == 11 && digits.starts_with('1') {
        digits[1..].to_owned()
    } else {
        digits
    }
}

fn normalized_identity(identity: &PersonIdentity) -> String {
    match identity.kind {
        PersonIdentityKind::Phone => semantic_phone(&identity.value),
        PersonIdentityKind::Email => identity.value.trim().to_lowercase(),
        PersonIdentityKind::External => identity.value.trim().to_owned(),
    }
}

#[derive(Clone)]
pub struct PersonDao {
    db: Database,
}

impl PersonDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, person_id: &str) -> DbResult<Option<Person>> {
        let row = sqlx::query_as::<_, PersonRow>(
            r#"
            select id::text as id, display_name, status, archived_at, company
            from person
            where id = $1::uuid and archived_at is null
            limit 1
            "#,
        )
        .bind(person_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.get", &error))?;
        Ok(row.map(map_person))
    }

    pub async fn find_by_identity(&self, identity: &PersonIdentity) -> DbResult<Option<Person>> {
        let kind = identity.kind.as_str();
        let value = normalized_identity(identity);
        let source_system = identity
            .source_system
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let rows = sqlx::query_as::<_, PersonRow>(
            r#"
            select p.id::text as id, p.display_name, p.status, p.archived_at, p.company
            from person_identity pi
            join person p on p.id = pi.person_id
            where p.archived_at is null
              and pi.identity_type = $1
              and (
                ($1 = 'phone' and
                  (case
                    when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                      and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
                    then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
                    else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
                  end) = $2)
                or ($1 = 'email' and lower(trim(pi.identity_value)) = $2)
                or ($1 = 'external' and pi.identity_value = $2)
              )
              and ($3::text is null or pi.source_system = $3)
            order by p.id
            limit 2
            "#,
        )
        .bind(kind)
        .bind(&value)
        .bind(source_system)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.find_by_identity", &error))?;

        if rows.len() > 1 {
            return Err(DbFailure::schema_mismatch(
                "person.find_by_identity",
                format!("ambiguous Person identity {kind}:{value}"),
            ));
        }

        Ok(rows.into_iter().next().map(map_person))
    }

    pub async fn set_display_name(
        &self,
        request: &SetPersonDisplayNameRequest,
    ) -> DbResult<Option<Person>> {
        let row = sqlx::query_as::<_, PersonRow>(
            r#"
            update person
            set display_name = $2, updated_at = now()
            where id = $1::uuid and archived_at is null
            returning id::text as id, display_name, status, archived_at, company
            "#,
        )
        .bind(&request.person_id)
        .bind(request.display_name.trim())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.set_display_name", &error))?;

        Ok(row.map(map_person))
    }

    pub async fn update_admin(
        &self,
        request: &UpdatePersonAdminRequest,
    ) -> DbResult<Option<Person>> {
        let row = sqlx::query_as::<_, PersonRow>(
            r#"
            update person
            set
                display_name = $2,
                status = $3,
                company = nullif($4::text, ''),
                updated_at = now()
            where id = $1::uuid and archived_at is null
            returning id::text as id, display_name, status, archived_at, company
            "#,
        )
        .bind(&request.person_id)
        .bind(request.display_name.trim())
        .bind(request.status.trim())
        .bind(request.company.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.update_admin", &error))?;

        Ok(row.map(map_person))
    }

    pub async fn attach_identity(
        &self,
        request: &AttachPersonIdentityRequest,
    ) -> DbResult<PersonIdentity> {
        let identity = &request.identity;
        let kind = identity.kind.as_str();
        let normalized = normalized_identity(identity);
        let source_system = identity
            .source_system
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let existing = sqlx::query_as::<_, IdentityRow>(
            r#"
            select person_id::text as person_id, identity_value, source_system, is_primary
            from person_identity
            where identity_type = $1
              and (
                ($1 = 'phone' and
                  (case
                    when length(regexp_replace(identity_value, '[^0-9]', '', 'g')) = 11
                      and left(regexp_replace(identity_value, '[^0-9]', '', 'g'), 1) = '1'
                    then substring(regexp_replace(identity_value, '[^0-9]', '', 'g') from 2)
                    else regexp_replace(identity_value, '[^0-9]', '', 'g')
                  end) = $2)
                or ($1 = 'email' and lower(trim(identity_value)) = $2)
                or ($1 = 'external' and identity_value = $2)
              )
              and ($3::text is null or source_system = $3)
            limit 2
            "#,
        )
        .bind(kind)
        .bind(&normalized)
        .bind(source_system)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.attach_identity.lookup", &error))?;

        if existing
            .iter()
            .any(|row| row.person_id != request.person_id)
        {
            return Err(DbFailure::schema_mismatch(
                "person.attach_identity",
                format!("identity already belongs to another Person: {kind}:{normalized}"),
            ));
        }

        if let Some(row) = existing.into_iter().next() {
            return Ok(PersonIdentity {
                kind: identity.kind.clone(),
                value: row.identity_value,
                source_system: row.source_system,
                is_primary: row.is_primary,
            });
        }

        let row = sqlx::query_as::<_, IdentityRow>(
            r#"
            insert into person_identity (
                person_id, identity_type, identity_value, source_system, is_primary
            )
            values ($1::uuid, $2, $3, $4, $5)
            returning person_id::text as person_id, identity_value, source_system, is_primary
            "#,
        )
        .bind(&request.person_id)
        .bind(kind)
        .bind(&normalized)
        .bind(source_system)
        .bind(identity.is_primary)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.attach_identity", &error))?;

        Ok(PersonIdentity {
            kind: identity.kind.clone(),
            value: row.identity_value,
            source_system: row.source_system,
            is_primary: row.is_primary,
        })
    }

    pub async fn search(&self, request: &SearchPeopleRequest) -> DbResult<Vec<PersonSearchResult>> {
        let query = request.query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }
        let limit = request.limit.unwrap_or(8).clamp(1, 100);
        let pattern = format!("%{query}%");

        let rows = sqlx::query_as::<_, SearchRow>(
            r#"
            select
              p.id::text as id,
              p.display_name,
              p.role,
              p.status,
              p.location,
              (
                select i.identity_value
                from person_identity i
                where i.person_id = p.id and i.identity_type = 'email'
                order by i.is_primary desc, i.created_at desc
                limit 1
              ) as email,
              (
                select i.identity_value
                from person_identity i
                where i.person_id = p.id and i.identity_type = 'phone'
                order by i.is_primary desc, i.created_at desc
                limit 1
              ) as phone
            from person p
            where p.archived_at is null
              and (
                p.display_name ilike $1
                or exists (
                  select 1 from person_identity i
                  where i.person_id = p.id and i.identity_value ilike $1
                )
              )
            order by p.display_name asc
            limit $2
            "#,
        )
        .bind(pattern)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("person.search", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| PersonSearchResult {
                id: row.id,
                display_name: row.display_name,
                role: row.role,
                status: row.status,
                location: row.location,
                email: row.email,
                phone: row.phone,
            })
            .collect())
    }
}
