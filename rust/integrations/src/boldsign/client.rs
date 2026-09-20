use crate::boldsign::BoldSignConfig;
use base64::Engine;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BoldSignClient {
    config: BoldSignConfig,
    http: Client,
}

#[derive(Debug, Clone)]
pub struct BoldSignClientError {
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for BoldSignClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BoldSignClientError {}

#[derive(Debug, Clone)]
pub struct BoldSignFormField {
    pub field_type: String,
    pub page_number: i32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size: Option<i32>,
    pub date_format: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BoldSignDirectSigner {
    pub name: String,
    pub email_address: String,
    pub signer_type: String,
    pub signer_order: i32,
    pub form_fields: Vec<BoldSignFormField>,
}

#[derive(Debug, Clone)]
pub struct BoldSignSendDocument {
    pub file_bytes: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
    pub title: Option<String>,
    pub message: Option<String>,
    pub signers: Vec<BoldSignDirectSigner>,
    pub completion_cc_emails: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BoldSignDocumentProperties {
    pub document_id: String,
    pub status: String,
    pub file_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BoldSignDownload {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
}

impl BoldSignClient {
    pub fn new(config: BoldSignConfig) -> Result<Self, BoldSignClientError> {
        let http = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|error| BoldSignClientError {
                message: format!("BoldSign HTTP client could not be created: {error}"),
                retryable: false,
            })?;
        Ok(Self { config, http })
    }

    /// Envelope creation is intentionally single-attempt. An automatic retry
    /// after timeout/network failure could create a second legal envelope.
    pub async fn send_document(
        &self,
        input: &BoldSignSendDocument,
    ) -> Result<String, BoldSignClientError> {
        let mut form = Form::new()
            .part(
                "files",
                Part::bytes(input.file_bytes.clone())
                    .file_name(input.filename.clone())
                    .mime_str(&input.mime_type)
                    .map_err(|error| BoldSignClientError {
                        message: format!("BoldSign PDF mime type is invalid: {error}"),
                        retryable: false,
                    })?,
            )
            .text("title", input.title.clone().unwrap_or_default())
            .text("message", input.message.clone().unwrap_or_default())
            .text("enableReassign", "false");

        if input.signers.len() > 1 {
            form = form.text("enableSigningOrder", "true");
        }

        for (index, signer) in input.signers.iter().enumerate() {
            let prefix = format!("signers[{index}]");
            form = form
                .text(format!("{prefix}.name"), signer.name.clone())
                .text(
                    format!("{prefix}.emailAddress"),
                    signer.email_address.clone(),
                )
                .text(format!("{prefix}.signerType"), signer.signer_type.clone())
                .text(
                    format!("{prefix}.signerOrder"),
                    signer.signer_order.to_string(),
                )
                .text(format!("{prefix}.authenticationType"), "EmailOTP");

            for (field_index, field) in signer.form_fields.iter().enumerate() {
                let field_prefix = format!("{prefix}.formFields[{field_index}]");
                form = form
                    .text(
                        format!("{field_prefix}.fieldType"),
                        field.field_type.clone(),
                    )
                    .text(
                        format!("{field_prefix}.pageNumber"),
                        field.page_number.to_string(),
                    )
                    .text(format!("{field_prefix}.bounds.x"), field.x.to_string())
                    .text(format!("{field_prefix}.bounds.y"), field.y.to_string())
                    .text(
                        format!("{field_prefix}.bounds.width"),
                        field.width.to_string(),
                    )
                    .text(
                        format!("{field_prefix}.bounds.height"),
                        field.height.to_string(),
                    )
                    .text(format!("{field_prefix}.isRequired"), "true");
                if let Some(font_size) = field.font_size {
                    form = form.text(
                        format!("{field_prefix}.fontSize"),
                        font_size.to_string(),
                    );
                }
                if let Some(date_format) = &field.date_format {
                    form = form.text(
                        format!("{field_prefix}.dateFormat"),
                        date_format.clone(),
                    );
                }
            }
        }

        for (index, email) in input.completion_cc_emails.iter().enumerate() {
            form = form.text(format!("cc[{index}].emailAddress"), email.clone());
        }
        if !input.completion_cc_emails.is_empty() {
            form = form.text("recipientNotificationSettings.completed", "true");
        }

        let response = self
            .http
            .post(self.url("/v1/document/send"))
            .header("x-api-key", &self.config.api_key)
            .header("accept", "application/json")
            .multipart(form)
            .send()
            .await
            .map_err(classify_transport_error)?;
        let value = self.parse_json_response(response, false).await?;
        let document_id = value
            .get("documentId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| BoldSignClientError {
                message: "BoldSign document send response is missing documentId.".into(),
                retryable: false,
            })?;
        Ok(document_id.to_owned())
    }

