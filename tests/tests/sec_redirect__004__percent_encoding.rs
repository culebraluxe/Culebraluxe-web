//! SEC.REDIRECT — Percent encoding (TST-SEC-REDIRECT-004).
//!
//! Contract: `encode` and `percent_decode` correctly handle percent encoding/decoding.
//!
//! Level: L0 Pure — pure function with deterministic inputs.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_redirect__004__percent_encoding

use web::api::google_auth::{encode, percent_decode};

const HARNESS: &str = "RedirectPolicyHarness/L0 Pure";

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-REDIRECT-004); the file and the assay use it.
fn sec_redirect_004__percent_encoding() {
    // 1. Round-trip encoding/decoding preserves valid paths
    let paths = [
        "/portal/dashboard",
        "/portal/clients?selected=a b&x=1",
        "/portal/clients?a=1&b=2",
        "/portal/100%",
        "/portal/Casa-Luar-áé",
        "/portal/a%",
        "/callback?state=abc123&code=xyz789",
    ];
    for path in paths {
        let encoded = encode(path);
        let decoded = percent_decode(&encoded);
        assert_eq!(
            decoded, path,
            "{HARNESS}: round-trip preserves path: {path}"
        );
    }

    // 2. encode percent-encodes special characters
    assert_eq!(
        encode(" "),
        "%20",
        "{HARNESS}: space encoded as %20"
    );
    assert_eq!(
        encode("!"),
        "%21",
        "{HARNESS}: ! encoded as %21"
    );
    assert_eq!(
        encode("#"),
        "%23",
        "{HARNESS}: # encoded as %23"
    );
    assert_eq!(
        encode("$"),
        "%24",
        "{HARNESS}: $ encoded as %24"
    );
    assert_eq!(
        encode("%"),
        "%25",
        "{HARNESS}: % encoded as %25"
    );
    assert_eq!(
        encode("&"),
        "%26",
        "{HARNESS}: & encoded as %26"
    );
    assert_eq!(
        encode("'"),
        "%27",
        "{HARNESS}: ' encoded as %27"
    );
    assert_eq!(
        encode("("),
        "%28",
        "{HARNESS}: ( encoded as %28"
    );
    assert_eq!(
        encode(")"),
        "%29",
        "{HARNESS}: ) encoded as %29"
    );
    assert_eq!(
        encode("*"),
        "%2A",
        "{HARNESS}: * encoded as %2A"
    );
    assert_eq!(
        encode("+"),
        "%2B",
        "{HARNESS}: + encoded as %2B"
    );
    assert_eq!(
        encode(","),
        "%2C",
        "{HARNESS}: , encoded as %2C"
    );
    assert_eq!(
        encode("/"),
        "%2F",
        "{HARNESS}: / encoded as %2F"
    );
    assert_eq!(
        encode(":"),
        "%3A",
        "{HARNESS}: : encoded as %3A"
    );
    assert_eq!(
        encode(";"),
        "%3B",
        "{HARNESS}: ; encoded as %3B"
    );
    assert_eq!(
        encode("="),
        "%3D",
        "{HARNESS}: = encoded as %3D"
    );
    assert_eq!(
        encode("?"),
        "%3F",
        "{HARNESS}: ? encoded as %3F"
    );
    assert_eq!(
        encode("@"),
        "%40",
        "{HARNESS}: @ encoded as %40"
    );
    assert_eq!(
        encode("["),
        "%5B",
        "{HARNESS}: [ encoded as %5B"
    );
    assert_eq!(
        encode("]"),
        "%5D",
        "{HARNESS}: ] encoded as %5D"
    );

    // 3. encode does not encode unreserved characters (RFC 3986)
    let unreserved = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    assert_eq!(
        encode(unreserved),
        unreserved,
        "{HARNESS}: unreserved characters not encoded"
    );

    // 4. percent_decode handles valid percent-encoded sequences
    assert_eq!(percent_decode("%20"), " ");
    assert_eq!(percent_decode("%21"), "!");
    assert_eq!(percent_decode("%41"), "A");
    assert_eq!(percent_decode("%7E"), "~");

    // 5. percent_decode leaves invalid percent sequences alone
    assert_eq!(percent_decode("%zz"), "%zz");
    assert_eq!(percent_decode("%"), "%");
    assert_eq!(percent_decode("%2"), "%2");
    assert_eq!(percent_decode("%2G"), "%2G");

    // 6. percent_decode handles incomplete sequences at end of string
    assert_eq!(percent_decode("hello%"), "hello%");
    assert_eq!(percent_decode("hello%2"), "hello%2");

    // 7. percent_decode replaces invalid UTF-8 with replacement character
    assert_eq!(percent_decode("%FF"), "\u{FFFD}");
    assert_eq!(percent_decode("%C0%80"), "\u{FFFD}\u{FFFD}"); // overlong encoding

    // 8. percent_decode handles mixed valid and invalid sequences
    assert_eq!(percent_decode("hello%20world%zz"), "hello world%zz");

    // 9. encode produces uppercase hex digits
    assert!(encode(" ").chars().all(|c| c.is_ascii_uppercase() || c == '%'));
    assert_eq!(encode("\u{00E9}"), "%C3%A9"); // é -> UTF-8 bytes C3 A9

    // 10. Empty string handling
    assert_eq!(encode(""), "");
    assert_eq!(percent_decode(""), "");
}