//! Moved from `vault.rs` (move only): LISTING_TEMPLATE_ID, SELLER_BROKER_NAME, DocumentRow, IssuedListRow, IssuedFormRow, MediaRow, ReceiptRow, IssueFormRow, SignerFormRow, SignerRow, BrokerRow, compact, string_map, map_document, role_for_form, outcome_result, claim_receipt, read_receipt, finalize_receipt, replay_from_receipt, list_signers_on, VaultDao.

#[allow(unused_imports)]
use super::*;

pub(super) const LISTING_TEMPLATE_ID: &str = "LISTING-01";
pub(super) const SELLER_BROKER_NAME: &str = "Lisa Penfield";

#[derive(Debug, FromRow)]
pub(super) struct DocumentRow {
    pub(super) id: String,
    pub(super) deal_id: Option<String>,
    pub(super) document_type: String,
    pub(super) document_type_label: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: String,
    pub(super) source: String,
    pub(super) source_system: Option<String>,
    pub(super) source_external_id: Option<String>,
    pub(super) prepared_by_user_id: Option<String>,
    pub(super) party_person_id: Option<String>,
    pub(super) media_id: Option<String>,
    pub(super) signed_media_id: Option<String>,
    pub(super) signed_audit_media_id: Option<String>,
    pub(super) signed_at: Option<DateTime<Utc>>,
    pub(super) supersedes_document_id: Option<String>,
    pub(super) issued_checksum_sha256: Option<String>,
    pub(super) template_id: Option<String>,
    pub(super) template_version: Option<i32>,
    pub(super) source_snapshot: Option<Value>,
    pub(super) issued_version: Option<i32>,
    pub(super) form_instance_id: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct IssuedListRow {
    pub(super) id: String,
    pub(super) deal_id: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) document_type_label: Option<String>,
    pub(super) title: Option<String>,
    pub(super) state: String,
    pub(super) template_id: Option<String>,
    pub(super) template_version: Option<i32>,
    pub(super) issued_version: Option<i32>,
    pub(super) issued_checksum_sha256: Option<String>,
    pub(super) issued_by_display_name: Option<String>,
    pub(super) party_name: Option<String>,
    pub(super) property_name: Option<String>,
    pub(super) deal_name: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) signed_media_id: Option<String>,
    pub(super) signed_audit_media_id: Option<String>,
    pub(super) party_person_id: Option<String>,
    pub(super) signed_at: Option<DateTime<Utc>>,
    pub(super) form_instance_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct IssuedFormRow {
    pub(super) document_id: String,
    pub(super) issued_version: i32,
    pub(super) checksum: String,
    pub(super) created_at: DateTime<Utc>,
    pub(super) media_id: Option<String>,
    pub(super) source_snapshot: Option<Value>,
}

#[derive(Debug, FromRow)]
pub(super) struct MediaRow {
    pub(super) file_data: Option<Vec<u8>>,
    pub(super) filename: String,
    pub(super) mime_type: String,
}

#[derive(Debug, FromRow)]
pub(super) struct ReceiptRow {
    pub(super) outcome: String,
    pub(super) aggregate_id: Option<String>,
    pub(super) message: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct IssueFormRow {
    pub(super) id: String,
    pub(super) template_id: String,
    pub(super) template_version: i32,
    pub(super) deal_id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) contract_id: Option<String>,
    pub(super) field_values: Value,
    pub(super) sections: Value,
}

