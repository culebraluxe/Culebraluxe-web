use crate::{Database, DbFailure, DbResult};
use domain::{RelationshipDecision, RelationshipEvidenceReview, RelationshipEvidenceRow};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct EvidenceDbRow {
    id: String,
    source: String,
    source_account: String,
    source_identity_key: String,
    source_label: Option<String>,
    display_name: Option<String>,
    organization: Option<String>,
    emails: Value,
    phones: Value,
    first_observed_at: Option<String>,
    last_observed_at: Option<String>,
    last_inbound_at: Option<String>,
    last_outbound_at: Option<String>,
    inbound_count: Option<i64>,
    outbound_count: Option<i64>,
    is_two_way: Option<bool>,
    is_owner_initiated: Option<bool>,
    is_automated_or_bulk: Option<bool>,
    is_organization_or_service: Option<bool>,
    known_apple_contact: Option<bool>,
    has_email: bool,
    has_phone: bool,
    coverage_note: Option<String>,
    canonical_person_id: Option<String>,
    match_method: Option<String>,
    match_confidence: Option<String>,
    review_state: String,
    match_reason: Option<String>,
    rule_version: Option<String>,
    evidence_fingerprint: String,
    updated_at: String,
}

impl From<EvidenceDbRow> for RelationshipEvidenceRow {
    fn from(row: EvidenceDbRow) -> Self {
        Self {
            id: row.id,
            source: row.source,
            source_account: row.source_account,
            source_identity_key: row.source_identity_key,
            source_label: row.source_label,
            display_name: row.display_name,
            organization: row.organization,
            emails: row.emails,
            phones: row.phones,
            first_observed_at: row.first_observed_at,
            last_observed_at: row.last_observed_at,
            last_inbound_at: row.last_inbound_at,
            last_outbound_at: row.last_outbound_at,
            inbound_count: row.inbound_count,
            outbound_count: row.outbound_count,
            is_two_way: row.is_two_way,
            is_owner_initiated: row.is_owner_initiated,
            is_automated_or_bulk: row.is_automated_or_bulk,
            is_organization_or_service: row.is_organization_or_service,
            known_apple_contact: row.known_apple_contact,
            has_email: row.has_email,
            has_phone: row.has_phone,
            coverage_note: row.coverage_note,
            review_state: row.review_state,
            match_method: row.match_method,
            match_confidence: row.match_confidence,
            canonical_person_id: row.canonical_person_id,
            match_reason: row.match_reason,
            rule_version: row.rule_version,
            evidence_fingerprint: row.evidence_fingerprint,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Clone)]
pub struct RelationshipEvidenceDao {
    db: Database,
}

impl RelationshipEvidenceDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    async fn recent_rows(&self, source: Option<&str>) -> DbResult<Vec<RelationshipEvidenceRow>> {
        let rows = if let Some(source) = source {
            sqlx::query_as::<_, EvidenceDbRow>(
                r#"
                select
                  id::text as id, source, source_account, source_identity_key, source_label,
                  display_name, organization, emails, phones,
                  first_observed_at::text as first_observed_at,
                  last_observed_at::text as last_observed_at,
                  last_inbound_at::text as last_inbound_at,
                  last_outbound_at::text as last_outbound_at,
                  inbound_count::bigint as inbound_count,
                  outbound_count::bigint as outbound_count,
                  is_two_way, is_owner_initiated, is_automated_or_bulk,
                  is_organization_or_service, known_apple_contact,
                  has_email, has_phone, coverage_note,
                  canonical_person_id::text as canonical_person_id,
                  match_method, match_confidence, review_state,
                  match_reason, rule_version, evidence_fingerprint,
                  updated_at::text as updated_at
                from integration_relationship_evidence
                where source = $1
                order by coalesce(last_observed_at, created_at) desc nulls last
                limit 10000
                "#,
            )
            .bind(source)
            .fetch_all(self.db.pool())
            .await
        } else {
            sqlx::query_as::<_, EvidenceDbRow>(
                r#"
                select
                  id::text as id, source, source_account, source_identity_key, source_label,
                  display_name, organization, emails, phones,
                  first_observed_at::text as first_observed_at,
                  last_observed_at::text as last_observed_at,
                  last_inbound_at::text as last_inbound_at,
                  last_outbound_at::text as last_outbound_at,
                  inbound_count::bigint as inbound_count,
                  outbound_count::bigint as outbound_count,
                  is_two_way, is_owner_initiated, is_automated_or_bulk,
                  is_organization_or_service, known_apple_contact,
                  has_email, has_phone, coverage_note,
                  canonical_person_id::text as canonical_person_id,
                  match_method, match_confidence, review_state,
                  match_reason, rule_version, evidence_fingerprint,
                  updated_at::text as updated_at
                from integration_relationship_evidence
                order by coalesce(last_observed_at, created_at) desc nulls last
                limit 10000
                "#,
            )
            .fetch_all(self.db.pool())
            .await
        }
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.recent_rows", &error))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn review(
        &self,
        review_state: &str,
        search: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<RelationshipEvidenceReview> {
        let rows = self.recent_rows(None).await?;
        let needle = search.trim().to_ascii_lowercase();
        let filtered = rows
            .into_iter()
            .filter(|row| review_state == "all" || row.review_state == review_state)
            .filter(|row| {
                if needle.is_empty() {
                    return true;
                }
                [
                    row.display_name.as_deref(),
                    row.organization.as_deref(),
                    Some(row.source_identity_key.as_str()),
                ]
                .into_iter()
                .flatten()
                .any(|value| value.to_ascii_lowercase().contains(&needle))
            })
            .collect::<Vec<_>>();
        let total = filtered.len() as i64;
        let start = offset.max(0) as usize;
        let end = (start + limit.clamp(1, 100) as usize).min(filtered.len());
        let rows = if start >= filtered.len() {
            Vec::new()
        } else {
            filtered[start..end].to_vec()
        };
        Ok(RelationshipEvidenceReview { rows, total })
    }

