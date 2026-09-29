//! Moved from `forms.rs` (move only): new.

#[allow(unused_imports)]
use super::*;

impl FormDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn create_instance(
        &self,
        request: &CreateFormInstanceRequest,
    ) -> DbResult<FormInstance> {
        let field_values = serde_json::to_value(&request.field_values).map_err(|error| {
            DbFailure::schema_mismatch("form.create.serialize_fields", error.to_string())
        })?;
        let sections = serde_json::to_value(&request.sections).map_err(|error| {
            DbFailure::schema_mismatch("form.create.serialize_sections", error.to_string())
        })?;

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
        .fetch_one(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
            .map_err(|error| {
                DbFailure::schema_mismatch("form.update.serialize_fields", error.to_string())
            })?;
        let sections = request
            .input
            .sections
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                DbFailure::schema_mismatch("form.update.serialize_sections", error.to_string())
            })?;

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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_all(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map(|value| value.flatten())
        .map_err(|error| DbFailure::from_sqlx("form.get_showing_id", &error))
    }

    pub async fn bind_showing(&self, request: &BindFormInstanceToShowingRequest) -> DbResult<bool> {
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
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("form.bind_showing", &error))?;

        Ok(id.is_some())
    }

}
