//! Meta WhatsApp Cloud API trust boundary.
//!
//! Rust owns verification of the webhook challenge and the exact raw-body
//! X-Hub-Signature-256 HMAC before any payload reaches canonical intake.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct MetaWhatsAppConfig {
    pub app_secret: String,
    pub phone_number_id: String,
    pub owned_phone_e164: String,
    pub verify_token: String,
}

impl MetaWhatsAppConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            app_secret: required_env("WHATSAPP_APP_SECRET")?,
            phone_number_id: required_env("WHATSAPP_PHONE_NUMBER_ID")?,
            owned_phone_e164: normalize_e164(&required_env("WHATSAPP_OWNED_PHONE_E164")?)?,
            verify_token: required_env("WHATSAPP_VERIFY_TOKEN")?,
        })
    }
}

fn required_env(key: &str) -> Result<String, String> {
    let value = std::env::var(key).unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{key} is not configured."));
    }
    Ok(trimmed.to_owned())
}

pub fn normalize_e164(value: &str) -> Result<String, String> {
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 7 || digits.len() > 15 {
        return Err("WhatsApp owned phone must normalize to a valid E.164 number.".into());
    }
    Ok(format!("+{digits}"))
}

pub fn verify_handshake(
    mode: Option<&str>,
    token: Option<&str>,
    challenge: Option<&str>,
    expected_token: &str,
) -> Option<String> {
    if mode != Some("subscribe") || expected_token.is_empty() {
        return None;
    }
    let token = token?;
    let challenge = challenge?;
    safe_equal(token.as_bytes(), expected_token.as_bytes()).then(|| challenge.to_owned())
}

/// Meta signs the exact raw request body as sha256=<lowercase hex>.
pub fn verify_signature(raw_body: &str, header: Option<&str>, app_secret: &str) -> bool {
    let Some(header) = header else {
        return false;
    };
    let Some(supplied_hex) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Some(supplied) = decode_hex_32(supplied_hex) else {
        return false;
    };
    if app_secret.is_empty() {
        return false;
    }

    let Ok(mut mac) = HmacSha256::new_from_slice(app_secret.as_bytes()) else {
        return false;
    };
    mac.update(raw_body.as_bytes());
    mac.verify_slice(&supplied).is_ok()
}

fn safe_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && bool::from(left.ct_eq(right))
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(body: &str, secret: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        let bytes = mac.finalize().into_bytes();
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("sha256={hex}")
    }

    #[test]
    fn handshake_requires_exact_constant_time_token_match() {
        let expected = ["verify", "token", "fixture"].join("-");
        assert_eq!(
            verify_handshake(
                Some("subscribe"),
                Some(&expected),
                Some("challenge"),
                &expected
            ),
            Some("challenge".into())
        );
        assert_eq!(
            verify_handshake(Some("subscribe"), Some("wrong"), Some("challenge"), &expected),
            None
        );
    }

    #[test]
    fn signature_covers_the_exact_raw_body() {
        let secret = ["test", "app", "secret"].join("-");
        let body = r#"{"object":"whatsapp_business_account"}"#;
        let header = signature(body, &secret);
        assert!(verify_signature(body, Some(&header), &secret));
        assert!(!verify_signature("{}", Some(&header), &secret));
    }

    #[test]
    fn e164_normalization_is_strictly_bounded() {
        assert_eq!(normalize_e164("+1 (787) 555-1212").unwrap(), "+17875551212");
        assert!(normalize_e164("123").is_err());
    }
}
