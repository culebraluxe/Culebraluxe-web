//! Moved from `signature.rs` (move only): new.

#[allow(unused_imports)]
use super::*;

impl SignatureDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn get(&self, id: &str) -> DbResult<Option<SignatureRequest>> {
        let row = sqlx::query_as::<_, SignatureRow>(
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
        .bind(id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.get", &error))?;
        row.map(map_signature).transpose()
    }

    pub async fn active_for_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        let row = sqlx::query_as::<_, SignatureRow>(
            r#"
            select id::text as id,
                   transaction_document_id::text as transaction_document_id,
                   status, message, execution_role, execution_slot_id,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from signature_request
            where transaction_document_id = $1::uuid
              and status in ('requested', 'sent', 'viewed', 'signed')
            order by created_at asc, id
            limit 1
            "#,
        )
        .bind(transaction_document_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.active_for_document", &error))?;
        row.map(map_signature).transpose()
    }

    pub async fn list_by_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>> {
        let rows = sqlx::query_as::<_, SignatureRow>(
            r#"
            select id::text as id,
                   transaction_document_id::text as transaction_document_id,
                   status, message, execution_role, execution_slot_id,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from signature_request
            where transaction_document_id = $1::uuid
            order by created_at asc, id
            "#,
        )
        .bind(transaction_document_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.list_by_document", &error))?;
        rows.into_iter().map(map_signature).collect()
    }

    pub async fn send(
        &self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        let mut tx = self.db.begin("signature.send").await?;
        let result = async {
            if !claim_receipt(&mut tx, &request.command_id, actor_app_user_id).await? {
                let receipt = read_receipt(&mut tx, &request.command_id).await?;
                return Ok(replay_result(&request.command_id, receipt));
            }

            let requires_snapshot = request.execution_slot_id.is_some()
                || request
                    .recipients
                    .iter()
                    .any(|recipient| recipient.execution_slot_id.is_some());

            let document = sqlx::query_as::<_, DocumentSnapshotRow>(
                r#"
                select source_snapshot
                from transaction_document
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(&request.transaction_document_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.send.document", &error))?;

            let Some(document) = document else {
                let outcome = SignatureCommandOutcome::NotFound;
                let message = "Transaction document not found.".to_owned();
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

            let mut execution_role = request.execution_role.clone();
            if requires_snapshot {
                let slots = match parse_slots(document.source_snapshot.as_ref()) {
                    Ok(slots) => slots,
                    Err(error) => {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!("Invalid issued-participant snapshot: {error}");
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
                    }
                };

                if let Some(slot_id) = request.execution_slot_id.as_deref() {
                    let Some(slot) = slots.iter().find(|slot| slot.slot_id == slot_id) else {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!(
                            "Execution slot '{slot_id}' does not exist in document {}.",
                            request.transaction_document_id
                        );
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

                    if request
                        .execution_role
                        .as_deref()
                        .is_some_and(|role| role != slot.role)
                    {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!(
                            "Execution role '{}' does not match slot '{slot_id}'.",
                            request.execution_role.as_deref().unwrap_or_default()
                        );
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
                    }

                    if let Some(expected) = request.slot_recipient_email.as_deref() {
                        if slot.email.as_deref().is_none_or(|email| {
                            normalize_signature_email(email)
                                != normalize_signature_email(expected)
                        }) {
                            let outcome = SignatureCommandOutcome::ValidationFailure;
                            let message =
                                "Recipient does not match the immutable execution slot.".to_owned();
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
                        }
                    }
                    execution_role = Some(slot.role.clone());
                }

                if let Err(message) = validate_bound_recipients(&slots, &request.recipients) {
                    let outcome = SignatureCommandOutcome::ValidationFailure;
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
                }
            }

            let inserted = sqlx::query_as::<_, SignatureRow>(
                r#"
                insert into signature_request (
                    transaction_document_id, status, message, created_by_user_id,
                    execution_role, execution_slot_id
                )
                values ($1::uuid,'requested',$2,$3::uuid,$4,$5)
                on conflict (transaction_document_id)
                  where status in ('requested', 'sent', 'viewed', 'signed')
                  do nothing
                returning id::text as id,
                          transaction_document_id::text as transaction_document_id,
                          status, message, execution_role, execution_slot_id,
                          created_by_user_id::text as created_by_user_id,
                          created_at, updated_at
                "#,
            )
            .bind(&request.transaction_document_id)
            .bind(request.message.as_deref())
            .bind(request.created_by_user_id.as_deref())
            .bind(execution_role.as_deref())
            .bind(request.execution_slot_id.as_deref())
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.send.insert", &error))?;

            let (signature, existing) = if let Some(row) = inserted {
                for recipient in &request.recipients {
                    sqlx::query(
                        r#"
                        insert into signature_envelope_recipient (
                            signature_request_id, execution_role, execution_slot_id,
                            recipient_name, recipient_email, signer_order
                        )
                        values ($1::uuid,$2,$3,$4,$5,$6)
                        "#,
                    )
                    .bind(&row.id)
                    .bind(recipient.execution_role.as_deref())
                    .bind(recipient.execution_slot_id.as_deref())
                    .bind(recipient.name.trim())
                    .bind(recipient.email.trim())
                    .bind(recipient.order)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("signature.send.insert_recipient", &error)
                    })?;
                }
                (map_signature(row)?, false)
            } else {
                let row = sqlx::query_as::<_, SignatureRow>(
                    r#"
                    select id::text as id,
                           transaction_document_id::text as transaction_document_id,
                           status, message, execution_role, execution_slot_id,
                           created_by_user_id::text as created_by_user_id,
                           created_at, updated_at
                    from signature_request
                    where transaction_document_id = $1::uuid
                      and status in ('requested', 'sent', 'viewed', 'signed')
                    order by created_at asc, id
                    limit 1
                    "#,
                )
                .bind(&request.transaction_document_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("signature.send.active", &error))?;

                let Some(row) = row else {
                    let outcome = SignatureCommandOutcome::Conflict;
                    let message =
                        "Could not record the signature request (active-request conflict)."
                            .to_owned();
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
                if row.execution_slot_id.as_deref() != request.execution_slot_id.as_deref() {
                    let outcome = SignatureCommandOutcome::Conflict;
                    let message = "Another signature request is active for this document; complete or void it before sending a different execution slot.".to_owned();
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        Some(&row.id),
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        Some(row.id),
                        Some(message),
                        None,
                    ));
                }
                (map_signature(row)?, true)
            };

            let aggregate_id = signature.id.clone();
            let value = serde_json::to_value(SignatureRequestResult {
                signature_request: signature,
                existing,
            })
            .map_err(|error| {
                DbFailure::schema_mismatch("signature.send.serialize", error.to_string())
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

    pub async fn apply_status(
        &self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(request, request.target_status, actor_app_user_id)
            .await
    }

    pub async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(
            &ApplySignatureStatusRequest {
                command_id: command_id.into(),
                signature_request_id: signature_request_id.into(),
                target_status: Some(SignatureRequestStatus::Voided),
            },
            Some(SignatureRequestStatus::Voided),
            actor_app_user_id,
        )
        .await
    }

    pub async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(
            &ApplySignatureStatusRequest {
                command_id: command_id.into(),
                signature_request_id: signature_request_id.into(),
                target_status: Some(SignatureRequestStatus::Declined),
            },
            Some(SignatureRequestStatus::Declined),
            actor_app_user_id,
        )
        .await
    }

    pub async fn reconciliation_needs_artifact(
        &self,
        event_id: &str,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        let command_id = format!("signature.reconcile:{event_id}");
        let receipt = sqlx::query_as::<_, ReceiptRow>(
            r#"
            select outcome, aggregate_id::text as aggregate_id, message
            from workflow_command_receipt
            where command_id = $1
            limit 1
            "#,
        )
        .bind(&command_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.reconcile.probe_receipt", &error))?;

        if receipt.as_ref().is_some_and(|row| row.outcome != "pending") {
            return Ok(false);
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
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.reconcile.probe_document", &error))?;

        Ok(row.is_some_and(|row| {
            row.signed_media_id.is_none()
                && matches!(row.state.as_str(), "draft" | "ready" | "sent")
        }))
    }

}
