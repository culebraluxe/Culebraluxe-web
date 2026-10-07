//! Moved from `vault.rs` (move only): bind_form_to_contract.

#[allow(unused_imports)]
use super::*;

impl VaultDao {
    pub async fn bind_form_to_contract(
        &self,
        form_instance_id: &str,
        contract_id: &str,
    ) -> DbResult<bool> {
        let id = sqlx::query_scalar::<_, String>(
            r#"
            update document_form_instance
            set contract_id = $2::uuid, updated_at = now()
            where id = $1::uuid
              and (contract_id is null or contract_id = $2::uuid)
            returning id::text
            "#,
        )
        .bind(form_instance_id)
        .bind(contract_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.bind_form_to_contract", &error))?;
        Ok(id.is_some())
    }

    pub async fn prior_contract_document(
        &self,
        contract_id: &str,
        template_id: &str,
    ) -> DbResult<Option<ContractIssuedLineage>> {
        let row = sqlx::query_as::<_, (String, i32)>(
            r#"
            select id::text, issued_version
            from transaction_document
            where contract_id = $1::uuid
              and template_id = $2
              and source = 'generated'
              and issued_version is not null
            order by issued_version desc, created_at desc
            limit 1
            "#,
        )
        .bind(contract_id)
        .bind(template_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.prior_contract_document", &error))?;

        Ok(row.map(|(id, issued_version)| ContractIssuedLineage { id, issued_version }))
    }

    pub async fn issue_from_form_instance<F, Fut>(
        &self,
        request: &IssueDocumentRequest,
        render: F,
    ) -> DbResult<VaultCommandResult>
    where
        F: FnOnce(VaultRenderRequest) -> Fut + Send,
        Fut: Future<Output = Result<VaultRenderedArtifact, VaultArtifactFailure>> + Send,
    {
        let mut tx = self.db.begin("vault.issue_from_form_instance").await?;
        let result = async {
            if !claim_receipt(
                &mut tx,
                &request.command_id,
                request.actor_app_user_id.as_deref(),
            )
            .await?
            {
                let receipt = read_receipt(&mut tx, &request.command_id).await?;
                return Ok(replay_from_receipt(&request.command_id, receipt));
            }

            let form = sqlx::query_as::<_, IssueFormRow>(
                r#"
                select id::text as id,
                       template_id,
                       template_version,
                       deal_id::text as deal_id,
                       person_id::text as person_id,
                       contract_id::text as contract_id,
                       field_values,
                       sections
                from document_form_instance
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(&request.form_instance_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.issue.form", &error))?;

            let Some(form) = form else {
                let outcome = VaultCommandOutcome::ValidationFailure;
                let message = "document.issue failed: form instance not found.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &request.command_id,
                    &outcome,
                    None,
                    Some(&message),
                    request.actor_app_user_id.as_deref(),
                )
                .await?;
                return Ok(outcome_result(
                    &request.command_id,
                    outcome,
                    None,
                    Some(message),
                    false,
                ));
            };

            let prior = if let Some(contract_id) = form.contract_id.as_deref() {
                sqlx::query_as::<_, (String, i32)>(
                    r#"
                    select id::text, issued_version
                    from transaction_document
                    where contract_id = $1::uuid
                      and template_id = $2
                      and source = 'generated'
                      and issued_version is not null
                    order by issued_version desc, created_at desc
                    limit 1
                    "#,
                )
                .bind(contract_id)
                .bind(&form.template_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("vault.issue.prior_contract", &error))?
            } else {
                sqlx::query_as::<_, (String, i32)>(
                    r#"
                    select id::text, issued_version
                    from transaction_document
                    where deal_id is not distinct from $1::uuid
                      and contract_id is null
                      and template_id = $2
                      and source = 'generated'
                      and issued_version is not null
                    order by issued_version desc, created_at desc
                    limit 1
                    "#,
                )
                .bind(form.deal_id.as_deref())
                .bind(&form.template_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("vault.issue.prior_legacy", &error))?
            };
            let issued_version = prior.as_ref().map_or(1, |(_, version)| version + 1);
            let supersedes_id = prior.as_ref().map(|(id, _)| id.clone());
            let participants = list_signers_on(tx.connection(), &form.id).await?;

            // THE BROKER'S PRE-SIGNATURE. Where the document names Lisa as the brokerage's signer and the person issuing is
            // her (or a ROOT delegate), her signature, initials and date are drawn into the PDF NOW, so the sellers
            // receive a document she has already signed. Nothing here is optional for the policy: a template with no
            // policy, or a broker line that is not hers, simply yields no signature.
            let form_values = string_map(form.field_values.clone());
            let slots: Vec<model::forms_execution::IssuedExecutionSlot> = participants
                .iter()
                .enumerate()
                .filter_map(|(order, person)| {
                    person.slot_id.clone().map(|slot_id| {
                        model::forms_execution::IssuedExecutionSlot {
                            slot_id,
                            role: person.role.clone(),
                            person_id: person.person_id.clone(),
                            name: person.name.clone(),
                            email: person.email.clone(),
                            required: true,
                            order,
                        }
                    })
                })
                .collect();
            let applied_signatures = match crate::broker_signature::resolve_for_issuance(
                tx.connection(),
                &form.template_id,
                &form_values,
                &slots,
                request.actor_app_user_id.as_deref(),
                request.issued_at.as_deref(),
                // The slot is optional here: a document whose participants carry no execution slot is still signed by
                // her at her own block (matched by role), rather than refused.
                false,
            )
            .await
            {
                Ok(applied) => applied,
                Err(failure) => {
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        &failure.outcome,
                        None,
                        Some(&failure.message),
                        request.actor_app_user_id.as_deref(),
                    )
                    .await?;
                    return Ok(outcome_result(
                        &request.command_id,
                        failure.outcome,
                        None,
                        Some(failure.message),
                        false,
                    ));
                }
            };

            let artifact = match render(VaultRenderRequest {
                form_instance_id: form.id.clone(),
                contract_id: form.contract_id.clone(),
                template_id: form.template_id.clone(),
                template_version: form.template_version,
                field_values: form_values.clone(),
                sections: string_map(form.sections.clone()),
                issued_version,
                participants: participants.clone(),
                actor_app_user_id: request.actor_app_user_id.clone(),
                issued_at: request.issued_at.clone(),
                applied_signatures,
            })
            .await
            {
                Ok(artifact) => artifact,
                Err(failure) => {
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        &failure.outcome,
                        None,
                        Some(&failure.message),
                        request.actor_app_user_id.as_deref(),
                    )
                    .await?;
                    return Ok(outcome_result(
                        &request.command_id,
                        failure.outcome,
                        None,
                        Some(failure.message),
                        false,
                    ));
                }
            };

            let checksum = Sha256::digest(&artifact.bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();

            let media_id = sqlx::query_scalar::<_, String>(
                r#"
                insert into media (file_data, filename, mime_type, file_size, media_type)
                values ($1,$2,'application/pdf',$3,'document')
                returning id::text
                "#,
            )
            .bind(&artifact.bytes)
            .bind(&artifact.filename)
            .bind(artifact.bytes.len() as i64)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.issue.media", &error))?;

            if let Some(supersedes_id) = supersedes_id.as_deref() {
                sqlx::query(
                    r#"
                    update transaction_document
                    set state = 'superseded', updated_at = now()
                    where id = $1::uuid and state <> 'superseded'
                    "#,
                )
                .bind(supersedes_id)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("vault.issue.supersede", &error))?;
            }

            let source_snapshot = json!({
                "contractId": form.contract_id.clone(),
                "templateId": form.template_id.clone(),
                "templateVersion": form.template_version,
                "fieldValues": string_map(form.field_values),
                "sections": string_map(form.sections),
                "issuedParticipants": participants,
                "render": artifact.render_metadata,
            });

            let party_person_id = if form.contract_id.is_none() {
                if form.person_id.is_some() {
                    form.person_id.clone()
                } else if let Some(deal_id) = form.deal_id.as_deref() {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        select person_id::text
                        from deal_participant
                        where deal_id = $1::uuid
                          and role = 'client'
                          and person_id is not null
                          and active
                        order by created_at asc, id
                        limit 1
                        "#,
                    )
                    .bind(deal_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("vault.issue.party", &error))?
                } else {
                    None
                }
            } else {
                None
            };

            let row = sqlx::query_as::<_, (String, DateTime<Utc>)>(
                r#"
                insert into transaction_document (
                    contract_id, deal_id, document_type, document_type_label, title, state,
                    source, prepared_by_user_id, party_person_id, media_id,
                    supersedes_document_id, issued_checksum_sha256, template_id,
                    template_version, source_snapshot, issued_version, form_instance_id
                )
                values (
                    $1::uuid,$2::uuid,'agreement',$3,$4,'ready',
                    'generated',$5::uuid,$6::uuid,$7::uuid,
                    $8::uuid,$9,$10,$11,$12,$13,$14::uuid
                )
                returning id::text, created_at
                "#,
            )
            .bind(form.contract_id.as_deref())
            .bind(if form.contract_id.is_some() {
                None
            } else {
                form.deal_id.as_deref()
            })
            .bind(&artifact.document_type_label)
            .bind(format!("{} v{}", artifact.display_name, issued_version))
            .bind(request.actor_app_user_id.as_deref())
            .bind(party_person_id.as_deref())
            .bind(&media_id)
            .bind(supersedes_id.as_deref())
            .bind(&checksum)
            .bind(&form.template_id)
            .bind(form.template_version)
            .bind(source_snapshot.clone())
            .bind(issued_version)
            .bind(&form.id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.issue.document", &error))?;

            sqlx::query(
                r#"
                update document_form_instance
                set status = 'issued', updated_at = now()
                where id = $1::uuid
                "#,
            )
            .bind(&form.id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.issue.mark_form", &error))?;

            let outcome = VaultCommandOutcome::Success;
            finalize_receipt(
                &mut tx,
                &request.command_id,
                &outcome,
                Some(&row.0),
                None,
                request.actor_app_user_id.as_deref(),
            )
            .await?;

            Ok(VaultCommandResult {
                command_id: request.command_id.clone(),
                outcome,
                aggregate_id: Some(row.0.clone()),
                message: None,
                replayed: false,
                value: Some(json!({
                    "documentId": row.0,
                    "mediaId": media_id,
                    "issuedVersion": issued_version,
                    "checksum": checksum,
                    "issuedAt": row.1.to_rfc3339(),
                })),
            })
        }
        .await;

        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}
