//! PROP.PROPERTY_BASED — phone/email normalization (TST-PROP-PROPERTY-BASED-007).
//!
//! Contract: identity matching runs on canonical forms. An email trims and
//! lowercases, and only when it is shaped like an address (one `@`, no
//! whitespace, a dotted domain with non-empty sides); a phone keeps ASCII
//! digits only and maps to canonical E.164 for US/Puerto Rico (10 digits, or
//! 11 starting with `1`); anything else is `None` — quarantined, never
//! guessed. Normalization is idempotent, and a mailbox never normalizes to a
//! value that is not email-shaped.
//!
//! Level: L0 Pure — the executable boundary is
//! `model::{apple_messages, applemail}`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__007__phone_email_normalization

use model::apple_messages::{normalize_email, normalize_phone};
use model::applemail::{normalize_mailbox, parse_sender_address};
use proptest::prelude::*;

fn local_part() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z0-9._%+-]{1,12}").unwrap()
}

fn domain_label() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z0-9-]{1,12}").unwrap()
}

fn email_address() -> impl Strategy<Value = String> {
    (local_part(), domain_label(), domain_label())
        .prop_map(|(local, left, right)| format!("{local}@{left}.{right}"))
}

fn digit_string() -> impl Strategy<Value = String> {
    prop::collection::vec(0u8..10, 0..16)
        .prop_map(|digits| digits.iter().map(|d| (b'0' + d) as char).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Canonical forms are stable, idempotent and shaped; unshaped input is
    /// refused; fixed vectors pin the documented US/PR phone rule.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_007__phone_email_normalization(
        email in email_address(),
        digits in digit_string(),
    ) {
        // Fixed positives: the documented canonical forms.
        prop_assert_eq!(
            normalize_email("  A@B.COM "),
            Some("a@b.com".to_string())
        );
        prop_assert_eq!(
            normalize_mailbox("  Dana@Example.COM "),
            Some("dana@example.com".to_string())
        );
        prop_assert_eq!(
            normalize_phone("+1 (787) 555-1234"),
            Some("+17875551234".to_string())
        );
        prop_assert_eq!(
            parse_sender_address(Some("Dana <dana@example.com>")),
            Some(("dana@example.com".to_string(), Some("Dana".to_string())))
        );
        // Fixed negatives: unshaped input is quarantined, never guessed.
        prop_assert_eq!(normalize_email(""), None);
        prop_assert_eq!(normalize_email("dana@example"), None);
        prop_assert_eq!(normalize_email("a@b@c.com"), None);
        prop_assert_eq!(normalize_email("a @b.com"), None);
        prop_assert_eq!(normalize_mailbox("dana@example"), None);
        prop_assert_eq!(normalize_phone(""), None);
        prop_assert_eq!(normalize_phone("44 20 7946 0958"), None);
        prop_assert_eq!(normalize_phone("123"), None);

        // Property: a generated address normalizes to its lowercase self —
        // and only its lowercase self.
        let canonical = email.to_lowercase();
        prop_assert_eq!(normalize_email(&email), Some(canonical.clone()));
        prop_assert_eq!(normalize_mailbox(&email), Some(canonical.clone()));
        // Property: normalization is idempotent — the canonical form is a fixpoint.
        prop_assert_eq!(normalize_email(&canonical), Some(canonical.clone()));
        prop_assert_eq!(normalize_mailbox(&canonical), Some(canonical.clone()));
        // Property: padding with whitespace changes nothing.
        prop_assert_eq!(
            normalize_email(&format!("  {email}  ")),
            Some(canonical.clone())
        );
        // Property: casing the address never escapes the lowercase rule.
        prop_assert_eq!(
            normalize_email(&email.to_uppercase()),
            Some(canonical.clone())
        );

        // Property: the phone rule is exactly the digit rule — 10 digits get
        // `+1`, 11 starting with `1` get `+`, everything else is refused.
        let expected = if digits.len() == 11 && digits.starts_with('1') {
            Some(format!("+{digits}"))
        } else if digits.len() == 10 {
            Some(format!("+1{digits}"))
        } else {
            None
        };
        prop_assert_eq!(normalize_phone(&digits), expected);
        // Property: phone formatting characters never affect the outcome.
        let decorated = format!("+1 ({}) {}-{}", "787", "555", "1234");
        prop_assert_eq!(
            normalize_phone(&decorated),
            Some("+17875551234".to_string())
        );
    }
}