    pub async fn by_id(&self, id: &str) -> DbResult<Option<RelationshipEvidenceRow>> {
        let row = sqlx::query_as::<_, EvidenceDbRow>(
            r#"
            select
              id::text as id, source, source_account, source_identity_key, source_label,
              display_name, organization, emails, phones,
              first_observed_at::text as first_observed_at,
              last_observed_at::text as last_observed_at,
              last_inbound_at::text as last_inbound_at,
              last_outbound_at::text as last_outbound_at,
              inbound_count::bigint as inbound_count,
              outbound_count::bigint as outbound_count,
              is_two_way, is_owner_initiated, is_automated_or_bulk,
              is_organization_or_service, known_apple_contact,
              has_email, has_phone, coverage_note,
              canonical_person_id::text as canonical_person_id,
              match_method, match_confidence, review_state,
              match_reason, rule_version, evidence_fingerprint,
              updated_at::text as updated_at
            from integration_relationship_evidence
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.by_id", &error))?;
        Ok(row.map(Into::into))
    }

    pub async fn classify(
        &self,
        id: &str,
        automated: Option<bool>,
        service: Option<bool>,
    ) -> DbResult<bool> {
        let updated = sqlx::query_scalar::<_, String>(
            r#"
            update integration_relationship_evidence
               set is_automated_or_bulk = coalesce($2, is_automated_or_bulk),
                   is_organization_or_service = coalesce($3, is_organization_or_service),
                   updated_at = now()
             where id = $1::uuid
             returning id::text
            "#,
        )
        .bind(id)
        .bind(automated)
        .bind(service)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.classify", &error))?;
        Ok(updated.is_some())
    }

    pub async fn person_exists(&self, person_id: &str) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from person where id = $1::uuid and archived_at is null)",
        )
        .bind(person_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.person_exists", &error))
    }

