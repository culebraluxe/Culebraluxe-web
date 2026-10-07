//! PROP.PROPERTY_BASED — URL redirect sanitizer (TST-PROP-PROPERTY-BASED-006).
//!
//! Contract: after sign-in the browser returns only to a path on this site.
//! Anything else — another origin, a protocol-relative `//host`, a
//! backslash-led `/\host` (every browser reads it as `//host`), an empty or
//! absent value — falls back to `/portal/dashboard`. The cookie value is
//! percent-decoded BEFORE it is filtered, so an encoded bypass decodes into
//! the refusal; and the encode/decode pair round-trips, so a legitimate
//! return address survives the cookie.
//!
//! Level: L0 Pure — the executable boundary is `web::api::google_auth`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__006__url_redirect_sanitizer

use proptest::prelude::*;
use web::api::google_auth::{encode, percent_decode, safe_next};

const FALLBACK: &str = "/portal/dashboard";

/// Path-shaped inputs: absolute paths, with query strings and odd characters.
fn site_path() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 _~.%-]{0,24}".prop_map(|tail| format!("/portal/{tail}"))
}

/// Hostile inputs: off-site URLs, protocol-relative hosts, backslash tricks.
fn hostile() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{3,8}://[a-z.]{3,16}".prop_map(|url| format!("https://{url}")),
        "[a-z.]{3,16}".prop_map(|host| format!("//{host}")),
        "[a-z.]{3,16}".prop_map(|host| format!("/{host}").replacen('/', "/\\", 1)),
        Just(String::new()),
        Just("/\\".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The filter keeps same-site paths byte-identical, refuses everything
    /// hostile to the fallback, decodes before it filters, and round-trips
    /// legitimate addresses through the cookie codec.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_006__url_redirect_sanitizer(
        path in site_path(),
        evil in hostile(),
    ) {
        // Fixed positives: a path on this site passes through untouched.
        prop_assert_eq!(safe_next(Some("/portal/clients?x=1")), "/portal/clients?x=1");
        prop_assert_eq!(safe_next(Some("/")), "/");
        // Fixed negatives: off-site, protocol-relative, backslash, absent.
        prop_assert_eq!(safe_next(Some("//evil.example")), FALLBACK);
        prop_assert_eq!(safe_next(Some("https://evil.example")), FALLBACK);
        prop_assert_eq!(safe_next(Some("/\\evil.example")), FALLBACK);
        prop_assert_eq!(safe_next(Some("/\\")), FALLBACK);
        prop_assert_eq!(safe_next(None), FALLBACK);
        prop_assert_eq!(safe_next(Some("")), FALLBACK);
        // Fixed order: decode happens before filtering — the raw encoded form
        // is not itself refused, but the decoded value is.
        prop_assert_eq!(safe_next(Some("/%5Cevil.example")), "/%5Cevil.example");
        prop_assert_eq!(
            safe_next(Some(&percent_decode("/%5Cevil.example"))),
            FALLBACK
        );
        // Fixed codec: invalid escapes are left alone, never dropped.
        prop_assert_eq!(percent_decode("%zz"), "%zz");
        prop_assert_eq!(percent_decode("/a%"), "/a%");

        // Property: a same-site path survives the filter byte-identical.
        prop_assert_eq!(safe_next(Some(&path)), path.as_str());
        // Property: every hostile shape falls back — never an open redirect.
        prop_assert_eq!(safe_next(Some(&evil)), FALLBACK, "hostile input escaped: {:?}", evil);
        // Property: the cookie round-trips, so a kept address is a kept page.
        prop_assert_eq!(percent_decode(&encode(&path)), path.as_str());
        // Property: the filter is idempotent — filtering twice changes nothing.
        let once = safe_next(Some(&path));
        prop_assert_eq!(safe_next(Some(&once)), once);
        // Property: decoding an encoded hostile value still refuses — the
        // bypass must decode INTO the fallback, not past it.
        let smuggled = format!("/%5C{}", evil.trim_start_matches('/').trim_start_matches('\\'));
        prop_assert_eq!(
            safe_next(Some(&percent_decode(&smuggled))),
            FALLBACK,
            "encoded bypass escaped: {:?}",
            smuggled
        );
    }
}
