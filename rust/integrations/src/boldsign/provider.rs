use super::client::{
    BoldSignClient, BoldSignDirectSigner, BoldSignFormField, BoldSignSendDocument,
};
use super::store::BoldSignStore;
use super::{
    map_status, map_webhook_event, parse_webhook_payload, verify_webhook_signature, BoldSignConfig,
    BOLD_SIGN_PROVIDER,
};
use async_trait::async_trait;
use db::{Database, DbFailure};
use domain::{
    SignatureArtifactDownload, SignatureProviderActionResult, SignatureProviderEvent,
    SignatureProviderSendRequest, SignatureProviderSendResult, SignatureProviderStatusResult,
    SignatureRecipientRole, SignatureRequestStatus, SignatureWebhookVerification,
};
use serde_json::Value;
use service::SignatureProvider;
use sqlx::FromRow;
use std::collections::BTreeSet;

const INITIAL_ENVELOPE_STATUS: &str = "InProgress";
const PDF_SIGNATURE_COORDINATE_SPACE: &str = "pdf-points-bottom-left";

#[derive(Debug, Clone)]
struct SignatureAnchor {
    role: String,
    slot_id: Option<String>,
    kind: AnchorKind,
    page_index: i32,
    page_height: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorKind {
    Signature,
    Initial,
    Date,
}

#[derive(Debug, FromRow)]
struct DocumentPdfRow {
    file_data: Option<Vec<u8>>,
    filename: String,
    mime_type: String,
    source_snapshot: Option<Value>,
}

#[derive(Clone)]
pub struct BoldSignSignatureProvider {
    config: BoldSignConfig,
    client: BoldSignClient,
    store: BoldSignStore,
}

impl BoldSignSignatureProvider {
    pub fn new(db: Database, config: BoldSignConfig) -> Result<Self, String> {
        let client = BoldSignClient::new(config.clone()).map_err(|error| error.message)?;
        Ok(Self {
            config,
            client,
            store: BoldSignStore::new(db),
        })
    }

    async fn load_document_pdf(
        &self,
        transaction_document_id: &str,
    ) -> Result<(Vec<u8>, String, String, Vec<SignatureAnchor>), String> {
        let database = self.store.database();
        let row = sqlx::query_as::<_, DocumentPdfRow>(
            r#"
            select m.file_data,
                   m.filename,
                   m.mime_type,
                   d.source_snapshot
            from transaction_document d
            join media m on m.id = d.media_id
            where d.id = $1::uuid
            limit 1
            "#,
        )
        .bind(transaction_document_id)
        .fetch_optional(database.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.provider.load_pdf", &error).to_string())?
        .ok_or_else(|| {
            format!(
                "Transaction document {transaction_document_id} has no unsigned PDF media to send."
            )
        })?;

        let bytes = row.file_data.ok_or_else(|| {
            format!(
                "Transaction document {transaction_document_id} has no unsigned PDF media to send."
            )
        })?;
        if bytes.is_empty() {
            return Err(format!(
                "Transaction document {transaction_document_id} has an empty unsigned PDF."
            ));
        }

        let anchors = parse_signature_anchors(
            row.source_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.get("signatureAnchors")),
        );
        if anchors.is_empty() {
            return Err(
                "The issued PDF has no immutable signature anchors. Reissue the document before sending it for signature."
                    .into(),
            );
        }

        Ok((bytes, row.filename, row.mime_type, anchors))
    }
}

#[async_trait]
impl SignatureProvider for BoldSignSignatureProvider {
    fn name(&self) -> &'static str {
        BOLD_SIGN_PROVIDER
    }

    fn map_status(&self, provider_status: &str) -> SignatureRequestStatus {
        map_status(provider_status)
    }