    pub async fn get_document_properties(
        &self,
        document_id: &str,
    ) -> Result<BoldSignDocumentProperties, BoldSignClientError> {
        self.retry(|| async {
            let response = self
                .http
                .get(self.url("/v1/document/properties"))
                .header("x-api-key", &self.config.api_key)
                .header("accept", "application/json")
                .query(&[("documentId", document_id)])
                .send()
                .await
                .map_err(classify_transport_error)?;
            let value = self.parse_json_response(response, false).await?;
            let id = value
                .get("documentId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| BoldSignClientError {
                    message: "BoldSign document properties response is missing documentId."
                        .into(),
                    retryable: false,
                })?
                .to_owned();
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| BoldSignClientError {
                    message: "BoldSign document properties response is missing status.".into(),
                    retryable: false,
                })?
                .to_owned();
            let file_ids = value
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|file| file.get("id").and_then(Value::as_str))
                .map(str::to_owned)
                .collect();
            Ok(BoldSignDocumentProperties {
                document_id: id,
                status,
                file_ids,
            })
        })
        .await
    }

    pub async fn revoke_document(
        &self,
        document_id: &str,
        reason: &str,
    ) -> Result<(), BoldSignClientError> {
        self.retry(|| async {
            let response = self
                .http
                .post(self.url("/v1/document/revoke"))
                .header("x-api-key", &self.config.api_key)
                .header("accept", "application/json")
                .json(&serde_json::json!({
                    "documentId": document_id,
                    "reason": reason,
                }))
                .send()
                .await
                .map_err(classify_transport_error)?;
            self.parse_empty_or_json_response(response).await
        })
        .await
    }

    pub async fn download_document(
        &self,
        document_id: &str,
    ) -> Result<BoldSignDownload, BoldSignClientError> {
        self.retry(|| async {
            let response = self
                .http
                .get(self.url("/v1/document/download"))
                .header("x-api-key", &self.config.api_key)
                .header("accept", "application/json")
                .header("x-response-format", "base64")
                .query(&[("documentId", document_id)])
                .send()
                .await
                .map_err(classify_transport_error)?;
            let value = self.parse_json_response(response, false).await?;
            let encoded = value
                .get("file")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| BoldSignClientError {
                    message: "BoldSign document download response is missing file.".into(),
                    retryable: false,
                })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|_| BoldSignClientError {
                    message: "BoldSign document download returned invalid base64.".into(),
                    retryable: false,
                })?;
            if bytes.is_empty() {
                return Err(BoldSignClientError {
                    message: "BoldSign document download returned an empty file.".into(),
                    retryable: false,
                });
            }
            Ok(BoldSignDownload {
                bytes,
                filename: value
                    .get("fileName")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{document_id}-signed.pdf")),
                mime_type: value
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| "application/pdf".into()),
            })
        })
        .await
    }

    pub async fn download_audit_trail(
        &self,
        document_id: &str,
    ) -> Result<BoldSignDownload, BoldSignClientError> {
        self.retry(|| async {
            let response = self
                .http
                .get(self.url("/v1/document/downloadAuditLog"))
                .header("x-api-key", &self.config.api_key)
                .header("accept", "application/pdf")
                .query(&[("documentId", document_id)])
                .send()
                .await
                .map_err(classify_transport_error)?;
            let status = response.status();
            if !status.is_success() {
                return Err(http_error(status, response.text().await.unwrap_or_default()));
            }
            let bytes = response
                .bytes()
                .await
                .map_err(classify_transport_error)?
                .to_vec();
            if bytes.is_empty() {
                return Err(BoldSignClientError {
                    message: "BoldSign audit trail download returned an empty file.".into(),
                    retryable: false,
                });
            }
            Ok(BoldSignDownload {
                bytes,
                filename: format!("{document_id}-audit-trail.pdf"),
                mime_type: "application/pdf".into(),
            })
        })
        .await
    }

    async fn retry<T, F, Fut>(&self, mut operation: F) -> Result<T, BoldSignClientError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, BoldSignClientError>>,
    {
        let attempts = self.config.max_attempts.max(1);
        let mut last = None;
        for attempt in 0..attempts {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(error) if error.retryable && attempt + 1 < attempts => {
                    let multiplier = 1_u64 << attempt.min(16);
                    let delay = self
                        .config
                        .retry_base_delay_ms
                        .saturating_mul(multiplier)
                        .min(self.config.retry_max_delay_ms);
                    last = Some(error);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(last.unwrap_or_else(|| BoldSignClientError {
            message: "BoldSign retry loop ended without an attempt.".into(),
            retryable: false,
        }))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    async fn parse_empty_or_json_response(
        &self,
        response: reqwest::Response,
    ) -> Result<(), BoldSignClientError> {
        let status = response.status();
        let body = response.text().await.map_err(classify_transport_error)?;
        if !status.is_success() {
            return Err(http_error(status, body));
        }
        if body.trim().is_empty() {
            return Ok(());
        }
        serde_json::from_str::<Value>(&body).map_err(|_| BoldSignClientError {
            message: "BoldSign API returned non-JSON content.".into(),
            retryable: false,
        })?;
        Ok(())
    }

    async fn parse_json_response(
        &self,
        response: reqwest::Response,
        allow_empty: bool,
    ) -> Result<Value, BoldSignClientError> {
        let status = response.status();
        let body = response.text().await.map_err(classify_transport_error)?;
        if !status.is_success() {
            return Err(http_error(status, body));
        }
        if body.trim().is_empty() && allow_empty {
            return Ok(Value::Null);
        }
        if body.trim().is_empty() {
            return Err(BoldSignClientError {
                message: "BoldSign API returned an empty body.".into(),
                retryable: false,
            });
        }
        serde_json::from_str(&body).map_err(|_| BoldSignClientError {
            message: "BoldSign API returned non-JSON content.".into(),
            retryable: false,
        })
    }
}

fn classify_transport_error(error: reqwest::Error) -> BoldSignClientError {
    BoldSignClientError {
        message: if error.is_timeout() {
            "BoldSign API request timed out.".into()
        } else {
            format!("BoldSign API transport failed: {error}")
        },
        retryable: error.is_timeout() || error.is_connect() || error.is_request(),
    }
}

fn http_error(status: StatusCode, body: String) -> BoldSignClientError {
    let retryable = matches!(
        status.as_u16(),
        408 | 425 | 429 | 500 | 502 | 503 | 504
    );
    let excerpt = body.chars().take(200).collect::<String>();
    BoldSignClientError {
        message: format!(
            "BoldSign API failed with HTTP {}{}",
            status.as_u16(),
            if excerpt.trim().is_empty() {
                String::new()
            } else {
                format!(": {excerpt}")
            }
        ),
        retryable,
    }
}

