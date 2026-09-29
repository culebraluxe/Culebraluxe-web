//! Moved from `apple_messages.rs` (move only): is_group_chat_guid, apple_nanos_to_iso, effective_date_iso, normalize_email, is_email_shaped, normalize_phone, handle_to_identities, fingerprint, bounded_preview, apple_service_to_channel, derive_source_account.

#[allow(unused_imports)]
use super::*;

/// A 1:1 vs group chat is decided by the Apple chat GUID, never by participant count.
pub fn is_group_chat_guid(chat_guid: Option<&str>) -> bool {
    let Some(guid) = chat_guid else {
        return false;
    };
    let lower = guid.to_lowercase();
    lower.contains(GROUP_CHAT_GUID_MARKER) || lower.starts_with(GROUP_CHAT_GUID_PLUS_PREFIX)
}

/// Convert an Apple INTEGER-nanoseconds timestamp to an ISO-8601 string.
pub fn apple_nanos_to_iso(raw: f64) -> Option<String> {
    if !raw.is_finite() {
        return None;
    }
    let unix = raw / 1_000_000_000.0 + APPLE_EPOCH_UNIX_OFFSET_SECONDS as f64;
    if !(unix > 0.0) {
        return None;
    }
    let seconds = unix.trunc() as i64;
    let nanos = (((unix - unix.trunc()) * 1_000_000_000.0).round() as i64).clamp(0, 999_999_999);
    let stamp = Utc.timestamp_opt(seconds, nanos as u32).single()?;
    Some(stamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

/// The observed timestamp of a message: the exporter's ISO value, or the raw Apple timestamp when
/// the exporter could not render one.
pub fn effective_date_iso(message: &AppleMessagesMessage) -> Option<String> {
    match message.date_iso.as_ref().map(|value| value.trim()) {
        Some(value) if !value.is_empty() => Some(value.to_owned()),
        _ => message.date.and_then(apple_nanos_to_iso),
    }
}

/// Normalize an email for matching. The original value is always retained by the caller.
pub fn normalize_email(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.encode_utf16().count() > 320 {
        return None;
    }
    if !is_email_shaped(trimmed) {
        return None;
    }
    Some(trimmed.to_lowercase())
}

/// Shared with `crate::applemail::normalize_mailbox`: one email shape rule for the whole crate.
pub(crate) fn is_email_shaped(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) || value.matches('@').count() != 1 {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return false;
    }
    match domain.split_once('.') {
        Some((left, right)) => !left.is_empty() && !right.is_empty(),
        None => false,
    }
}

/// Normalize a phone number for matching (US/Puerto Rico, canonical E.164).
/// `None` means "not reliably matchable" — quarantined, never guessed at.
pub fn normalize_phone(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    if digits.len() == 11 && digits.starts_with('1') {
        return Some(format!("+{digits}"));
    }
    if digits.len() == 10 {
        return Some(format!("+1{digits}"));
    }
    None
}

/// Split an Apple handle id into neutral email/phone identity evidence.
pub fn handle_to_identities(handle_id: &str) -> (Vec<IdentityEvidence>, Vec<IdentityEvidence>) {
    let raw = handle_id.trim();
    if raw.is_empty() {
        return (Vec::new(), Vec::new());
    }
    if let Some(normalized) = normalize_email(raw) {
        return (
            vec![IdentityEvidence {
                value: raw.to_owned(),
                normalized,
                label: None,
            }],
            Vec::new(),
        );
    }
    if let Some(normalized) = normalize_phone(raw) {
        return (
            Vec::new(),
            vec![IdentityEvidence {
                value: raw.to_owned(),
                normalized,
                label: None,
            }],
        );
    }
    (Vec::new(), Vec::new())
}

/// Stable deterministic fingerprint for replay/dedup. Not a cryptographic hash: it is the same
/// algorithm the TypeScript engine used (UTF-16 code units, `cyrb53`-style), so fingerprints written
/// before the port keep their shape and width.
pub fn fingerprint(input: &str) -> String {
    let mut h1: u32 = 0xdeadbeef;
    let mut h2: u32 = 0x41c6ce57;
    for unit in input.encode_utf16() {
        h1 = (h1 ^ unit as u32).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ unit as u32).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    format!("{h2:08x}{h1:08x}")
}

/// Bounded one-line memory cue: whitespace collapsed, capped at 160 characters, `None` when there is
/// no usable text (never fabricate prose).
pub fn bounded_preview(text: Option<&str>) -> Option<String> {
    let raw = text?;
    let one_line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    let units: Vec<u16> = one_line.encode_utf16().collect();
    if units.len() <= APPLE_PREVIEW_MAX_LENGTH {
        return Some(one_line);
    }
    let truncated = String::from_utf16_lossy(&units[..APPLE_PREVIEW_MAX_LENGTH - 1]);
    Some(format!("{}…", truncated.trim_end()))
}

/// Map an Apple service value to the canonical interaction channel.
pub fn apple_service_to_channel(service: Option<&str>) -> &'static str {
    match service.map(str::to_lowercase) {
        Some(value) if value.contains("sms") => "sms",
        _ => "imessage",
    }
}

/// Deterministic source account derived from the export messages.
pub fn derive_source_account(messages: &[AppleMessagesMessage]) -> String {
    for message in messages {
        if let Some(account) = message.account.as_deref() {
            if account.contains('@') && !account.trim().is_empty() {
                return account.to_owned();
            }
        }
    }
    APPLE_LOCAL_SOURCE_ACCOUNT.to_owned()
}

// ---------------------------------------------------------------------------
// Evidence: one source-neutral row per Apple handle identity.
// ---------------------------------------------------------------------------