    async fn send(
        &self,
        request: SignatureProviderSendRequest,
    ) -> Result<SignatureProviderSendResult, String> {
        let existing = self
            .store
            .get_by_signature_request(&request.signature_request_id)
            .await
            .map_err(|error| error.to_string())?;

        if let Some(row) = existing.as_ref().filter(|row| row.envelope_id.is_some()) {
            return Ok(SignatureProviderSendResult {
                ok: true,
                provider_status: row.status.clone(),
                error: None,
            });
        }

        if existing
            .as_ref()
            .is_some_and(|row| row.error_retryable == Some(true))
        {
            return Ok(SignatureProviderSendResult {
                ok: false,
                provider_status: INITIAL_ENVELOPE_STATUS.into(),
                error: existing.and_then(|row| row.last_error).or_else(|| {
                    Some(
                        "BoldSign send outcome is uncertain; do not resend until provider state is resolved."
                            .into(),
                    )
                }),
            });
        }

        let send_result = async {
            let (file_bytes, filename, mime_type, anchors) = self
                .load_document_pdf(&request.transaction_document_id)
                .await
                .map_err(|message| (message, false))?;
            let signer_anchors = resolve_recipient_anchor_sets(&anchors, &request)?;
            let signers = request
                .recipients
                .iter()
                .zip(signer_anchors)
                .map(|(recipient, anchors)| BoldSignDirectSigner {
                    name: recipient.name.clone(),
                    email_address: recipient.email.clone(),
                    signer_type: match recipient.role {
                        SignatureRecipientRole::Approver => "Reviewer".into(),
                        SignatureRecipientRole::Signer => "Signer".into(),
                    },
                    signer_order: recipient.order,
                    form_fields: anchors.into_iter().map(anchor_field).collect(),
                })
                .collect::<Vec<_>>();

            let envelope_id = self
                .client
                .send_document(&BoldSignSendDocument {
                    file_bytes,
                    filename,
                    mime_type,
                    title: Some("Signature request".into()),
                    message: request.message.clone(),
                    signers,
                    completion_cc_emails: request.completion_recipient_emails.clone(),
                })
                .await
                .map_err(|error| (error.message, error.retryable))?;

            let mut row = self
                .store
                .create(
                    &request.signature_request_id,
                    &envelope_id,
                    INITIAL_ENVELOPE_STATUS,
                )
                .await
                .map_err(|error| (error.to_string(), false))?;
            if row.is_none() {
                row = self
                    .store
                    .get_by_signature_request(&request.signature_request_id)
                    .await
                    .map_err(|error| (error.to_string(), false))?;
            }
            let row = row.filter(|row| row.envelope_id.is_some()).ok_or_else(|| {
                (
                    "BoldSign send did not produce a durable provider envelope.".into(),
                    false,
                )
            })?;

            Ok::<_, (String, bool)>(SignatureProviderSendResult {
                ok: true,
                provider_status: row.status,
                error: None,
            })
        }
        .await;

        match send_result {
            Ok(result) => Ok(result),
            Err((message, retryable)) => {
                let status = if retryable {
                    INITIAL_ENVELOPE_STATUS
                } else {
                    "error"
                };
                self.store
                    .record_error(&request.signature_request_id, &message, retryable, status)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(SignatureProviderSendResult {
                    ok: false,
                    provider_status: status.into(),
                    error: Some(message),
                })
            }
        }
    }

