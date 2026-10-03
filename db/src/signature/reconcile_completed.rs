//! Moved from `signature.rs` (move only): reconcile_completed, insert_signature_media.

#[allow(unused_imports)]
use super::*;

impl SignatureDao {
    pub async fn reconcile_completed(
        &self,
        event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<&SignatureArtifactDownload>,
        audit_artifact: Option<&SignatureArtifactDownload>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        let command_id = format!("signature.reconcile:{event_id}");
        let mut tx = self.db.begin("signature.reconcile_completed").await?;
        let result = async {
            if !claim_receipt(&mut tx, &command_id, actor_app_user_id).await? {
                let receipt = read_receipt(&mut tx, &command_id).await?;
                return Ok(replay_result(&command_id, receipt));
            }

            let row = sqlx::query_as::<_, ReconcileRow>(
                r#"
                select td.id::text as document_id,
                       td.state,
                       td.signed_media_id::text as signed_media_id,
                       td.signed_audit_media_id::text as signed_audit_media_id,
                       td.signed_at
                from signature_request sr
                join transaction_document td on td.id = sr.transaction_document_id
                where sr.id = $1::uuid
                limit 1
                "#,
            )
            .bind(signature_request_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.reconcile.resolve", &error))?;

            let Some(row) = row else {
                let outcome = SignatureCommandOutcome::NotFound;
                let message = "Signature request or transaction document not found.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &command_id,
                    outcome,
                    None,
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &command_id,
                    outcome,
                    None,
                    Some(message),
                    None,
                ));
            };

            if row.signed_media_id.is_some() {
                let value = json!({
                    "replayed": true,
                    "documentId": row.document_id,
                    "mediaId": row.signed_media_id,
                    "auditMediaId": row.signed_audit_media_id,
                    "signedAt": row.signed_at.map(|value| value.to_rfc3339()),
                    "signatureRequestId": signature_request_id,
                });
                let outcome = SignatureCommandOutcome::Success;
                finalize_receipt(
                    &mut tx,
                    &command_id,
                    outcome,
                    Some(&row.document_id),
                    Some("Document already signed; completion treated as replayed."),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &command_id,
                    outcome,
                    Some(row.document_id),
                    Some("Document already signed; completion treated as replayed.".into()),
                    Some(value),
                ));
            }

