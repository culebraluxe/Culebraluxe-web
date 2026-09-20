use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    BindFormInstanceToDirectContextRequest, BindFormInstanceToShowingRequest,
    BindListingFormContextRequest, CreateFormInstanceRequest, DealFormFacts, DirectFormContext,
    FormInstance, FormInstanceEvidence, FormInstanceListItem, FormInstanceStatus, FormSignerPerson,
    LatestFormEvidenceRequest, UpdateFormInstanceRequest,
};
use serde_json::Value;
use sqlx::FromRow;
use std::collections::BTreeMap;

const LISTING_TEMPLATE_ID: &str = "LISTING-01";
const SELLER_BROKER_NAME: &str = "Lisa Penfield";

#[derive(Debug, FromRow)]
struct FormRow {
    id: String,
    template_id: String,
    template_version: i32,
    deal_id: Option<String>,
    person_id: Option<String>,
    property_id: Option<String>,
    contract_id: Option<String>,
    status: String,
    field_values: Value,
    sections: Value,
    created_by_user_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct FormListRow {
    id: String,
    template_id: String,
    template_version: i32,
    deal_id: Option<String>,
    person_id: Option<String>,
    property_id: Option<String>,
    contract_id: Option<String>,
    status: String,
    field_values: Value,
    sections: Value,
    created_by_user_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deal_label: Option<String>,
    property_label: Option<String>,
    client_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct DealFactsRow {
    offer_price: Option<String>,
    closing_date: Option<NaiveDate>,
    financing_type: Option<String>,
    property_name: Option<String>,
    property_location: Option<String>,
    client_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct ParticipantSeedRow {
    role: String,
    person_id: Option<String>,
    display_name: String,
}

#[derive(Debug, FromRow)]
struct EvidenceRow {
    id: String,
    property_id: Option<String>,
    field_values: Value,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DirectContextRow {
    person_id: Option<String>,
    property_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct SignerFormRow {
    deal_id: Option<String>,
    template_id: String,
    status: String,
    person_name: Option<String>,
    resolved_person_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct SignerRow {
    person_id: Option<String>,
    display_name: String,
    email: Option<String>,
    role: String,
}

#[derive(Debug, FromRow)]
struct BrokerRow {
    person_id: Option<String>,
    email: Option<String>,
}

fn compact(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

fn string_map(value: Value) -> BTreeMap<String, String> {
    serde_json::from_value(value).unwrap_or_default()
}

fn map_form(row: FormRow) -> DbResult<FormInstance> {
    let status = FormInstanceStatus::try_from(row.status.as_str())
        .map_err(|error| DbFailure::schema_mismatch("form.map", error))?;

    Ok(FormInstance {
        id: row.id,
        template_id: row.template_id,
        template_version: row.template_version,
        deal_id: row.deal_id,
        person_id: row.person_id,
        property_id: row.property_id,
        contract_id: row.contract_id,
        status,
        field_values: string_map(row.field_values),
        sections: string_map(row.sections),
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

fn map_list(row: FormListRow) -> DbResult<FormInstanceListItem> {
    let instance = map_form(FormRow {
        id: row.id,
        template_id: row.template_id,
        template_version: row.template_version,
        deal_id: row.deal_id,
        person_id: row.person_id,
        property_id: row.property_id,
        contract_id: row.contract_id,
        status: row.status,
        field_values: row.field_values,
        sections: row.sections,
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })?;

    Ok(FormInstanceListItem {
        instance,
        deal_label: compact(row.deal_label),
        property_label: compact(row.property_label),
        client_name: compact(row.client_name),
    })
}

fn role_for_form(template_id: &str, role: &str) -> String {
    if template_id == LISTING_TEMPLATE_ID {
        if matches!(role, "owner" | "seller" | "SELLER_BROKER") {
            return "SELLER".into();
        }
    }

    match role {
        "client" => "BUYER".into(),
        "seller" | "owner" => "SELLER".into(),
        "" => "OTHER".into(),
        other => other.to_owned(),
    }
}

#[derive(Clone)]
pub struct FormDao {
    db: Database,
}

impl FormDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create_instance(
        &self,
        request: &CreateFormInstanceRequest,
    ) -> DbResult<FormInstance> {
        let field_values = serde_json::to_value(&request.field_values)
            .map_err(|error| DbFailure::schema_mismatch("form.create.serialize_fields", error.to_string()))?;
        let sections = serde_json::to_value(&request.sections)
            .map_err(|error| DbFailure::schema_mismatch("form.create.serialize_sections", error.to_string()))?;

        let row = sqlx::query_as::<_, FormRow>(
            r#"
            insert into document_form_instance (
                template_id, template_version, deal_id, person_id, property_id,
                field_values, sections, created_by_user_id
            )
            values ($1,$2,$3::uuid,$4::uuid,$5::uuid,$6,$7,$8::uuid)
            returning id::text as id, template_id, template_version,
                      deal_id::text as deal_id, person_id::text as person_id,
                      property_id::text as property_id, contract_id::text as contract_id,
                      status, field_values, sections,
                      created_by_user_id::text as created_by_user_id,
                      created_at, updated_at
            "#,
        )
        .bind(request.template_id.trim())
        .bind(request.template_version)
        .bind(request.deal_id.as_deref())
        .bind(request.person_id.as_deref())
        .bind(request.property_id.as_deref())
        .bind(field_values)
        .bind(sections)
        .bind(request.created_by_user_id.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.create", &error))?;

        map_form(row)
    }

    pub async fn get_instance(&self, form_instance_id: &str) -> DbResult<Option<FormInstance>> {
        let row = sqlx::query_as::<_, FormRow>(
            r#"
            select id::text as id, template_id, template_version,
                   deal_id::text as deal_id, person_id::text as person_id,
                   property_id::text as property_id, contract_id::text as contract_id,
                   status, field_values, sections,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from document_form_instance
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.get", &error))?;

        row.map(map_form).transpose()
    }

    pub async fn update_instance(
        &self,
        request: &UpdateFormInstanceRequest,
    ) -> DbResult<Option<FormInstance>> {
        let field_values = request
            .input
            .field_values
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| DbFailure::schema_mismatch("form.update.serialize_fields", error.to_string()))?;
        let sections = request
            .input
            .sections
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| DbFailure::schema_mismatch("form.update.serialize_sections", error.to_string()))?;

        let row = sqlx::query_as::<_, FormRow>(
            r#"
            update document_form_instance
            set field_values = coalesce($2::jsonb, field_values),
                sections = coalesce($3::jsonb, sections),
                status = coalesce($4::text, status),
                contract_id = coalesce($5::uuid, contract_id),
                updated_at = now()
            where id = $1::uuid
            returning id::text as id, template_id, template_version,
                      deal_id::text as deal_id, person_id::text as person_id,
                      property_id::text as property_id, contract_id::text as contract_id,
                      status, field_values, sections,
                      created_by_user_id::text as created_by_user_id,
                      created_at, updated_at
            "#,
        )
        .bind(&request.form_instance_id)
        .bind(field_values)
        .bind(sections)
        .bind(request.input.status.map(FormInstanceStatus::as_str))
        .bind(request.input.contract_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.update", &error))?;

        row.map(map_form).transpose()
    }

    pub async fn list_instances(&self) -> DbResult<Vec<FormInstanceListItem>> {
        let rows = sqlx::query_as::<_, FormListRow>(
            r#"
            select f.id::text as id, f.template_id, f.template_version,
                   f.deal_id::text as deal_id, f.person_id::text as person_id,
                   f.property_id::text as property_id, f.contract_id::text as contract_id,
                   f.status, f.field_values, f.sections,
                   f.created_by_user_id::text as created_by_user_id,
                   f.created_at, f.updated_at,
                   null::text as deal_label,
                   coalesce(p.name, fp.name) as property_label,
                   coalesce(c.display_name, person.display_name) as client_name
            from document_form_instance f
            left join deal d on d.id = f.deal_id
            left join property p on p.id = d.property_id
            left join person c on c.id = d.client_person_id
            left join person on person.id = f.person_id
            left join property fp on fp.id = f.property_id
            order by f.updated_at desc, f.id
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.list", &error))?;

        rows.into_iter().map(map_list).collect()
    }

    pub async fn deal_facts(&self, deal_id: &str) -> DbResult<Option<DealFormFacts>> {
        let row = sqlx::query_as::<_, DealFactsRow>(
            r#"
            select d.offer_price::text as offer_price,
                   d.closing_date,
                   d.financing_type,
                   p.name as property_name,
                   p.location as property_location,
                   client.display_name as client_name
            from deal d
            left join property p on p.id = d.property_id
            join lateral (
                select person.id, person.display_name
                from deal_participant dp
                join person on person.id = dp.person_id
                where dp.deal_id = d.id
                  and dp.role = 'client'
                  and dp.active
                order by dp.created_at asc
                limit 1
            ) client on true
            where d.id = $1::uuid
            limit 1
            "#,
        )
        .bind(deal_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.deal_facts", &error))?;

        Ok(row.map(|row| {
            let client_name = compact(row.client_name);
            let financing_type = row.financing_type.map(|value| {
                if value == "cash" {
                    "Cash".into()
                } else {
                    "Financed".into()
                }
            });
            DealFormFacts {
                client_name: client_name.clone(),
                property_label: compact(row.property_name.clone()),
                offer_amount: compact(row.offer_price),
                financing_type,
                closing_date: row.closing_date.map(|value| value.to_string()),
                person_display_name: client_name,
                property_name: compact(row.property_name),
                property_location: compact(row.property_location),
            }
        }))
    }

    pub async fn seed_participants_from_deal(
        &self,
        form_instance_id: &str,
        deal_id: &str,
    ) -> DbResult<()> {
        let mut tx = self.db.begin("form.seed_participants").await?;
        let result = async {
            let rows = sqlx::query_as::<_, ParticipantSeedRow>(
                r#"
                select dp.role,
                       dp.person_id::text as person_id,
                       coalesce(person.display_name, dp.role) as display_name
                from deal_participant dp
                left join person on person.id = dp.person_id
                where dp.deal_id = $1::uuid and dp.active = true
                order by dp.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.seed_participants.read", &error))?;

            for (order, row) in rows.into_iter().enumerate() {
                let role = match row.role.as_str() {
                    "client" => "BUYER",
                    "seller" | "owner" => "SELLER",
                    _ => "OTHER",
                };
                sqlx::query(
                    r#"
                    insert into document_form_participant (
                        form_instance_id, role, person_id, display_name, sort_order
                    )
                    values ($1::uuid,$2,$3::uuid,$4,$5)
                    "#,
                )
                .bind(form_instance_id)
                .bind(role)
                .bind(row.person_id.as_deref())
                .bind(row.display_name)
                .bind(order as i32)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("form.seed_participants.insert", &error))?;
            }
            Ok(())
        }
        .await;

        match result {
            Ok(()) => tx.commit().await,
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn latest_evidence(
        &self,
        request: &LatestFormEvidenceRequest,
    ) -> DbResult<Option<FormInstanceEvidence>> {
        let row = sqlx::query_as::<_, EvidenceRow>(
            r#"
            select f.id::text as id,
                   f.property_id::text as property_id,
                   f.field_values,
                   f.updated_at
            from document_form_instance f
            left join deal d on d.id = f.deal_id
            where f.template_id = $1
              and (
                f.person_id = $2::uuid
                or d.client_person_id = $2::uuid
                or exists (
                    select 1
                    from deal_participant dp
                    where dp.deal_id = f.deal_id
                      and dp.person_id = $2::uuid
                      and dp.active = true
                      and ($3::text[] is null or dp.role = any($3::text[]))
                )
              )
            order by f.updated_at desc, f.id desc
            limit 1
            "#,
        )
        .bind(request.template_id.trim())
        .bind(&request.person_id)
        .bind(request.roles.clone())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.latest_evidence", &error))?;

        Ok(row.map(|row| FormInstanceEvidence {
            form_instance_id: row.id,
            property_id: row.property_id,
            field_values: string_map(row.field_values),
            updated_at: Some(row.updated_at.to_rfc3339()),
        }))
    }

    pub async fn resolve_deal_launch_context(
        &self,
        deal_id: &str,
    ) -> DbResult<Option<DirectFormContext>> {
        let row = sqlx::query_as::<_, DirectContextRow>(
            r#"
            select coalesce(d.client_person_id, client.person_id)::text as person_id,
                   d.property_id::text as property_id
            from deal d
            left join lateral (
                select dp.person_id
                from deal_participant dp
                where dp.deal_id = d.id
                  and dp.role = 'client'
                  and dp.active = true
                order by dp.created_at asc
                limit 1
            ) client on true
            where d.id = $1::uuid
            limit 1
            "#,
        )
        .bind(deal_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.resolve_deal_launch_context", &error))?;

        Ok(row.and_then(|row| match (row.person_id, row.property_id) {
            (Some(person_id), Some(property_id)) => Some(DirectFormContext {
                person_id,
                property_id,
            }),
            _ => None,
        }))
    }

    pub async fn bind_direct_context(
        &self,
        request: &BindFormInstanceToDirectContextRequest,
    ) -> DbResult<bool> {
        let id = sqlx::query_scalar::<_, String>(
            r#"
            update document_form_instance
            set person_id = $2::uuid,
                property_id = $3::uuid,
                deal_id = null,
                updated_at = now()
            where id = $1::uuid
              and (person_id is null or person_id = $2::uuid)
              and (property_id is null or property_id = $3::uuid)
            returning id::text
            "#,
        )
        .bind(&request.form_instance_id)
        .bind(&request.person_id)
        .bind(&request.property_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.bind_direct_context", &error))?;

        Ok(id.is_some())
    }

    pub async fn bind_listing_context(
        &self,
        request: &BindListingFormContextRequest,
    ) -> DbResult<bool> {
        let id = sqlx::query_scalar::<_, String>(
            r#"
            update document_form_instance f
            set person_id = $2::uuid,
                property_id = $3::uuid,
                updated_at = now()
            where f.id = $1::uuid
              and f.template_id = 'LISTING-01'
              and f.status <> 'issued'
              and not exists (
                  select 1
                  from transaction_document td
                  where td.form_instance_id = f.id
                    and td.source = 'generated'
              )
            returning f.id::text
            "#,
        )
        .bind(&request.form_instance_id)
        .bind(&request.person_id)
        .bind(request.property_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.bind_listing_context", &error))?;

        Ok(id.is_some())
    }

    pub async fn get_showing_id(&self, form_instance_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            select showing_id::text
            from document_form_instance
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|value| value.flatten())
        .map_err(|error| DbFailure::from_sqlx("form.get_showing_id", &error))
    }

    pub async fn bind_showing(
        &self,
        request: &BindFormInstanceToShowingRequest,
    ) -> DbResult<bool> {
        let id = sqlx::query_scalar::<_, String>(
            r#"
            update document_form_instance
            set showing_id = $2::uuid, updated_at = now()
            where id = $1::uuid
              and (showing_id is null or showing_id = $2::uuid)
            returning id::text
            "#,
        )
        .bind(&request.form_instance_id)
        .bind(&request.showing_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.bind_showing", &error))?;

        Ok(id.is_some())
    }

    pub async fn list_signer_people(&self, form_instance_id: &str) -> DbResult<Vec<FormSignerPerson>> {
        let form = sqlx::query_as::<_, SignerFormRow>(
            r#"
            select f.deal_id::text as deal_id,
                   f.template_id,
                   f.status,
                   person.display_name as person_name,
                   person.id::text as resolved_person_id
            from document_form_instance f
            left join person on person.id = f.person_id
            where f.id = $1::uuid
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("form.signers.form", &error))?;

        let Some(form) = form else {
            return Ok(vec![]);
        };

        let listing_direct_draft = form.template_id == LISTING_TEMPLATE_ID
            && form.status != "issued"
            && form.resolved_person_id.is_some();
        let mut people = Vec::new();

        if let Some(person_id) = form.resolved_person_id.clone() {
            let email = sqlx::query_scalar::<_, String>(
                r#"
                select identity_value
                from person_identity
                where person_id = $1::uuid and identity_type = 'email'
                order by is_primary desc, created_at asc
                limit 1
                "#,
            )
            .bind(&person_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.direct_email", &error))?;

            people.push(FormSignerPerson {
                person_id: Some(person_id),
                name: form.person_name.unwrap_or_default(),
                email,
                role: if listing_direct_draft {
                    "SELLER".into()
                } else {
                    "CLIENT".into()
                },
            });
        }

        if let Some(deal_id) = form.deal_id.as_deref().filter(|_| !listing_direct_draft) {
            if let Some(client) = sqlx::query_as::<_, SignerRow>(
                r#"
                select p.id::text as person_id,
                       p.display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = p.id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       'BUYER'::text as role
                from deal d
                join person p on p.id = d.client_person_id
                where d.id = $1::uuid
                limit 1
                "#,
            )
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.deal_client", &error))?
            {
                people.push(FormSignerPerson {
                    person_id: client.person_id,
                    name: client.display_name,
                    email: compact(client.email),
                    role: client.role,
                });
            }

            let participants = sqlx::query_as::<_, SignerRow>(
                r#"
                select fp.person_id::text as person_id,
                       fp.display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = fp.person_id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       fp.role
                from document_form_participant fp
                where fp.form_instance_id = $1::uuid
                order by fp.sort_order asc, fp.display_name
                "#,
            )
            .bind(form_instance_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.form_participants", &error))?;

            for row in participants {
                people.push(FormSignerPerson {
                    person_id: row.person_id,
                    name: row.display_name,
                    email: compact(row.email),
                    role: role_for_form(&form.template_id, &row.role),
                });
            }

            let deal_people = sqlx::query_as::<_, SignerRow>(
                r#"
                select dp.person_id::text as person_id,
                       coalesce(person.display_name, dp.role) as display_name,
                       (
                         select pi.identity_value
                         from person_identity pi
                         where pi.person_id = dp.person_id and pi.identity_type = 'email'
                         order by pi.is_primary desc, pi.created_at asc
                         limit 1
                       ) as email,
                       dp.role
                from deal_participant dp
                left join person on person.id = dp.person_id
                where dp.deal_id = $1::uuid and dp.active = true
                order by dp.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.deal_participants", &error))?;

            for row in deal_people {
                people.push(FormSignerPerson {
                    person_id: row.person_id,
                    name: row.display_name,
                    email: compact(row.email),
                    role: role_for_form(&form.template_id, &row.role),
                });
            }
        }

        if matches!(form.template_id.as_str(), "LISTING-01" | "PR-PNS") {
            let brokers = sqlx::query_as::<_, BrokerRow>(
                r#"
                select person_id::text as person_id, email
                from app_user
                where active = true and lower(display_name) = lower($1)
                order by id
                limit 2
                "#,
            )
            .bind(SELLER_BROKER_NAME)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("form.signers.broker", &error))?;

            if brokers.len() != 1 {
                return Err(DbFailure::schema_mismatch(
                    "form.signers.broker",
                    format!(
                        "{} signer resolution requires exactly one active {SELLER_BROKER_NAME} app user",
                        form.template_id
                    ),
                ));
            }
            let broker = brokers.into_iter().next().expect("checked len == 1");
            people.push(FormSignerPerson {
                person_id: broker.person_id,
                name: SELLER_BROKER_NAME.into(),
                email: compact(broker.email),
                role: "SELLER_BROKER".into(),
            });
        }

        Ok(people)
    }
}