    pub async fn source_link(
        &self,
        source: &str,
        source_account: &str,
        source_identity_key: &str,
    ) -> DbResult<Option<String>> {
        let dedicated = sqlx::query_scalar::<_, bool>(
            "select to_regclass('public.integration_source_person_link') is not null",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.link_table", &error))?;

        let row = if dedicated {
            sqlx::query_scalar::<_, String>(
                r#"
                select canonical_person_id::text
                  from integration_source_person_link
                 where source = $1 and source_account = $2 and source_identity_key = $3
                 limit 1
                "#,
            )
            .bind(source)
            .bind(source_account)
            .bind(source_identity_key)
            .fetch_optional(self.db.pool())
            .await
        } else {
            sqlx::query_scalar::<_, String>(
                r#"
                select canonical_person_id::text
                  from integration_relationship_evidence
                 where source = $1 and source_account = $2 and source_identity_key = $3
                   and canonical_person_id is not null
                 limit 1
                "#,
            )
            .bind(source)
            .bind(source_account)
            .bind(source_identity_key)
            .fetch_optional(self.db.pool())
            .await
        }
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.source_link", &error))?;

        Ok(row)
    }

    pub async fn people_by_email(&self, email: &str) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select distinct pi.person_id::text
              from person_identity pi
              join person p on p.id = pi.person_id
             where pi.identity_type = 'email'
               and lower(btrim(pi.identity_value)) = lower(btrim($1))
               and p.archived_at is null
             order by pi.person_id::text
             limit 2
            "#,
        )
        .bind(email)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.people_by_email", &error))
    }

    pub async fn people_by_phone(&self, phone: &str) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select distinct pi.person_id::text
              from person_identity pi
              join person p on p.id = pi.person_id
             where pi.identity_type = 'phone'
               and p.archived_at is null
               and (
                 case
                   when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                    and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
                   then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
                   else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
                 end
               ) = $1
             order by pi.person_id::text
             limit 2
            "#,
        )
        .bind(phone)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.people_by_phone", &error))
    }

    pub async fn candidates(
        &self,
        source: Option<&str>,
        review_state: Option<&str>,
        ids: &[String],
        limit: i64,
    ) -> DbResult<Vec<RelationshipEvidenceRow>> {
        let id_filter = if ids.is_empty() {
            None
        } else {
            Some(
                ids.iter()
                    .cloned()
                    .collect::<std::collections::HashSet<_>>(),
            )
        };
        let rows = self.recent_rows(source).await?;
        Ok(rows
            .into_iter()
            .filter(|row| {
                review_state
                    .map(|state| row.review_state == state)
                    .unwrap_or(true)
            })
            .filter(|row| {
                id_filter
                    .as_ref()
                    .map(|set| set.contains(&row.id))
                    .unwrap_or(true)
            })
            .take(limit.clamp(1, 500) as usize)
            .collect())
    }

    pub async fn record_decision(
        &self,
        id: &str,
        decision: &RelationshipDecision,
    ) -> DbResult<bool> {
        let updated = sqlx::query_scalar::<_, String>(
            r#"
            update integration_relationship_evidence
               set canonical_person_id = case
                     when canonical_person_id is null then $2::uuid
                     else canonical_person_id
                   end,
                   review_state = case
                     when canonical_person_id is null then $3
                     else 'exact_linked'
                   end,
                   match_method = $4,
                   match_confidence = case
                     when canonical_person_id is null then $5
                     when $2::uuid is not null and $2::uuid = canonical_person_id then $5
                     else 'ambiguous'
                   end,
                   match_reason = case
                     when canonical_person_id is null then $6
                     when $2::uuid is not null and $2::uuid = canonical_person_id then $6
                     else 'established_link_preserved_automated_conflict'
                   end,
                   rule_version = $7,
                   updated_at = now()
             where id = $1::uuid
             returning id::text
            "#,
        )
        .bind(id)
        .bind(decision.canonical_person_id.as_deref())
        .bind(&decision.review_state)
        .bind(&decision.match_method)
        .bind(&decision.match_confidence)
        .bind(&decision.reason)
        .bind(&decision.rule_version)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("relationship_evidence.record_decision", &error))?;
        Ok(updated.is_some())
    }
}
