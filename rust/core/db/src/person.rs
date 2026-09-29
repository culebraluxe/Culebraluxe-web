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
    civil_status: Option<String>,
    status: String,
    archived_at: Option<DateTime<Utc>>,
    company: Option<String>,
    manual_override: bool,
    manual_override_at: Option<DateTime<Utc>>,
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
        civil_status: row.civil_status,
        status: row.status,
        archived_at: row.archived_at.map(|value| value.to_rfc3339()),
        company: row.company,
        manual_override: row.manual_override,
        manual_override_at: row.manual_override_at.map(|value| value.to_rfc3339()),
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

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn get(&self, person_id: &str) -> DbResult<Option<Person>> {
        let row = sqlx::query_as::<_, PersonRow>(
            r#"
            select id::text as id, display_name, civil_status, status, archived_at, company,
                   manual_override, manual_override_at
            from person
            where id = $1::uuid and archived_at is null
            limit 1
            "#,
        )
        .bind(person_id)
        .fetch_optional(&mut *self.db.connection().await?)
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
            select p.id::text as id, p.display_name, p.civil_status, p.status, p.archived_at, p.company,
                   p.manual_override, p.manual_override_at
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
        .fetch_all(&mut *self.db.connection().await?)
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
            returning id::text as id, display_name, civil_status, status, archived_at, company,
                      manual_override, manual_override_at
            "#,
        )
        .bind(&request.person_id)
        .bind(request.display_name.trim())
        .fetch_optional(&mut *self.db.connection().await?)
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
                civil_status = coalesce(nullif($3::text, ''), civil_status),
                status = $4,
                company = nullif($5::text, ''),
                location = case when $6::boolean then nullif(trim($7::text), '') else location end,
                -- The hand-fix hold (migration 256): what a human decided, the feed may not overwrite.
                -- `None` leaves the hold as it is; setting it stamps when, clearing it forgets it.
                manual_override = coalesce($8::boolean, manual_override),
                manual_override_at = case when $8::boolean is null then manual_override_at
                                          when $8::boolean then now() else null end,
                updated_at = now()
            where id = $1::uuid and archived_at is null
            returning id::text as id, display_name, civil_status, status, archived_at, company,
                      manual_override, manual_override_at
            "#,
        )
        .bind(&request.person_id)
        .bind(request.display_name.trim())
        .bind(request.civil_status.as_deref().map(str::trim))
        .bind(request.status.trim())
        .bind(request.company.as_deref())
        .bind(request.location.is_some())
        .bind(request.location.as_deref())
        .bind(request.manual_override)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("person.update_admin", &error))?;

        Ok(row.map(map_person))
    }

    /// A new person, as named on a contract: the seller the Forms screen was given, when no one has that name yet.
    pub async fn create_seller(&self, display_name: &str) -> DbResult<Person> {
        let row = sqlx::query_as::<_, PersonRow>(
            r#"
            insert into person (display_name, role, status)
            values ($1, 'seller', 'new')
            returning id::text as id, display_name, civil_status, status, archived_at, company,
                      manual_override, manual_override_at
            "#,
        )
        .bind(display_name.trim())
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("person.create_seller", &error))?;
        Ok(map_person(row))
    }

    /// Sets the email or phone a person's record shows, as typed: the one shown now is replaced, and so is any copy
    /// of the same address or number on this person written another way (`787-555-1234` and `(787) 555 1234` are
    /// one phone). Answers the other person's name, and changes nothing, when someone else already has it.
    pub async fn set_contact(
        &self,
        person_id: &str,
        kind: &str,
        value: &str,
    ) -> DbResult<Option<String>> {
        let value = if kind == "email" {
            value.trim().to_lowercase()
        } else {
            value.trim().to_owned()
        };
        let normalized = if kind == "phone" {
            semantic_phone(&value)
        } else {
            value.clone()
        };
        let matches = sqlx::query_as::<_, (String, String, String)>(
            r#"
            select pi.id::text, pi.person_id::text, p.display_name
              from person_identity pi
              join person p on p.id = pi.person_id
             where pi.identity_type = $1
               and (
                 ($1 = 'phone' and
                   (case
                     when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                       and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
                     then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
                     else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
                   end) = $2)
                 or ($1 = 'email' and lower(trim(pi.identity_value)) = $2)
               )
            "#,
        )
        .bind(kind)
        .bind(&normalized)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("person.set_contact.lookup", &error))?;
        if let Some((_, _, owner)) = matches
            .iter()
            .find(|(_, owner_id, _)| owner_id != person_id)
        {
            return Ok(Some(owner.clone()));
        }
        let mut replaced: Vec<String> = matches.into_iter().map(|(id, _, _)| id).collect();
        let shown = sqlx::query_scalar::<_, String>(
            r#"
            select id::text from person_identity
             where person_id = $1::uuid and identity_type = $2
             order by is_primary desc, created_at asc
             limit 1
            "#,
        )
        .bind(person_id)
        .bind(kind)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("person.set_contact.shown", &error))?;
        replaced.extend(shown);
        sqlx::query("delete from person_identity where id = any($1::uuid[])")
            .bind(&replaced)
            .execute(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("person.set_contact.replace", &error))?;
        sqlx::query("update person_identity set is_primary = false where person_id = $1::uuid and identity_type = $2")
            .bind(person_id)
            .bind(kind)
            .execute(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("person.set_contact.demote", &error))?;
        sqlx::query(
            r#"
            insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
            values ($1::uuid, $2, $3, 'portal', true)
            "#,
        )
        .bind(person_id)
        .bind(kind)
        .bind(&value)
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("person.set_contact.insert", &error))?;
        Ok(None)
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
        .fetch_all(&mut *self.db.connection().await?)
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
        .fetch_one(&mut *self.db.connection().await?)
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
        .fetch_all(&mut *self.db.connection().await?)
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