            if !matches!(row.state.as_str(), "draft" | "ready" | "sent") {
                let outcome = SignatureCommandOutcome::ValidationFailure;
                let message = format!(
                    "Transaction document cannot be signed from state '{}'.",
                    row.state
                );
                finalize_receipt(
                    &mut tx,
                    &command_id,
                    outcome,
                    Some(&row.document_id),
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &command_id,
                    outcome,
                    Some(row.document_id),
                    Some(message),
                    None,
                ));
            }

            let Some(signed_artifact) = signed_artifact else {
                let outcome = SignatureCommandOutcome::Conflict;
                let message =
                    "Signed artifact is required for an unreconciled completed request.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &command_id,
                    outcome,
                    Some(&row.document_id),
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &command_id,
                    outcome,
                    Some(row.document_id),
                    Some(message),
                    None,
                ));
            };

            let signed_media_id = insert_signature_media(tx.connection(), signed_artifact).await?;
            let audit_media_id = match audit_artifact {
                Some(artifact) => Some(insert_signature_media(tx.connection(), artifact).await?),
                None => None,
            };

            if row.state == "draft" {
                let changed = sqlx::query_scalar::<_, String>(
                    r#"
                    update transaction_document
                    set state = 'ready', updated_at = now()
                    where id = $1::uuid and state = 'draft'
                    returning id::text
                    "#,
                )
                .bind(&row.document_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("signature.reconcile.advance_ready", &error)
                })?;
                if changed.is_none() {
                    return Err(DbFailure::schema_mismatch(
                        "signature.reconcile.advance_ready",
                        "transaction document changed concurrently",
                    ));
                }
            }

            if matches!(row.state.as_str(), "draft" | "ready") {
                let changed = sqlx::query_scalar::<_, String>(
                    r#"
                    update transaction_document
                    set state = 'sent', updated_at = now()
                    where id = $1::uuid and state = 'ready'
                    returning id::text
                    "#,
                )
                .bind(&row.document_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| {
                    DbFailure::from_sqlx("signature.reconcile.advance_sent", &error)
                })?;
                if changed.is_none() {
                    return Err(DbFailure::schema_mismatch(
                        "signature.reconcile.advance_sent",
                        "transaction document changed concurrently",
                    ));
                }
            }

            let signed = sqlx::query_as::<_, (String, DateTime<Utc>)>(
                r#"
                update transaction_document
                set state = 'signed',
                    signed_media_id = $2::uuid,
                    signed_audit_media_id = $3::uuid,
                    signed_at = now(),
                    updated_at = now()
                where id = $1::uuid
                  and state = 'sent'
                  and signed_media_id is null
                returning id::text, signed_at
                "#,
            )
            .bind(&row.document_id)
            .bind(&signed_media_id)
            .bind(audit_media_id.as_deref())
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.reconcile.sign", &error))?
            .ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "signature.reconcile.sign",
                    "transaction document changed concurrently before signed transition",
                )
            })?;

            let value = json!({
                "replayed": false,
                "documentId": signed.0,
                "mediaId": signed_media_id,
                "auditMediaId": audit_media_id,
                "signedAt": signed.1.to_rfc3339(),
                "signatureRequestId": signature_request_id,
            });
            let outcome = SignatureCommandOutcome::Success;
            finalize_receipt(
                &mut tx,
                &command_id,
                outcome,
                Some(&signed.0),
                None,
                actor_app_user_id,
            )
            .await?;
            Ok(command_result(
                &command_id,
                outcome,
                Some(signed.0),
                None,
                Some(value),
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

    pub(super) async fn transition(
        &self,
        request: &ApplySignatureStatusRequest,
        target: Option<SignatureRequestStatus>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        let mut tx = self.db.begin("signature.transition").await?;
        let result = async {
            if !claim_receipt(&mut tx, &request.command_id, actor_app_user_id).await? {
                let receipt = read_receipt(&mut tx, &request.command_id).await?;
                return Ok(replay_result(&request.command_id, receipt));
            }

            let current = sqlx::query_as::<_, SignatureRow>(
                r#"
                select id::text as id,
                       transaction_document_id::text as transaction_document_id,
                       status, message, execution_role, execution_slot_id,
                       created_by_user_id::text as created_by_user_id,
                       created_at, updated_at
                from signature_request
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(&request.signature_request_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.transition.read", &error))?;

            let Some(current) = current else {
                let outcome = SignatureCommandOutcome::NotFound;
                let message = "Signature request not found.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &request.command_id,
                    outcome,
                    None,
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &request.command_id,
                    outcome,
                    None,
                    Some(message),
                    None,
                ));
            };

            let from = SignatureRequestStatus::try_from(current.status.as_str())
                .map_err(|error| DbFailure::schema_mismatch("signature.transition", error))?;
            let mapped_current = map_signature(current)?;

            let (signature, transitioned) = match target {
                None => (mapped_current, false),
                Some(target) if target == from => (mapped_current, false),
                Some(target) if !from.can_transition_to(target) => {
                    let outcome = SignatureCommandOutcome::ValidationFailure;
                    let message = format!(
                        "Transition {} -> {} is not allowed.",
                        from.as_str(),
                        target.as_str()
                    );
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        Some(&mapped_current.id),
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        Some(mapped_current.id),
                        Some(message),
                        None,
                    ));
                }
                Some(target) => {
                    let updated = sqlx::query_as::<_, SignatureRow>(
                        r#"
                        update signature_request
                        set status = $2, updated_at = now()
                        where id = $1::uuid and status = $3
                        returning id::text as id,
                                  transaction_document_id::text as transaction_document_id,
                                  status, message, execution_role, execution_slot_id,
                                  created_by_user_id::text as created_by_user_id,
                                  created_at, updated_at
                        "#,
                    )
                    .bind(&request.signature_request_id)
                    .bind(target.as_str())
                    .bind(from.as_str())
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("signature.transition.update", &error))?;

                    let Some(updated) = updated else {
                        let outcome = SignatureCommandOutcome::Conflict;
                        let message = format!(
                            "Signature request {} changed concurrently.",
                            request.signature_request_id
                        );
                        finalize_receipt(
                            &mut tx,
                            &request.command_id,
                            outcome,
                            Some(&request.signature_request_id),
                            Some(&message),
                            actor_app_user_id,
                        )
                        .await?;
                        return Ok(command_result(
                            &request.command_id,
                            outcome,
                            Some(request.signature_request_id.clone()),
                            Some(message),
                            None,
                        ));
                    };
                    (map_signature(updated)?, true)
                }
            };

            let aggregate_id = signature.id.clone();
            let value = serde_json::to_value(SignatureStatusResult {
                signature_request: signature,
                transitioned,
            })
            .map_err(|error| {
                DbFailure::schema_mismatch("signature.transition.serialize", error.to_string())
            })?;
            let outcome = SignatureCommandOutcome::Success;
            finalize_receipt(
                &mut tx,
                &request.command_id,
                outcome,
                Some(&aggregate_id),
                None,
                actor_app_user_id,
            )
            .await?;
            Ok(command_result(
                &request.command_id,
                outcome,
                Some(aggregate_id),
                None,
                Some(value),
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
}

pub(super) async fn insert_signature_media(
    connection: &mut sqlx::PgConnection,
    artifact: &SignatureArtifactDownload,
) -> DbResult<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        insert into media (file_data, filename, mime_type, file_size, media_type)
        values ($1,$2,$3,$4,'document')
        returning id::text
        "#,
    )
    .bind(&artifact.bytes)
    .bind(&artifact.filename)
    .bind(&artifact.mime_type)
    .bind(artifact.bytes.len() as i64)
    .fetch_one(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("signature.reconcile.insert_media", &error))
}
