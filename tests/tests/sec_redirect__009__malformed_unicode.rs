//! SEC.REDIRECT — Malformed Unicode handling (TST-SEC-REDIRECT-009).
//!
//! Contract: `encode`, `percent_decode`, and `safe_next` correctly handle malformed Unicode.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__009__malformed_unicode

use web::api::google_auth::{safe_next, encode, percent_decode};

const HARNESS: &str = "SecurityHarness/L0 Pure";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-009); the file and the assay use it.
fn sec_redirect_009__malformed_unicode() {
    // 1. Valid Unicode in paths is preserved
    assert_eq!(
        safe_next(Some("/portal/Casa-Luar-áé")),
        "/portal/Casa-Luar-áé",
        "{HARNESS}: valid Unicode preserved"
    );
    assert_eq!(
        safe_next(Some("/portal/日本語")),
        "/portal/日本語",
        "{HARNESS}: Japanese characters preserved"
    );
    assert_eq!(
        safe_next(Some("/portal/🏠🏡")),
        "/portal/🏠🏡",
        "{HARNESS}: emoji preserved"
    );

    // 2. encode handles Unicode by UTF-8 encoding then percent-encoding
    let café = "café";
    let encoded = encode(café);
    assert_eq!(encoded, "caf%C3%A9");
    let decoded = percent_decode(&encoded);
    assert_eq!(decoded, café);

    // 3. percent_decode replaces invalid UTF-8 sequences with replacement character
    assert_eq!(percent_decode("%FF"), "\u{FFFD}");
    assert_eq!(percent_decode("%C0%80"), "\u{FFFD}\u{FFFD}"); // overlong null
    assert_eq!(percent_decode("%E0%80%80"), "\u{FFFD}\u{FFFD}\u{FFFD}"); // overlong

    // 4. Mixed valid and invalid UTF-8 in percent-encoded string
    assert_eq!(
        percent_decode("hello%20world%FF"),
        "hello world\u{FFFD}"
    );

    // 4. Invalid surrogate pairs in UTF-8 (not valid UTF-8)
    // UTF-8 doesn't have surrogates, but percent_decode operates on bytes
    // so this tests the replacement character behavior
    assert_eq!(percent_decode("%ED%A0%80"), "\u{FFFD}\u{FFFD}\u{FFFD}"); // surrogate

    // 5. safe_next accepts paths with valid Unicode
    assert_eq!(
        safe_next(Some("/portal/café")),
        "/portal/café",
        "{HARNESS}: Unicode in path accepted"
    );

    // 6. safe_next accepts paths with Unicode query parameters
    assert_eq!(
        safe_next(Some("/portal/clients?search=café")),
        "/portal/clients?search=café",
        "{HARNESS}: Unicode in query accepted"
    );

    // 7. percent_decode handles lone continuation bytes
    assert_eq!(percent_decode("%80"), "\u{FFFD}");
    assert_eq!(percent_decode("%BF"), "\u{FFFD}");

    // 8. percent_decode handles truncated multi-byte sequences
    assert_eq!(percent_decode("%C3"), "\u{FFFD}"); // incomplete 2-byte
    assert_eq!(percent_decode("%E2%82"), "\u{FFFD}\u{FFFD}"); // incomplete 3-byte
    assert_eq!(percent_decode("%F0%90%80"), "\u{FFFD}\u{FFFD}\u{FFFD}"); // incomplete 4-byte

    // 9. Round-trip with Unicode
    let unicode_path = "/portal/日本語/测试/тест";
    let encoded = encode(unicode_path);
    let decoded = percent_decode(&encoded);
    assert_eq!(decoded, unicode_path);

    // 10. Null bytes and control characters
    // These would be percent-encoded by encode
    assert_eq!(encode("\0"), "%00");
    assert_eq!(encode("\n"), "%0A");
    assert_eq!(encode("\r"), "%0D");
    assert_eq!(encode("\t"), "%09");
}