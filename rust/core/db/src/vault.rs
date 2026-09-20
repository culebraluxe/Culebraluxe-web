use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    ContractIssuedLineage, CreateTransactionDocumentRequest, FormSignerPerson,
    IssuedDocumentEvidence, IssuedDocumentForFormInstance, IssuedDocumentListItem,
    IssueDocumentRequest, NextIssuedVersionRequest, SignedArtifactRef, TransactionDocument,
    TransactionDocumentSource, TransactionDocumentState, TransactionDocumentType,
    TransitionTransactionDocumentRequest, VaultActorScope, VaultArtifactFailure,
    VaultCommandOutcome, VaultCommandResult, VaultMediaBytes, VaultRenderRequest,
    VaultRenderedArtifact,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection};
use std::collections::BTreeMap;
use std::future::Future;

const LISTING_TEMPLATE_ID: &str = "LISTING-01";
const SELLER_BROKER_NAME: &str = "Lisa Penfield";

#[derive(Debug, FromRow)]
struct DocumentRow {
    id: String,
    deal_id: Option<String>,
    document_type: String,
    document_type_label: Option<String>,
    title: Option<String>,
    state: String,
    source: String,
    source_system: Option<String>,
    source_external_id: Option<String>,
    prepared_by_user_id: Option<String>,
    party_person_id: Option<String>,
    media_id: Option<String>,
    signed_media_id: Option<String>,
    signed_audit_media_id: Option<String>,
    signed_at: Option<DateTime<Utc>>,
    supersedes_document_id: Option<String>,
    issued_checksum_sha256: Option<String>,
    template_id: Option<String>,
    template_version: Option<i32>,
    source_snapshot: Option<Value>,
    issued_version: Option<i32>,
    form_instance_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct IssuedListRow {
    id: String,
    deal_id: Option<String>,
    property_id: Option<String>,
    document_type_label: Option<String>,
    title: Option<String>,
    state: String,
    template_id: Option<String>,
    template_version: Option<i32>,
    issued_version: Option<i32>,
    issued_checksum_sha256: Option<String>,
    issued_by_display_name: Option<String>,
    party_name: Option<String>,
    property_name: Option<String>,
    deal_name: Option<String>,
    created_at: DateTime<Utc>,
    signed_media_id: Option<String>,
    signed_audit_media_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct IssuedFormRow {
    document_id: String,
    issued_version: i32,
    checksum: String,
    created_at: DateTime<Utc>,
    media_id: Option<String>,
    source_snapshot: Option<Value>,
}

#[derive(Debug, FromRow)]
struct MediaRow {
    file_data: Option<Vec<u8>>,
    filename: String,
    mime_type: String,
}

#[derive(Debug, FromRow)]
struct ReceiptRow {
    outcome: String,
    aggregate_id: Option<String>,
    message: Option<String>,
}

#[derive(Debug, FromRow)]
struct IssueFormRow {
    id: String,
    template_id: String,
    template_version: i32,
    deal_id: Option<String>,
    person_id: Option<String>,
    contract_id: Option<String>,
    field_values: Value,
    sections: Value,
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

fn map_document(row: DocumentRow) -> DbResult<TransactionDocument> {
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
                DbFailure::schema_mismatch("vault.map_document", "issued document missing template_id")
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

fn role_for_form(template_id: &str, role: &str) -> String {
    if template_id == LISTING_TEMPLATE_ID
        && matches!(role, "owner" | "seller" | "SELLER_BROKER")
    {
        return "SELLER".into();
    }
    match role {
        "client" => "BUYER".into(),
        "seller" | "owner" => "SELLER".into(),
        "" => "OTHER".into(),
        other => other.to_owned(),
    }
}

fn outcome_result(
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

async fn claim_receipt(
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

async fn read_receipt(tx: &mut DbTransaction, command_id: &str) -> DbResult<Option<ReceiptRow>> {
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

async fn finalize_receipt(
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

fn replay_from_receipt(command_id: &str, receipt: Option<ReceiptRow>) -> VaultCommandResult {
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

async fn list_signers_on(
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
        });
    }

    Ok(people)
}

#[derive(Clone)]
pub struct VaultDao {
    db: Database,
}

impl VaultDao {
    pub fn new(db: Database) -> Self {
        Self { db }
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
                   pr.id::text as property_id,
                   td.document_type_label,
                   td.title,
                   td.state,
                   td.template_id,
                   td.template_version,
                   td.issued_version,
                   td.issued_checksum_sha256,
                   u.display_name as issued_by_display_name,
                   p.display_name as party_name,
                   pr.name as property_name,
                   null::text as deal_name,
                   td.created_at,
                   td.signed_media_id::text as signed_media_id,
                   td.signed_audit_media_id::text as signed_audit_media_id
            from transaction_document td
            left join app_user u on u.id = td.prepared_by_user_id
            left join person p on p.id = td.party_person_id
            left join deal d on d.id = td.deal_id
            left join property pr on pr.id = d.property_id
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
        .fetch_all(self.db.pool())
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
        .fetch_optional(self.db.pool())
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
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.list_by_deal", &error))?;
        rows.into_iter().map(map_document).collect()
    }

    pub async fn create_document(
        &self,
        request: &CreateTransactionDocumentRequest,
    ) -> DbResult<TransactionDocument> {
        let signed_media_id = request.signed_artifact.as_ref().map(|signed| signed.media_id.as_str());
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
        .fetch_optional(self.db.pool())
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
        .fetch_optional(self.db.pool())
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
                    let from = TransactionDocumentState::try_from(current.as_str()).map_err(
                        |error| DbFailure::schema_mismatch("vault.transition_state", error),
                    )?;
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
        .fetch_optional(self.db.pool())
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

    pub async fn next_issued_version(
        &self,
        request: &NextIssuedVersionRequest,
    ) -> DbResult<i32> {
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
            .fetch_optional(self.db.pool())
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
            .fetch_optional(self.db.pool())
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
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(media_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("vault.media_bytes", &error))?;
        Ok(row.map(|row| VaultMediaBytes {
            bytes: row.file_data.unwrap_or_default(),
            filename: row.filename,
            mime_type: row.mime_type,
        }))
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
        .fetch_optional(self.db.pool())
        .await
        .map(|value| value.flatten())
        .map_err(|error| DbFailure::from_sqlx("vault.form_contract_id", &error))
    }

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
        .fetch_optional(self.db.pool())
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
        .fetch_optional(self.db.pool())
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

            let artifact = match render(VaultRenderRequest {
                form_instance_id: form.id.clone(),
                contract_id: form.contract_id.clone(),
                template_id: form.template_id.clone(),
                template_version: form.template_version,
                field_values: string_map(form.field_values.clone()),
                sections: string_map(form.sections.clone()),
                issued_version,
                participants: participants.clone(),
                actor_app_user_id: request.actor_app_user_id.clone(),
                issued_at: request.issued_at.clone(),
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
