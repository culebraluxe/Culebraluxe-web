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
            // A template whose blocks name their party's email is signed by those parties, one slot per block, named the
            // way the document's own blocks are; every other template keeps the people linked to the form.
            let participants = match template_party_signers(
                &form.template_id,
                form.template_version,
                &string_map(form.field_values.clone()),
            )? {
                Some(parties) => parties,
                None => list_signers_on(tx.connection(), &form.id).await?,
            };
            let form_values = string_map(form.field_values.clone());

            // ONE RENDER REQUEST, built before the signature is resolved and carried into both halves. The resolution
            // reads exactly the values, participants and issuance instant that will be rendered, so a signature can
            // never be placed against a document the caller did not ask for.
            let mut render_request = VaultRenderRequest {
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
                applied_signatures: Vec::new(),
            };

            // THE BROKER'S PRE-SIGNATURE. Where the document names Lisa as the brokerage's signer and the person issuing is
            // her (or a ROOT delegate), her signature, initials and date are drawn into the PDF NOW, so the sellers
            // receive a document she has already signed. Nothing here is optional for the policy: a template with no
            // policy, or a broker line that is not hers, simply yields no signature — while a policy this document
            // cannot satisfy (no issuance instant, no authority) stops the issuance rather than issuing it unsigned.
            //
            // Resolved through the DAO's own method, which is the same one the browser preview calls: ONE
            // implementation of the policy, the declared-signer check, the slot mapping and the authority rule, so the
            // draft on screen and the document that is issued cannot disagree about her line.
            let applied_signatures = match self
                .resolve_applied_signatures_in(
                    tx.connection(),
                    &render_request,
                    model::forms_broker_signature::SignatureAuthority::ApplyingActor,
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
            render_request.applied_signatures = applied_signatures;

            let artifact = match render(render_request).await {
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
    /// THE BROKER'S PRE-SIGNATURE, resolved against a render request: the ONE implementation of the policy.
    ///
    /// Both halves of the feature call this — the issuance path above, inside its transaction and with the receipt it
    /// has to finalize on refusal, and the browser preview. A preview can therefore only ever show what issuance would
    /// apply: the same template policy, the same declared-signer check, the same slot mapping, the same authority rule.
    ///
    /// The slot is optional (`false` below): a participant row whose `slotId` is null is still matched at her own block
    /// by role rather than refused, which is the behaviour the issued record has always had.
    ///
    /// An `Err` here is a REFUSAL, never an empty answer — policy refuses to issue unsigned, and a caller that drew
    /// nothing would hide the defect that makes the document unissuable. What a refusal COSTS is the caller's decision
    /// at its own seam: the issuance path finalizes a refused receipt (`PreconditionFailure` for rows it could not
    /// reach, `ValidationFailure`/`Unauthorized` for policy), and the preview's service method maps the first to
    /// infrastructure and the rest to a business refusal (`web/src/vault/mod.rs::resolve_applied_signatures`).
    pub(crate) async fn resolve_applied_signatures_in(
        &self,
        connection: &mut PgConnection,
        request: &VaultRenderRequest,
        authority: model::forms_broker_signature::SignatureAuthority,
    ) -> Result<Vec<model::forms_applied_signature::FormAppliedSignature>, VaultArtifactFailure>
    {
        crate::broker_signature::resolve_for_issuance(
            connection,
            &request.template_id,
            &request.field_values,
            &model::forms_execution::slots_from_signers(&request.participants),
            request.actor_app_user_id.as_deref(),
            authority,
            request.issued_at.as_deref(),
            false,
        )
        .await
    }

    /// The same resolution for a caller that has no transaction to be part of: it takes its own connection.
    ///
    /// The preview is not an issuance, and it must not be handed a connection it never asked for, so it acquires one
    /// here instead (`self.db`, the process's pool).
    ///
    /// It resolves under `OwnerByConstruction`: a preview applies nothing, so it is asked for no application authority,
    /// while every other rule — the template policy, the declared-signer check, the slot mapping, the protected asset —
    /// is the one issuance obeys.
    pub async fn resolve_applied_signatures(
        &self,
        request: &VaultRenderRequest,
    ) -> Result<Vec<model::forms_applied_signature::FormAppliedSignature>, VaultArtifactFailure>
    {
        let mut connection = self
            .db
            .connection()
            .await
            .map_err(crate::broker_signature::precondition_failure)?;
        self.resolve_applied_signatures_in(
            &mut connection,
            request,
            model::forms_broker_signature::SignatureAuthority::OwnerByConstruction,
        )
        .await
    }
}

/// The parties a template sends to, from the blocks that name an email: `None` for a template without them.
fn template_party_signers(
    template_id: &str,
    template_version: i32,
    field_values: &std::collections::BTreeMap<String, String>,
) -> DbResult<Option<Vec<model::FormSignerPerson>>> {
    let library = model::forms_template::TemplateLibrary::load_default()
        .map_err(|error| DbFailure::schema_mismatch("vault.issue.template", format!("{error}")))?;
    let Some(template) = library.version(template_id, template_version) else {
        return Ok(None);
    };
    if template
        .signature_groups
        .iter()
        .all(|group| group.email.is_none())
    {
        return Ok(None);
    }
    let value = |name: &str| {
        field_values
            .get(name)
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    let parties = template
        .signature_groups
        .iter()
        .filter(|group| group.email.is_some())
        .map(|group| {
            let name = group
                .field
                .as_deref()
                .map(value)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| group.label.clone());
            let email = group
                .email
                .as_deref()
                .map(value)
                .filter(|email| !email.is_empty());
            model::FormSignerPerson {
                person_id: None,
                name,
                email,
                role: group.role.clone(),
                slot_id: Some(format!("{}:1", group.role)),
            }
        })
        .collect();
    Ok(Some(parties))
}
