//! Moved from `vault.rs` (move only): new.

#[allow(unused_imports)]
use super::*;

impl VaultDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn list_issued_documents(
        &self,
        actor: Option<&VaultActorScope>,
    ) -> DbResult<Vec<IssuedDocumentListItem>> {
        let external = actor.is_some_and(|actor| actor.account_type == "external");
        let person_id = actor.and_then(|actor| actor.person_id.as_deref());
        let rows = sqlx::query_as::<_, IssuedListRow>(
            r#"
            select td.id::text as id,
                   td.deal_id::text as deal_id,
                   -- A contract made without a deal (a listing agreement) still names its property: on its form.
                   coalesce(pr.id, fpr.id)::text as property_id,
                   td.document_type_label,
                   td.title,
                   td.state,
                   td.template_id,
                   td.template_version,
                   td.issued_version,
                   td.issued_checksum_sha256,
                   u.display_name as issued_by_display_name,
                   p.display_name as party_name,
                   coalesce(pr.name, fpr.name) as property_name,
                   null::text as deal_name,
                   td.created_at,
                   td.signed_media_id::text as signed_media_id,
                   td.signed_audit_media_id::text as signed_audit_media_id,
                   coalesce(td.party_person_id, fi.person_id)::text as party_person_id,
                   td.signed_at,
                   td.form_instance_id::text as form_instance_id
            from transaction_document td
            left join app_user u on u.id = td.prepared_by_user_id
            left join person p on p.id = td.party_person_id
            left join deal d on d.id = td.deal_id
            left join property pr on pr.id = d.property_id
            left join document_form_instance fi on fi.id = td.form_instance_id
            left join property fpr on fpr.id = fi.property_id
            where td.source = 'generated'
              and td.template_id is not null
              and ($1::boolean = false
                   or td.deal_id in (
                       select dp.deal_id
                       from deal_participant dp
                       where dp.person_id = $2::uuid
                         and dp.ended_at is null
                   ))
            order by td.created_at desc, td.id
            "#,
        )
        .bind(external)
        .bind(person_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.list_issued_documents", &error))?;

        rows.into_iter()
            .map(|row| {
                Ok(IssuedDocumentListItem {
                    id: row.id,
                    deal_id: row.deal_id,
                    property_id: row.property_id,
                    document_type_label: compact(row.document_type_label),
                    title: compact(row.title),
                    state: TransactionDocumentState::try_from(row.state.as_str()).map_err(
                        |error| DbFailure::schema_mismatch("vault.list_issued_documents", error),
                    )?,
                    template_id: row.template_id,
                    template_version: row.template_version,
                    issued_version: row.issued_version,
                    issued_checksum_sha256: row.issued_checksum_sha256,
                    issued_by_display_name: compact(row.issued_by_display_name),
                    party_name: compact(row.party_name),
                    property_name: compact(row.property_name),
                    deal_name: compact(row.deal_name),
                    created_at: row.created_at.to_rfc3339(),
                    signed_artifact_available: row.signed_media_id.is_some(),
                    signed_audit_available: row.signed_audit_media_id.is_some(),
                    party_person_id: row.party_person_id,
                    signed_at: row.signed_at.map(|at| at.to_rfc3339()),
                    form_instance_id: row.form_instance_id,
                })
            })
            .collect()
    }

    pub async fn get_document(&self, document_id: &str) -> DbResult<Option<TransactionDocument>> {
        let row = sqlx::query_as::<_, DocumentRow>(
            r#"
            select id::text as id, deal_id::text as deal_id, document_type,
                   document_type_label, title, state, source, source_system,
                   source_external_id, prepared_by_user_id::text as prepared_by_user_id,
                   party_person_id::text as party_person_id, media_id::text as media_id,
                   signed_media_id::text as signed_media_id,
                   signed_audit_media_id::text as signed_audit_media_id,
                   signed_at, supersedes_document_id::text as supersedes_document_id,
                   issued_checksum_sha256, template_id, template_version,
                   source_snapshot, issued_version, form_instance_id::text as form_instance_id,
                   created_at, updated_at
            from transaction_document
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(document_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.get_document", &error))?;
        row.map(map_document).transpose()
    }

    pub async fn list_by_deal(&self, deal_id: &str) -> DbResult<Vec<TransactionDocument>> {
        let rows = sqlx::query_as::<_, DocumentRow>(
            r#"
            select id::text as id, deal_id::text as deal_id, document_type,
                   document_type_label, title, state, source, source_system,
                   source_external_id, prepared_by_user_id::text as prepared_by_user_id,
                   party_person_id::text as party_person_id, media_id::text as media_id,
                   signed_media_id::text as signed_media_id,
                   signed_audit_media_id::text as signed_audit_media_id,
                   signed_at, supersedes_document_id::text as supersedes_document_id,
                   issued_checksum_sha256, template_id, template_version,
                   source_snapshot, issued_version, form_instance_id::text as form_instance_id,
                   created_at, updated_at
            from transaction_document
            where deal_id = $1::uuid
            order by created_at asc, id
            "#,
        )
        .bind(deal_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.list_by_deal", &error))?;
        rows.into_iter().map(map_document).collect()
    }

    pub async fn create_document(
        &self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument> {
        let signed_media_id = request
            .signed_artifact
            .as_ref()
            .map(|signed| signed.media_id.as_str());
        let signed_at = request
            .signed_artifact
            .as_ref()
            .map(|signed| signed.signed_at.as_str());
        let evidence = request.issued_evidence.as_ref();
        let row = sqlx::query_as::<_, DocumentRow>(
            r#"
            insert into transaction_document (
                deal_id, document_type, document_type_label, title, state, source,
                source_system, source_external_id, prepared_by_user_id, party_person_id,
                media_id, signed_media_id, signed_at, supersedes_document_id,
                issued_checksum_sha256, template_id, template_version, source_snapshot,
                issued_version, form_instance_id
            )
            values (
                $1::uuid,$2,$3,$4,$5,$6,$7,$8,$9::uuid,$10::uuid,
                $11::uuid,$12::uuid,$13::timestamptz,$14::uuid,
                $15,$16,$17,$18,$19,$20::uuid
            )
            on conflict (deal_id, source_system, source_external_id)
                where source_external_id is not null
                do nothing
            returning id::text as id, deal_id::text as deal_id, document_type,
                      document_type_label, title, state, source, source_system,
                      source_external_id, prepared_by_user_id::text as prepared_by_user_id,
                      party_person_id::text as party_person_id, media_id::text as media_id,
                      signed_media_id::text as signed_media_id,
                      signed_audit_media_id::text as signed_audit_media_id,
                      signed_at, supersedes_document_id::text as supersedes_document_id,
                      issued_checksum_sha256, template_id, template_version,
                      source_snapshot, issued_version, form_instance_id::text as form_instance_id,
                      created_at, updated_at
            "#,
        )
        .bind(request.deal_id.as_deref())
        .bind(request.document_type.as_str())
        .bind(request.document_type_label.as_deref())
        .bind(request.title.as_deref())
        .bind(request.state.as_str())
        .bind(request.source.as_str())
        .bind(request.source_system.as_deref())
        .bind(request.source_external_id.as_deref())
        .bind(request.prepared_by_user_id.as_deref())
        .bind(request.party_person_id.as_deref())
        .bind(request.media_id.as_deref())
        .bind(signed_media_id)
        .bind(signed_at)
        .bind(request.supersedes_document_id.as_deref())
        .bind(evidence.map(|value| value.checksum_sha256.as_str()))
        .bind(evidence.map(|value| value.template_id.as_str()))
        .bind(evidence.map(|value| value.template_version))
        .bind(evidence.map(|value| value.source_snapshot.clone()))
        .bind(evidence.map(|value| value.issued_version))
        .bind(evidence.map(|value| value.form_instance_id.as_str()))
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.create_document", &error))?;

        if let Some(row) = row {
            return map_document(row);
        }

        let existing = sqlx::query_as::<_, DocumentRow>(
            r#"
            select id::text as id, deal_id::text as deal_id, document_type,
                   document_type_label, title, state, source, source_system,
                   source_external_id, prepared_by_user_id::text as prepared_by_user_id,
                   party_person_id::text as party_person_id, media_id::text as media_id,
                   signed_media_id::text as signed_media_id,
                   signed_audit_media_id::text as signed_audit_media_id,
                   signed_at, supersedes_document_id::text as supersedes_document_id,
                   issued_checksum_sha256, template_id, template_version,
                   source_snapshot, issued_version, form_instance_id::text as form_instance_id,
                   created_at, updated_at
            from transaction_document
            where deal_id is not distinct from $1::uuid
              and source_system = $2
              and source_external_id = $3
            order by created_at asc, id
            limit 1
            "#,
        )
        .bind(request.deal_id.as_deref())
        .bind(request.source_system.as_deref())
        .bind(request.source_external_id.as_deref())
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.create_document.replay", &error))?
        .ok_or_else(|| {
            DbFailure::schema_mismatch(
                "vault.create_document.replay",
                "source idempotency conflict returned no existing document",
            )
        })?;
        map_document(existing)
    }

    pub async fn transition_state(
        &self,
        request: &TransitionTransactionDocumentRequest,
    ) -> DbResult<VaultCommandResult> {
        let mut tx = self.db.begin("vault.transition_state").await?;
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

            let current = sqlx::query_scalar::<_, String>(
                "select state from transaction_document where id = $1::uuid limit 1",
            )
            .bind(&request.document_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.transition_state.read", &error))?;

            let (outcome, aggregate_id, message) = match current {
                None => (
                    VaultCommandOutcome::NotFound,
                    None,
                    Some("Transaction document not found.".to_owned()),
                ),
                Some(current) => {
                    let from =
                        TransactionDocumentState::try_from(current.as_str()).map_err(|error| {
                            DbFailure::schema_mismatch("vault.transition_state", error)
                        })?;
                    if !from.can_transition_to(request.to) {
                        (
                            VaultCommandOutcome::ValidationFailure,
                            None,
                            Some(format!(
                                "Transition {} -> {} is not allowed.",
                                from.as_str(),
                                request.to.as_str()
                            )),
                        )
                    } else if request.to == TransactionDocumentState::Signed
                        && request.signed_artifact.is_none()
                    {
                        (
                            VaultCommandOutcome::ValidationFailure,
                            None,
                            Some(
                                "Signing requires a new signed artifact (media id + signed time)."
                                    .into(),
                            ),
                        )
                    } else {
                        let signed_media_id = request
                            .signed_artifact
                            .as_ref()
                            .map(|signed| signed.media_id.as_str());
                        let signed_at = request
                            .signed_artifact
                            .as_ref()
                            .map(|signed| signed.signed_at.as_str());
                        let updated = sqlx::query_scalar::<_, String>(
                            r#"
                            update transaction_document
                            set state = $2,
                                signed_media_id = case when $2 = 'signed'
                                  then $3::uuid else signed_media_id end,
                                signed_at = case when $2 = 'signed'
                                  then $4::timestamptz else signed_at end,
                                updated_at = now()
                            where id = $1::uuid and state = $5
                            returning id::text
                            "#,
                        )
                        .bind(&request.document_id)
                        .bind(request.to.as_str())
                        .bind(signed_media_id)
                        .bind(signed_at)
                        .bind(from.as_str())
                        .fetch_optional(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("vault.transition_state.update", &error)
                        })?;
                        if updated.is_some() {
                            (
                                VaultCommandOutcome::Success,
                                Some(request.document_id.clone()),
                                None,
                            )
                        } else {
                            (
                                VaultCommandOutcome::Conflict,
                                None,
                                Some(format!(
                                    "State changed concurrently for document {}.",
                                    request.document_id
                                )),
                            )
                        }
                    }
                }
            };

            finalize_receipt(
                &mut tx,
                &request.command_id,
                &outcome,
                aggregate_id.as_deref(),
                message.as_deref(),
                request.actor_app_user_id.as_deref(),
            )
            .await?;
            Ok(outcome_result(
                &request.command_id,
                outcome,
                aggregate_id,
                message,
                false,
            ))
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

    pub async fn issued_for_form_instance(
        &self,
        form_instance_id: &str,
    ) -> DbResult<Option<IssuedDocumentForFormInstance>> {
        let row = sqlx::query_as::<_, IssuedFormRow>(
            r#"
            select id::text as document_id,
                   issued_version,
                   issued_checksum_sha256 as checksum,
                   created_at,
                   media_id::text as media_id,
                   source_snapshot
            from transaction_document
            where form_instance_id = $1::uuid
              and source = 'generated'
            order by issued_version desc nulls last, created_at desc
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issued_for_form_instance", &error))?;
        Ok(row.map(|row| IssuedDocumentForFormInstance {
            document_id: row.document_id,
            issued_version: row.issued_version,
            checksum: row.checksum,
            created_at: row.created_at.to_rfc3339(),
            media_id: row.media_id,
            source_snapshot: row.source_snapshot,
        }))
    }

    pub async fn next_issued_version(&self, request: &NextIssuedVersionRequest) -> DbResult<i32> {
        let current = if let Some(contract_id) = request.contract_id.as_deref() {
            sqlx::query_scalar::<_, i32>(
                r#"
                select issued_version
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
            .bind(request.template_id.trim())
            .fetch_optional(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.next_version.contract", &error))?
        } else {
            sqlx::query_scalar::<_, i32>(
                r#"
                select issued_version
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
            .bind(request.deal_id.as_deref())
            .bind(request.template_id.trim())
            .fetch_optional(&mut *self.db.connection().await?)
            .await
            .map_err(|error| DbFailure::from_sqlx("vault.next_version.legacy", &error))?
        };
        Ok(current.unwrap_or(0) + 1)
    }

    pub async fn media_bytes(&self, media_id: &str) -> DbResult<Option<VaultMediaBytes>> {
        let row = sqlx::query_as::<_, MediaRow>(
            r#"
            select file_data, filename, mime_type
            from media
            where id = $1::uuid and media_type = 'document' and file_data is not null
            limit 1
            "#,
        )
        .bind(media_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.media_bytes", &error))?;
        Ok(row.map(|row| VaultMediaBytes {
            bytes: row.file_data.unwrap_or_default(),
            filename: row.filename,
            mime_type: row.mime_type,
        }))
    }

    /// A guest may receive bytes only when every link is to a live public listing
    /// as a document and the asset has no transaction-document lineage.
    pub async fn public_listing_document_bytes(
        &self,
        media_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        let row = sqlx::query_as::<_, MediaRow>(
            r#"
            select m.file_data, m.filename, m.mime_type
            from media m
            where m.id = $1::uuid
              and m.media_type = 'document'
              and lower(split_part(m.mime_type, ';', 1)) = 'application/pdf'
              and m.file_data is not null
              and exists (
                  select 1 from property_media pm
                  join property p on p.id = pm.property_id
                  where pm.media_id = m.id
                    and pm.role = 'document'
                    and p.status in ('active', 'under_contract', 'sold')
                    and p.archived_at is null
              )
              and not exists (
                  select 1 from property_media pm
                  left join property p on p.id = pm.property_id
                  where pm.media_id = m.id
                    and (pm.role <> 'document' or p.id is null
                      or p.is_published is distinct from true
                      or p.is_active_listing is distinct from true
                      or p.archived_at is not null)
              )
              and not exists (
                  select 1 from transaction_document td
                  where td.media_id = m.id
                     or td.signed_media_id = m.id
                     or td.signed_audit_media_id = m.id
              )
            limit 1
            "#,
        )
        .bind(media_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.public_listing_document_bytes", &error))?;
        Ok(row.map(|row| VaultMediaBytes {
            bytes: row.file_data.unwrap_or_default(),
            filename: row.filename,
            mime_type: row.mime_type,
        }))
    }

    /// The document a signer is being asked to sign, and nothing else.
    ///
    /// The signing door proves its own entitlement in SQL, the way the anonymous listing door does: the recipient must
    /// belong to THIS request, the request must have been ISSUED (a draft is the operator's, not a signer's) and still be
    /// live (declined, voided, expired and errored envelopes show nothing), and the bytes are the request's own
    /// transaction document — a PDF stored as a `document`. A recipient id from another envelope, or an envelope that was
    /// never sent, answers `None`.
    pub async fn signing_document_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        let row = sqlx::query_as::<_, MediaRow>(
            r#"
            select m.file_data, m.filename, m.mime_type
            from luxesign_envelope_recipient r
            join luxesign_request sr on sr.id = r.signature_request_id
            join luxesign_config dsr on dsr.signature_request_id = sr.id
            join transaction_document td on td.id = sr.transaction_document_id
            join media m on m.id = td.media_id
            where r.id = $2::uuid
              and sr.id = $1::uuid
              and dsr.issued_at is not null
              and sr.status not in ('declined', 'voided', 'expired', 'error')
              and m.media_type = 'document'
              and lower(split_part(m.mime_type, ';', 1)) = 'application/pdf'
              and m.file_data is not null
            limit 1
            "#,
        )
        .bind(signature_request_id)
        .bind(recipient_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.signing_document_bytes", &error))?;
        Ok(row.map(|row| VaultMediaBytes {
            bytes: row.file_data.unwrap_or_default(),
            filename: row.filename,
            mime_type: row.mime_type,
        }))
    }

    /// The SEALED copy a signer may keep, once the envelope they were part of is completed. Same shape of proof as
    /// `signing_document_bytes`: the recipient must belong to the envelope, and the envelope must be `completed` with a
    /// sealed PDF linked. Anything else answers `None`.
    pub async fn signing_signed_bytes(
        &self,
        signature_request_id: &str,
        recipient_id: &str,
    ) -> DbResult<Option<VaultMediaBytes>> {
        let row = sqlx::query_as::<_, MediaRow>(
            r#"
            select m.file_data, m.filename, m.mime_type
            from luxesign_envelope_recipient r
            join luxesign_request sr on sr.id = r.signature_request_id
            join transaction_document td on td.id = sr.transaction_document_id
            join media m on m.id = td.signed_media_id
            where r.id = $2::uuid
              and sr.id = $1::uuid
              and sr.status = 'completed'
              and m.media_type = 'document'
              and m.file_data is not null
            limit 1
            "#,
        )
        .bind(signature_request_id)
        .bind(recipient_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.signing_signed_bytes", &error))?;
        Ok(row.map(|row| VaultMediaBytes {
            bytes: row.file_data.unwrap_or_default(),
            filename: row.filename,
            mime_type: row.mime_type,
        }))
    }

    /// What a completion email carries: the sealed document first, then its certificate. Only a `completed`
    /// envelope has either, and only the artifacts the finalize step linked.
    pub async fn completion_artifacts(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<VaultMediaBytes>> {
        let rows = sqlx::query_as::<_, MediaRow>(
            r#"
            select m.file_data, m.filename, m.mime_type
            from luxesign_request sr
            join transaction_document td on td.id = sr.transaction_document_id
            join media m on m.id in (td.signed_media_id, td.signed_audit_media_id)
            where sr.id = $1::uuid
              and sr.status = 'completed'
              and m.media_type = 'document'
              and m.file_data is not null
            order by (m.id = td.signed_media_id) desc
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.completion_artifacts", &error))?;
        Ok(rows
            .into_iter()
            .map(|row| VaultMediaBytes {
                bytes: row.file_data.unwrap_or_default(),
                filename: row.filename,
                mime_type: row.mime_type,
            })
            .collect())
    }

    pub async fn form_contract_id(&self, form_instance_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            select contract_id::text
            from document_form_instance
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(form_instance_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map(|value| value.flatten())
        .map_err(|error| DbFailure::from_sqlx("vault.form_contract_id", &error))
    }
}