    async fn status(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderStatusResult, String> {
        let row = self
            .store
            .get_by_signature_request(signature_request_id)
            .await
            .map_err(|error| error.to_string())?;

        let Some(row) = row else {
            return Ok(SignatureProviderStatusResult {
                status: SignatureRequestStatus::Error,
            });
        };
        let Some(envelope_id) = row.envelope_id.as_deref() else {
            return Ok(SignatureProviderStatusResult {
                status: if row.error_retryable == Some(true) {
                    SignatureRequestStatus::Sent
                } else {
                    SignatureRequestStatus::Error
                },
            });
        };

        match self.client.get_document_properties(envelope_id).await {
            Ok(properties) => {
                self.store
                    .update_status(
                        signature_request_id,
                        &properties.status,
                        &properties.file_ids,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(SignatureProviderStatusResult {
                    status: map_status(&properties.status),
                })
            }
            Err(error) => {
                let status = if error.retryable {
                    row.status.as_str()
                } else {
                    "error"
                };
                self.store
                    .record_error(
                        signature_request_id,
                        &error.message,
                        error.retryable,
                        status,
                    )
                    .await
                    .map_err(|failure| failure.to_string())?;
                Ok(SignatureProviderStatusResult {
                    status: if error.retryable {
                        map_status(&row.status)
                    } else {
                        SignatureRequestStatus::Error
                    },
                })
            }
        }
    }

    async fn cancel(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderActionResult, String> {
        let row = self
            .store
            .get_by_signature_request(signature_request_id)
            .await
            .map_err(|error| error.to_string())?;

        let Some(row) = row else {
            return Ok(SignatureProviderActionResult {
                ok: false,
                error: Some("No BoldSign envelope exists for this request.".into()),
            });
        };
        let Some(envelope_id) = row.envelope_id.as_deref() else {
            return Ok(SignatureProviderActionResult {
                ok: false,
                error: Some(if row.error_retryable == Some(true) {
                    "BoldSign send outcome is uncertain; provider envelope id is unknown and automatic cancellation is unsafe."
                        .into()
                } else {
                    "No BoldSign envelope exists for this request.".into()
                }),
            });
        };

        match self
            .client
            .revoke_document(envelope_id, "Signature request cancelled.")
            .await
        {
            Ok(()) => {
                self.store
                    .update_status(signature_request_id, "Revoked", &row.document_ids)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(SignatureProviderActionResult {
                    ok: true,
                    error: None,
                })
            }
            Err(error) => {
                self.store
                    .record_error(
                        signature_request_id,
                        &error.message,
                        error.retryable,
                        &row.status,
                    )
                    .await
                    .map_err(|failure| failure.to_string())?;
                Ok(SignatureProviderActionResult {
                    ok: false,
                    error: Some(error.message),
                })
            }
        }
    }

    async fn verify_webhook(
        &self,
        raw_payload: &str,
        signature: &str,
    ) -> Result<SignatureWebhookVerification, String> {
        let now_seconds = chrono::Utc::now().timestamp();
        verify_webhook_signature(
            raw_payload,
            signature,
            &self.config.webhook_secret,
            now_seconds,
            self.config.webhook_tolerance_seconds,
        )?;

        let event = parse_webhook_payload(raw_payload)?;
        let row = self
            .store
            .get_by_envelope(&event.envelope_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                format!(
                    "BoldSign webhook for unknown envelope {}.",
                    event.envelope_id
                )
            })?;

        let neutral = map_webhook_event(&event.event_type, event.document_status.as_deref())?;
        let payload: Value = serde_json::from_str(raw_payload)
            .map_err(|_| "BoldSign webhook payload is not valid JSON.".to_owned())?;
        self.store
            .record_webhook(
                &event.provider_event_id,
                &event.envelope_id,
                &row.signature_request_id,
                &event.event_type,
                neutral_event_name(neutral),
                &payload,
            )
            .await
            .map_err(|error| error.to_string())?;

        Ok(SignatureWebhookVerification {
            event: neutral,
            signature_request_id: row.signature_request_id,
        })
    }

    async fn download_signed_artifact(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureArtifactDownload, String> {
        let envelope_id = self.envelope_id(signature_request_id).await?;
        let download = self
            .client
            .download_document(&envelope_id)
            .await
            .map_err(|error| error.message)?;
        Ok(SignatureArtifactDownload {
            bytes: download.bytes,
            filename: download.filename,
            mime_type: download.mime_type,
        })
    }

    async fn download_audit_trail(
        &self,
        signature_request_id: &str,
    ) -> Result<Option<SignatureArtifactDownload>, String> {
        let envelope_id = self.envelope_id(signature_request_id).await?;
        let download = self
            .client
            .download_audit_trail(&envelope_id)
            .await
            .map_err(|error| error.message)?;
        Ok(Some(SignatureArtifactDownload {
            bytes: download.bytes,
            filename: download.filename,
            mime_type: download.mime_type,
        }))
    }
}

impl BoldSignSignatureProvider {
    async fn envelope_id(&self, signature_request_id: &str) -> Result<String, String> {
        self.store
            .get_by_signature_request(signature_request_id)
            .await
            .map_err(|error| error.to_string())?
            .and_then(|row| row.envelope_id)
            .ok_or_else(|| {
                format!(
                    "BoldSign: no envelope exists for signature request {signature_request_id}."
                )
            })
    }
}

fn parse_signature_anchors(value: Option<&Value>) -> Vec<SignatureAnchor> {
    let Some(entries) = value.and_then(Value::as_array) else {
        return vec![];
    };

    entries
        .iter()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            if object.get("coordinateSpace")?.as_str()? != PDF_SIGNATURE_COORDINATE_SPACE {
                return None;
            }
            let role = object.get("role")?.as_str()?.trim();
            if role.is_empty() {
                return None;
            }
            let kind = match object.get("kind")?.as_str()? {
                "signature" => AnchorKind::Signature,
                "initial" => AnchorKind::Initial,
                "date" => AnchorKind::Date,
                _ => return None,
            };
            let page_index = object.get("pageIndex")?.as_i64()?;
            if page_index < 0 || page_index > i32::MAX as i64 {
                return None;
            }
            let page_height = object.get("pageHeight")?.as_f64()?;
            let rect = object.get("rect")?.as_object()?;
            let x = rect.get("x")?.as_f64()?;
            let y = rect.get("y")?.as_f64()?;
            let width = rect.get("width")?.as_f64()?;
            let height = rect.get("height")?.as_f64()?;
            if !page_height.is_finite()
                || page_height <= 0.0
                || !x.is_finite()
                || x < 0.0
                || !y.is_finite()
                || y < 0.0
                || !width.is_finite()
                || width <= 0.0
                || !height.is_finite()
                || height <= 0.0
                || y + height > page_height
            {
                return None;
            }

            Some(SignatureAnchor {
                role: role.to_owned(),
                slot_id: object
                    .get("slotId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
                kind,
                page_index: page_index as i32,
                page_height,
                x,
                y,
                width,
                height,
            })
        })
        .collect()
}

fn resolve_recipient_anchor_sets(
    anchors: &[SignatureAnchor],
    request: &SignatureProviderSendRequest,
) -> Result<Vec<Vec<SignatureAnchor>>, (String, bool)> {
    let sets = anchor_sets(anchors);
    let mut resolved = Vec::with_capacity(request.recipients.len());

    for (index, recipient) in request.recipients.iter().enumerate() {
        let role = recipient
            .execution_role
            .as_deref()
            .or(request.signature_role.as_deref());
        let slot_id = recipient
            .execution_slot_id
            .as_deref()
            .or(request.signature_slot_id.as_deref());

        let matches = sets
            .iter()
            .filter(|(set_role, set_slot, _)| {
                role.is_none_or(|role| role == set_role)
                    && slot_id.is_none_or(|slot| set_slot.as_deref() == Some(slot))
            })
            .collect::<Vec<_>>();

        let selected = if matches.len() == 1 {
            matches[0].2.clone()
        } else if role.is_none() && slot_id.is_none() && sets.len() == request.recipients.len() {
            sets[index].2.clone()
        } else {
            return Err((
                "The issued PDF signature anchor is missing or ambiguous for the selected participant. Reissue the document and select a declared signature role."
                    .into(),
                false,
            ));
        };
        resolved.push(selected);
    }

    Ok(resolved)
}

fn anchor_sets(anchors: &[SignatureAnchor]) -> Vec<(String, Option<String>, Vec<SignatureAnchor>)> {
    let mut order = Vec::<(String, Option<String>)>::new();
    let mut seen = BTreeSet::new();
    for anchor in anchors
        .iter()
        .filter(|anchor| anchor.kind == AnchorKind::Signature)
    {
        let key = (anchor.role.clone(), anchor.slot_id.clone());
        if seen.insert(key.clone()) {
            order.push(key);
        }
    }

    order
        .into_iter()
        .map(|(role, slot_id)| {
            let fields = anchors
                .iter()
                .filter(|anchor| anchor.role == role && anchor.slot_id == slot_id)
                .cloned()
                .collect::<Vec<_>>();
            (role, slot_id, fields)
        })
        .collect()
}

fn anchor_field(anchor: SignatureAnchor) -> BoldSignFormField {
    BoldSignFormField {
        field_type: match anchor.kind {
            AnchorKind::Signature => "Signature",
            AnchorKind::Initial => "Initial",
            AnchorKind::Date => "DateSigned",
        }
        .into(),
        page_number: anchor.page_index + 1,
        x: anchor.x,
        y: anchor.page_height - anchor.y - anchor.height,
        width: anchor.width,
        height: anchor.height,
        font_size: Some(match anchor.kind {
            AnchorKind::Signature => 14,
            AnchorKind::Initial => 10,
            AnchorKind::Date => 9,
        }),
        date_format: (anchor.kind == AnchorKind::Date).then(|| "MMM dd, yyyy".into()),
    }
}

fn neutral_event_name(event: SignatureProviderEvent) -> &'static str {
    match event {
        SignatureProviderEvent::Sent => "sent",
        SignatureProviderEvent::Viewed => "viewed",
        SignatureProviderEvent::Signed => "signed",
        SignatureProviderEvent::Completed => "completed",
        SignatureProviderEvent::Declined => "declined",
        SignatureProviderEvent::Voided => "voided",
        SignatureProviderEvent::Expired => "expired",
        SignatureProviderEvent::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_bottom_left_anchor_converts_to_boldsign_top_left_y() {
        let field = anchor_field(SignatureAnchor {
            role: "BUYER".into(),
            slot_id: Some("BUYER:1".into()),
            kind: AnchorKind::Signature,
            page_index: 0,
            page_height: 792.0,
            x: 72.0,
            y: 100.0,
            width: 180.0,
            height: 30.0,
        });
        assert_eq!(field.page_number, 1);
        assert_eq!(field.y, 662.0);
    }

    #[test]
    fn ambiguous_role_only_anchor_is_rejected() {
        let anchors = vec![
            SignatureAnchor {
                role: "BUYER".into(),
                slot_id: Some("BUYER:1".into()),
                kind: AnchorKind::Signature,
                page_index: 0,
                page_height: 792.0,
                x: 10.0,
                y: 10.0,
                width: 100.0,
                height: 20.0,
            },
            SignatureAnchor {
                role: "BUYER".into(),
                slot_id: Some("BUYER:2".into()),
                kind: AnchorKind::Signature,
                page_index: 0,
                page_height: 792.0,
                x: 10.0,
                y: 40.0,
                width: 100.0,
                height: 20.0,
            },
        ];
        let request = SignatureProviderSendRequest {
            signature_request_id: "00000000-0000-0000-0000-000000000001".into(),
            transaction_document_id: "00000000-0000-0000-0000-000000000002".into(),
            recipients: vec![domain::SignatureRecipient {
                role: SignatureRecipientRole::Signer,
                name: "Buyer".into(),
                email: "buyer@example.test".into(),
                order: 1,
                execution_role: Some("BUYER".into()),
                execution_slot_id: None,
            }],
            message: None,
            signature_role: None,
            signature_slot_id: None,
            completion_recipient_emails: vec![],
        };
        assert!(resolve_recipient_anchor_sets(&anchors, &request).is_err());
    }
}
