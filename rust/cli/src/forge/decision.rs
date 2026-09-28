//! Decision mirrors — shape checks only. Ported from the retired `lib/forge-decision.ts` (deleted with
//! the TypeScript application in `4cf98110`).
//!
//! A decision is ONE ROW with a stable key, and its git mirror is what an agent without database access
//! reads. Whether a mirror still matches its row needs the database, and that half belongs to
//! `pnpm forge:decision check`. What can be checked offline is the SHAPE, and shape is what a hand edit
//! breaks first: the filename IS the key, the status is one of three, the statement is one sentence.

pub const DECISION_STATUSES: [&str; 3] = ["candidate", "active", "superseded"];

/// The cap migration 180's CHECK constraint enforces, restated so a writer gets a sentence instead of a
/// Postgres constraint name.
pub const MAX_STATEMENT_LENGTH: usize = 240;

/// The shape the lint reads. The field block also carries `domain`, `owner` and `evidence`; this gate
/// does not judge those, and whether a mirror still matches its ROW is the database check's job — so
/// they are parsed as fields (they must not be mistaken for the statement) and not returned.
pub struct ParsedDecisionFile {
    pub key: String,
    pub status: String,
    pub statement: String,
}

/// Keys are slugs because humans, packets and mirror filenames all reference them.
pub fn is_valid_key(key: &str) -> bool {
    if key.is_empty() || key.len() > 80 {
        return false;
    }
    let mut previous_dash = true; // a leading dash is invalid: the key opens alphanumeric
    for character in key.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            previous_dash = false;
            continue;
        }
        if character == '-' && !previous_dash {
            previous_dash = true;
            continue;
        }
        return false;
    }
    !previous_dash
}

/// The inverse of `renderDecisionFile`, strict enough to catch a hand edit.
pub fn parse_decision_file(markdown: &str, fallback_key: &str) -> ParsedDecisionFile {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut last_field_index: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        let Some((name, value)) = parse_field(line) else {
            continue;
        };
        // Last write wins, as the TypeScript Map did.
        if let Some(existing) = fields.iter_mut().find(|(key, _)| *key == name) {
            existing.1 = value;
        } else {
            fields.push((name, value));
        }
        last_field_index = Some(index);
    }
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let title = lines
        .iter()
        .find(|line| line.starts_with("# "))
        .map(|line| line[2..].trim().to_string())
        .unwrap_or_else(|| fallback_key.to_string());
    // The statement is the first real content line AFTER the last field, not after the first one: the
    // field block is a list, so slicing from the first match would read `- domain: forge` as the rule.
    let statement = lines
        .iter()
        .skip(last_field_index.map(|index| index + 1).unwrap_or(0))
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with('<'))
        .unwrap_or_default()
        .to_string();

    ParsedDecisionFile {
        key: title,
        status: field("status"),
        statement,
    }
}

/// `- name: value`, with a lowercase field name — the mirror's only structure.
fn parse_field(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('-')?;
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let (name, value) = rest.trim_start().split_once(':')?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return None;
    }
    Some((name.to_string(), value.trim().to_string()))
}

/// The database refuses a two-sentence statement and so does this, for the same reason: the packet's
/// first stop condition is "the decision table becomes a blog".
pub fn validate_statement(statement: &str) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let trimmed = statement.trim();
    let length = trimmed.chars().count();
    if trimmed.is_empty() {
        problems.push("a decision needs a statement".to_string());
    }
    if length > MAX_STATEMENT_LENGTH {
        problems.push(format!(
            "statement is {length} characters; the cap is {MAX_STATEMENT_LENGTH} (one sentence, not a paragraph)"
        ));
    }
    if more_than_one_sentence(trimmed) {
        problems.push(
            "statement reads as more than one sentence; say the one thing that is true".to_string(),
        );
    }
    problems
}

/// `/[.!?]\s+\S/` — a terminator followed by whitespace and more text.
fn more_than_one_sentence(value: &str) -> bool {
    let characters: Vec<char> = value.chars().collect();
    for (index, character) in characters.iter().enumerate() {
        if !matches!(character, '.' | '!' | '?') {
            continue;
        }
        let mut next = index + 1;
        let mut saw_space = false;
        while let Some(character) = characters.get(next) {
            if !character.is_whitespace() {
                break;
            }
            saw_space = true;
            next += 1;
        }
        if saw_space && characters.get(next).is_some() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_good_mirror_parses_and_reads_as_one_sentence() {
        let parsed = parse_decision_file(
            "# intent-is-not-status\n\n<!-- GENERATED from forge_decision. Do not hand-edit. -->\n\n\
             - status: active\n- domain: forge\n\nIntent is recorded in its own column or table.\n",
            "",
        );
        assert_eq!(parsed.key, "intent-is-not-status");
        assert_eq!(parsed.status, "active");
        assert_eq!(parsed.statement, "Intent is recorded in its own column or table.");
        assert!(validate_statement(&parsed.statement).is_empty());
    }

    #[test]
    fn the_statement_is_the_line_after_the_last_field() {
        let parsed = parse_decision_file("# x\n\n- status: active\n- domain: forge\n\nOne thing.\n", "");
        assert_eq!(parsed.statement, "One thing.");
    }

    #[test]
    fn a_two_sentence_statement_is_refused() {
        let problems = validate_statement("First thing is true. Second thing is also true.");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("more than one sentence"));
        assert!(validate_statement("").iter().any(|p| p.contains("needs a statement")));
        assert!(!validate_statement(&"x".repeat(241)).is_empty());
        assert!(validate_statement("One sentence is true.").is_empty());
    }

    #[test]
    fn keys_are_lowercase_slugs() {
        assert!(is_valid_key("intent-is-not-status"));
        assert!(is_valid_key("x1"));
        assert!(!is_valid_key("Intent-Is-Not-Status"));
        assert!(!is_valid_key("-leading-dash"));
        assert!(!is_valid_key("trailing-dash-"));
        assert!(!is_valid_key("double--dash"));
        assert!(!is_valid_key(""));
        assert!(!is_valid_key(&"x".repeat(81)));
    }
}