#[derive(Debug, FromRow)]
pub(super) struct SignerFormRow {
    pub(super) deal_id: Option<String>,
    pub(super) template_id: String,
    pub(super) status: String,
    pub(super) person_name: Option<String>,
    pub(super) resolved_person_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct SignerRow {
    pub(super) person_id: Option<String>,
    pub(super) display_name: String,
    pub(super) email: Option<String>,
    pub(super) role: String,
}

#[derive(Debug, FromRow)]
pub(super) struct BrokerRow {
    pub(super) person_id: Option<String>,
    pub(super) email: Option<String>,
}

pub(super) fn compact(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

pub(super) fn string_map(value: Value) -> BTreeMap<String, String> {
    serde_json::from_value(value).unwrap_or_default()
}

pub(super) fn map_document(row: DocumentRow) -> DbResult<TransactionDocument> {
    let document_type = TransactionDocumentType::try_from(row.document_type.as_str())
        .map_err(|error| DbFailure::schema_mismatch("vault.map_document", error))?;
    let state = TransactionDocumentState::try_from(row.state.as_str())
        .map_err(|error| DbFailure::schema_mismatch("vault.map_document", error))?;
    let source = TransactionDocumentSource::try_from(row.source.as_str())
        .map_err(|error| DbFailure::schema_mismatch("vault.map_document", error))?;

    let signed_artifact = match (row.signed_media_id, row.signed_at) {
        (Some(media_id), Some(signed_at)) => Some(SignedArtifactRef {
            media_id,
            signed_at: signed_at.to_rfc3339(),
        }),
        (None, None) => None,
        _ => {
            return Err(DbFailure::schema_mismatch(
                "vault.map_document",
                "signed_media_id and signed_at must be set together",
            ))
        }
    };

    let issued_evidence = match row.issued_checksum_sha256 {
        None => None,
        Some(checksum_sha256) => Some(IssuedDocumentEvidence {
            checksum_sha256,
            template_id: row.template_id.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "vault.map_document",
                    "issued document missing template_id",
                )
            })?,
            template_version: row.template_version.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "vault.map_document",
                    "issued document missing template_version",
                )
            })?,
            source_snapshot: row.source_snapshot.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "vault.map_document",
                    "issued document missing source_snapshot",
                )
            })?,
            issued_version: row.issued_version.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "vault.map_document",
                    "issued document missing issued_version",
                )
            })?,
            form_instance_id: row.form_instance_id.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "vault.map_document",
                    "issued document missing form_instance_id",
                )
            })?,
        }),
    };

    Ok(TransactionDocument {
        id: row.id,
        deal_id: row.deal_id,
        document_type,
        document_type_label: compact(row.document_type_label),
        title: compact(row.title),
        state,
        source,
        source_system: compact(row.source_system),
        source_external_id: compact(row.source_external_id),
        prepared_by_user_id: row.prepared_by_user_id,
        party_person_id: row.party_person_id,
        media_id: row.media_id,
        signed_artifact,
        signed_audit_media_id: row.signed_audit_media_id,
        supersedes_document_id: row.supersedes_document_id,
        issued_evidence,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

pub(super) fn role_for_form(template_id: &str, role: &str) -> String {
    if template_id == LISTING_TEMPLATE_ID && matches!(role, "owner" | "seller" | "SELLER_BROKER") {
        return "SELLER".into();
    }
    match role {
        "client" => "BUYER".into(),
        "seller" | "owner" => "SELLER".into(),
        "" => "OTHER".into(),
        other => other.to_owned(),
    }
}

pub(super) fn outcome_result(
    command_id: &str,
    outcome: VaultCommandOutcome,
    aggregate_id: Option<String>,
    message: Option<String>,
    replayed: bool,
) -> VaultCommandResult {
    VaultCommandResult {
        command_id: command_id.to_owned(),
        outcome,
        aggregate_id,
        message,
        replayed,
        value: None,
    }
}

pub(super) async fn claim_receipt(
    tx: &mut DbTransaction,
    command_id: &str,
    actor_app_user_id: Option<&str>,
) -> DbResult<bool> {
    let claimed = sqlx::query_scalar::<_, String>(
        r#"
        insert into workflow_command_receipt (
            command_id, outcome, aggregate_id, message, actor_app_user_id
        )
        values ($1, 'pending', null, null, $2::uuid)
        on conflict (command_id) do nothing
        returning command_id
        "#,
    )
    .bind(command_id)
    .bind(actor_app_user_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("vault.receipt.claim", &error))?;
    Ok(claimed.is_some())
}

pub(super) async fn read_receipt(tx: &mut DbTransaction, command_id: &str) -> DbResult<Option<ReceiptRow>> {
    sqlx::query_as::<_, ReceiptRow>(
        r#"
        select outcome, aggregate_id::text as aggregate_id, message
        from workflow_command_receipt
        where command_id = $1
        limit 1
        "#,
    )
    .bind(command_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("vault.receipt.read", &error))
}

pub(super) async fn finalize_receipt(
    tx: &mut DbTransaction,
    command_id: &str,
    outcome: &VaultCommandOutcome,
    aggregate_id: Option<&str>,
    message: Option<&str>,
    actor_app_user_id: Option<&str>,
) -> DbResult<()> {
    sqlx::query(
        r#"
        update workflow_command_receipt
        set outcome = $2,
            aggregate_id = $3::uuid,
            message = $4,
            actor_app_user_id = $5::uuid
        where command_id = $1
        "#,
    )
    .bind(command_id)
    .bind(outcome.as_str())
    .bind(aggregate_id)
    .bind(message)
    .bind(actor_app_user_id)
    .execute(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("vault.receipt.finalize", &error))?;
    Ok(())
}

pub(super) fn replay_from_receipt(command_id: &str, receipt: Option<ReceiptRow>) -> VaultCommandResult {
    let Some(receipt) = receipt else {
        return outcome_result(
            command_id,
            VaultCommandOutcome::Conflict,
            None,
            Some("Command has no receipt; treat as in-flight.".into()),
            true,
        );
    };
    if receipt.outcome == "pending" {
        return outcome_result(
            command_id,
            VaultCommandOutcome::Conflict,
            receipt.aggregate_id,
            Some("Command claim is in-flight (pending receipt); retry later.".into()),
            true,
        );
    }
    let outcome = VaultCommandOutcome::try_from(receipt.outcome.as_str())
        .unwrap_or(VaultCommandOutcome::Conflict);
    outcome_result(
        command_id,
        outcome,
        receipt.aggregate_id,
        receipt.message,
        true,
    )
}

pub(super) async fn list_signers_on(
    connection: &mut PgConnection,
    form_instance_id: &str,
) -> DbResult<Vec<FormSignerPerson>> {
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
    .fetch_optional(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.form", &error))?;

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
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.direct_email", &error))?;
        people.push(FormSignerPerson {
            person_id: Some(person_id),
            name: form.person_name.unwrap_or_default(),
            email,
            role: if listing_direct_draft {
                "SELLER".into()
            } else {
                "CLIENT".into()
            },
            slot_id: None,
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
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.deal_client", &error))?
        {
            people.push(FormSignerPerson {
                person_id: client.person_id,
                name: client.display_name,
                email: compact(client.email),
                role: client.role,
                slot_id: None,
            });
        }

        let form_participants = sqlx::query_as::<_, SignerRow>(
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
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.form_participants", &error))?;
        for row in form_participants {
            people.push(FormSignerPerson {
                person_id: row.person_id,
                name: row.display_name,
                email: compact(row.email),
                role: role_for_form(&form.template_id, &row.role),
                slot_id: None,
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
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.deal_participants", &error))?;
        for row in deal_people {
            people.push(FormSignerPerson {
                person_id: row.person_id,
                name: row.display_name,
                email: compact(row.email),
                role: role_for_form(&form.template_id, &row.role),
                slot_id: None,
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
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.issue.signers.broker", &error))?;
        if brokers.len() != 1 {
            return Err(DbFailure::schema_mismatch(
                "vault.issue.signers.broker",
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
            slot_id: None,
        });
    }

    Ok(people)
}

#[derive(Clone)]
pub struct VaultDao {
    pub(super) db: Database,
}
