//! Moved from `applemail.rs` (move only): normalize_mailbox, bounded_email_subject, parse_sender_address, first_address_token.

#[allow(unused_imports)]
use super::*;

/// Normalize a mailbox address for matching: trimmed, lowercased, and only when it is
/// shaped like an address. The original value is never lost — the caller keeps it.
pub fn normalize_mailbox(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_lowercase();
    if is_email_shaped(&normalized) {
        Some(normalized)
    } else {
        None
    }
}

/// Bound a subject for storage and display: NFKC, control characters to spaces, whitespace
/// collapsed, and anything past the limit truncated with an ellipsis. An over-long subject
/// is shortened, never dropped.
pub fn bounded_email_subject(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let mut normalized = String::with_capacity(value.len());
    for character in value.nfkc() {
        if character.is_control() {
            normalized.push(' ');
        } else {
            normalized.push(character);
        }
    }
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    // The limit is counted in characters, not bytes: a truncation must never split one.
    let length = collapsed.chars().count();
    if length <= EMAIL_SUBJECT_MAX_LENGTH {
        return Some(collapsed);
    }
    let head: String = collapsed
        .chars()
        .take(EMAIL_SUBJECT_MAX_LENGTH - 1)
        .collect::<String>()
        .trim_end()
        .to_owned();
    Some(format!("{head}\u{2026}"))
}

/// `"Dana <dana@example.com>"` -> the address and the display name. A bare address also
/// works. The address is normalized; the name is trimmed and unquoted, or absent.
pub fn parse_sender_address(value: Option<&str>) -> Option<(String, Option<String>)> {
    let value = value?;
    if value.trim().is_empty() {
        return None;
    }
    // Bracketed form: everything before the LAST `<...>` is the display name. When the
    // bracketed address is unusable the row is unaddressed — it is never re-parsed as a
    // bare token, because that would attribute a malformed sender to whoever came first.
    let bracketed = value.rfind('<').and_then(|open| {
        let tail = value[open + 1..].trim_end();
        let inner = tail.strip_suffix('>')?;
        if inner.is_empty() || inner.contains('<') || inner.contains('>') {
            return None;
        }
        Some((&value[..open], inner))
    });

    let (address, raw_name) = match bracketed {
        Some((name, inner)) => (normalize_mailbox(inner)?, Some(name)),
        None => (normalize_mailbox(&first_address_token(value)?)?, None),
    };

    let name = raw_name
        .map(str::trim)
        .map(|name| name.trim_matches(|c| c == '"' || c == '\'').trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    Some((address, name))
}

/// The first token that looks like an address (`[^\s<>]+@[^\s<>]+`, the rule the deleted
/// TypeScript used before it fell back to `null`).
fn first_address_token(value: &str) -> Option<String> {
    let at = value.find('@')?;
    let is_token_char = |c: char| !c.is_whitespace() && c != '<' && c != '>';
    let start = value[..at]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_token_char(*c))
        .map(|(index, c)| index + c.len_utf8())
        .unwrap_or(0);
    let end = value[at + 1..]
        .char_indices()
        .find(|(_, c)| !is_token_char(*c))
        .map(|(index, _)| at + 1 + index)
        .unwrap_or(value.len());
    Some(value[start..end].to_owned())
}
