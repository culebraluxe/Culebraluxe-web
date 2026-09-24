//! Mux Video provider edge.
//!
//! CulebraLuxe stores only Mux identity/metadata for video. Source video bytes
//! upload directly from the browser to a short-lived Mux Direct Upload URL.

use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MuxConfig {
    pub token_id: String,
    pub token_secret: String,
    pub base_url: String,
    pub timeout_ms: u64,
}

impl MuxConfig {
    pub fn from_env() -> Result<Self, String> {
        let token_id = required_mux_env("MUX_TOKEN_ID")?;
        let token_secret = required_mux_env("MUX_TOKEN_SECRET")?;
        let base_url = std::env::var("MUX_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "https://api.mux.com/video/v1".into());
        let timeout_ms = std::env::var("MUX_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(20_000);

        Ok(Self {
            token_id,
            token_secret,
            base_url,
            timeout_ms,
        })
    }
}

fn required_mux_env(key: &str) -> Result<String, String> {
    if let Some(value) = env_value(key) {
        return Ok(value);
    }

    let suffix = environment_suffix(
        std::env::var("VERCEL_ENV").ok().as_deref(),
        std::env::var("APP_ENV").ok().as_deref(),
    );
    if let Some(suffix) = suffix {
        let scoped = format!("{key}_{suffix}");
        if let Some(value) = env_value(&scoped) {
            return Ok(value);
        }
        return Err(format!(
            "Mux config is incomplete; set {key} or environment-specific {scoped}."
        ));
    }

    Err(format!(
        "Mux config is incomplete; set {key}, or declare VERCEL_ENV/APP_ENV so the _DEV/_PROD key can be selected."
    ))
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn environment_suffix(vercel_env: Option<&str>, app_env: Option<&str>) -> Option<&'static str> {
    match vercel_env
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "production" => return Some("PROD"),
        "preview" | "development" => return Some("DEV"),
        _ => {}
    }

    match app_env
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "production" | "prod" => Some("PROD"),
        "preview" | "development" | "dev" | "test" | "testing" => Some("DEV"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct MuxClient {
    config: MuxConfig,
    http: Client,
}

#[derive(Debug, Clone)]
pub struct MuxClientError {
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for MuxClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MuxClientError {}

#[derive(Debug, Clone)]
pub struct MuxDirectUpload {
    pub id: String,
    pub url: Option<String>,
    pub status: String,
    pub asset_id: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MuxPlaybackId {
    pub id: String,
    pub policy: String,
}

#[derive(Debug, Clone)]
pub struct MuxAsset {
    pub id: String,
    pub status: String,
    pub duration_seconds: Option<String>,
    pub aspect_ratio: Option<String>,
    pub playback_ids: Vec<MuxPlaybackId>,
}

impl MuxClient {
    pub fn new(config: MuxConfig) -> Result<Self, MuxClientError> {
        let http = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|error| MuxClientError {
                message: format!("Mux HTTP client could not be created: {error}"),
                retryable: false,
            })?;
        Ok(Self { config, http })
    }

    pub async fn create_direct_upload(
        &self,
        cors_origin: &str,
    ) -> Result<MuxDirectUpload, MuxClientError> {
        let response = self
            .http
            .post(self.url("/uploads"))
            .basic_auth(&self.config.token_id, Some(&self.config.token_secret))
            .json(&serde_json::json!({
                "cors_origin": cors_origin,
                "new_asset_settings": {
                    "playback_policies": ["public"]
                }
            }))
            .send()
            .await
            .map_err(classify_transport_error)?;
        let value = self.parse_data(response).await?;
        parse_upload(&value)
    }

    pub async fn direct_upload(&self, upload_id: &str) -> Result<MuxDirectUpload, MuxClientError> {
        let response = self
            .http
            .get(self.url(&format!("/uploads/{upload_id}")))
            .basic_auth(&self.config.token_id, Some(&self.config.token_secret))
            .send()
            .await
            .map_err(classify_transport_error)?;
        let value = self.parse_data(response).await?;
        parse_upload(&value)
    }

    pub async fn asset(&self, asset_id: &str) -> Result<MuxAsset, MuxClientError> {
        let response = self
            .http
            .get(self.url(&format!("/assets/{asset_id}")))
            .basic_auth(&self.config.token_id, Some(&self.config.token_secret))
            .send()
            .await
            .map_err(classify_transport_error)?;
        let value = self.parse_data(response).await?;
        parse_asset(&value)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    async fn parse_data(&self, response: reqwest::Response) -> Result<Value, MuxClientError> {
        let status = response.status();
        let body = response.text().await.map_err(classify_transport_error)?;
        if !status.is_success() {
            return Err(http_error(status, &body));
        }
        let value: Value = serde_json::from_str(&body).map_err(|_| MuxClientError {
            message: "Mux API returned non-JSON content.".into(),
            retryable: false,
        })?;
        value.get("data").cloned().ok_or_else(|| MuxClientError {
            message: "Mux API response is missing data.".into(),
            retryable: false,
        })
    }
}

fn parse_upload(value: &Value) -> Result<MuxDirectUpload, MuxClientError> {
    let id = required_string(value, "id", "Mux Direct Upload")?;
    let status = required_string(value, "status", "Mux Direct Upload")?;
    let error_message = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(MuxDirectUpload {
        id,
        url: optional_string(value, "url"),
        status,
        asset_id: optional_string(value, "asset_id"),
        error_message,
    })
}

fn parse_asset(value: &Value) -> Result<MuxAsset, MuxClientError> {
    let playback_ids = value
        .get("playback_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(MuxPlaybackId {
                id: item.get("id")?.as_str()?.to_owned(),
                policy: item.get("policy")?.as_str()?.to_owned(),
            })
        })
        .collect();

    Ok(MuxAsset {
        id: required_string(value, "id", "Mux Asset")?,
        status: required_string(value, "status", "Mux Asset")?,
        duration_seconds: value.get("duration").and_then(|value| match value {
            Value::Number(number) => Some(number.to_string()),
            Value::String(value) => Some(value.clone()),
            _ => None,
        }),
        aspect_ratio: optional_string(value, "aspect_ratio"),
        playback_ids,
    })
}

fn required_string(value: &Value, key: &str, kind: &str) -> Result<String, MuxClientError> {
    optional_string(value, key).ok_or_else(|| MuxClientError {
        message: format!("{kind} response is missing {key}."),
        retryable: false,
    })
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn classify_transport_error(error: reqwest::Error) -> MuxClientError {
    MuxClientError {
        message: if error.is_timeout() {
            "Mux API request timed out.".into()
        } else {
            format!("Mux API transport failed: {error}")
        },
        retryable: error.is_timeout() || error.is_connect() || error.is_request(),
    }
}

fn http_error(status: StatusCode, body: &str) -> MuxClientError {
    let retryable = matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504);
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| {
                    error
                        .get("messages")
                        .and_then(Value::as_array)
                        .and_then(|messages| messages.first())
                        .or_else(|| error.get("message"))
                })
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| {
            let excerpt = body.chars().take(200).collect::<String>();
            if excerpt.trim().is_empty() {
                format!("Mux API failed with HTTP {}", status.as_u16())
            } else {
                format!("Mux API failed with HTTP {}: {excerpt}", status.as_u16())
            }
        });

    MuxClientError { message, retryable }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_mux_environment_suffix() {
        assert_eq!(
            environment_suffix(Some("production"), Some("dev")),
            Some("PROD")
        );
        assert_eq!(
            environment_suffix(Some("preview"), Some("prod")),
            Some("DEV")
        );
        assert_eq!(environment_suffix(None, Some("prod")), Some("PROD"));
        assert_eq!(environment_suffix(None, Some("development")), Some("DEV"));
        assert_eq!(environment_suffix(None, None), None);
    }

    #[test]
    fn parses_ready_asset_with_public_playback() {
        let value = serde_json::json!({
            "id": "asset-1",
            "status": "ready",
            "duration": 72.5,
            "aspect_ratio": "16:9",
            "playback_ids": [{"id": "play-1", "policy": "public"}]
        });
        let asset = parse_asset(&value).unwrap();
        assert_eq!(asset.id, "asset-1");
        assert_eq!(asset.duration_seconds.as_deref(), Some("72.5"));
        assert_eq!(asset.playback_ids[0].id, "play-1");
    }
}
