//! Credential shapes — the ONE catalog, ported from the retired `lib/secret-shapes.ts`
//! (deleted with the TypeScript application in `4cf98110`).
//!
//! Two readers asked "does this line look like a credential?": the harness linter and the candidate
//! publish gate. Two lists answering that question would be two sources for one fact, so the shapes
//! live in one place. The TypeScript copy is gone; this is that one place now.
//!
//! A match is a REFUSAL, so a false positive blocks real work. Keep the patterns narrow and
//! line-oriented: no `^`/`$` anchors (a match anywhere on the line is the signal), and no state
//! carried between lines.

use regex::Regex;
use std::sync::OnceLock;

/// Stable rule names, printed in a finding. A change here changes output, so it is a deliberate act.
const SHAPES: [(&str, &str); 4] = [
    ("openai-style key", r"\bsk-[A-Za-z0-9]{16,}\b"),
    ("github token", r"\bghp_[A-Za-z0-9]{20,}\b"),
    ("aws access key id", r"\bAKIA[0-9A-Z]{12,}\b"),
    (
        "database url with credentials",
        r#"(?i)postgres(?:ql)?://[^\s'"]+:[^\s'"]+@"#,
    ),
];

fn compiled() -> &'static Vec<(&'static str, Regex)> {
    static COMPILED: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        SHAPES
            .iter()
            .map(|(name, pattern)| {
                (
                    *name,
                    Regex::new(pattern).expect("credential shape pattern is a compile-time constant"),
                )
            })
            .collect()
    })
}

/// Every shape this line matches. Empty means the line carries no known credential shape.
pub fn shapes_in_line(line: &str) -> Vec<&'static str> {
    compiled()
        .iter()
        .filter(|(_, pattern)| pattern.is_match(line))
        .map(|(name, _)| *name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_the_shapes_the_catalog_names() {
        assert_eq!(shapes_in_line("token: sk-abcdefghijklmnopqrstuvwx"), vec!["openai-style key"]);
        assert_eq!(
            shapes_in_line("db: postgres://user:pw@host/db"),
            vec!["database url with credentials"]
        );
        assert_eq!(
            shapes_in_line("POSTGRESQL://u:p@h/d"),
            vec!["database url with credentials"]
        );
    }

    #[test]
    fn does_not_fire_on_prose_or_bare_urls() {
        assert!(shapes_in_line("the secret shapes live in one catalog").is_empty());
        assert!(shapes_in_line("DATABASE_URL_DEV is read from .env.local").is_empty());
        assert!(shapes_in_line("https://example.com/sk-short").is_empty());
    }
}
