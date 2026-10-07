//! INT.GOOGLE — contacts/calendar if currently wired (TST-INT-GOOGLE-002).
//!
//! Contract: the Google contacts capability IS wired — the Gmail census
//! artifact parses into source-neutral identity evidence
//! (`model::gmail::parse_gmail_census`, source `gmail_contacts`), with
//! duplicates accounted for rather than pretended away — while a Google
//! calendar capability is NOT wired: no channel, no mapping, no route treats
//! a Google calendar source as anything but `Other`.
//!
//! Level: L1 Component — the production census parser plus the production
//! channel mapping. No database, no network, no live Google.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_google__002__contacts_calendar_if_currently_wired

use model::comms::{source_channel_for, CommsSourceChannel};
use model::gmail::{header_emails, parse_gmail_census, GMAIL_CONTEXT_SOURCE};

/// A census artifact: one clean row, its duplicate, and one unusable row.
fn artifact() -> String {
    let header = "normalized_email,display,unused,account,first,last,unused,in,out,unused,ic,oc,unused,unused,unused,unused,unused,unused,tw,oi,auto20,auto21,org,unused,unused,unused,unused,unused,note";
    let clean = "alice@example.com,Alice,,acct-1,2026-01-01,2026-09-01,,2026-08-01,2026-09-01,,3,5,,,,,,true,false,false,false,false,,,,,,";
    let duplicate = "ALICE@example.com,Alice A,,acct-1,2026-01-01,2026-09-01,,2026-08-01,2026-09-01,,3,5,,,,,,true,false,false,false,false,,,,,,";
    let unusable = "not-an-email,Nobody,,acct-1,,,,,,,,,,,,,,,,,,,,,,,";
    [header, clean, duplicate, unusable].join("\n")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn int_google_002__contacts_calendar_if_currently_wired() {
    // Positive (contacts): the census artifact parses — contacts are wired.
    let batch = parse_gmail_census(&artifact());
    assert_eq!(
        batch.declared, 3,
        "the header names columns, not a correspondent"
    );
    assert_eq!(batch.accepted, 1, "one clean correspondent is accepted");
    assert_eq!(
        batch.deduplicated, 1,
        "the repeated correspondent is deduplicated by (account, identity)"
    );
    assert_eq!(batch.rejected, 1, "the unusable row is rejected, not lost");
    assert!(batch.balances(), "every declared row is accounted for");
    assert_eq!(batch.rows.len(), 1);
    assert_eq!(
        batch.rows[0].evidence.source, GMAIL_CONTEXT_SOURCE,
        "accepted evidence carries the Gmail contacts source"
    );
    assert_eq!(GMAIL_CONTEXT_SOURCE, "gmail_contacts");
    assert_eq!(
        source_channel_for(GMAIL_CONTEXT_SOURCE),
        CommsSourceChannel::Email,
        "Gmail contacts file under Email in the comms boundary"
    );

    // Positive (contacts): correspondent addresses normalize and dedupe in
    // order — the same person written twice is one identity.
    assert_eq!(
        header_emails(Some("Bob <b@x.com>, B@X.com")),
        vec!["b@x.com".to_owned()],
        "the same address in two spellings is one normalized identity"
    );
    assert!(header_emails(None).is_empty());

    // Pinned absence (calendar): no Google calendar capability is wired. A
    // Google calendar source maps to Other — it is not mistaken for the Apple
    // Calendar channel, and it is not silently treated as email either.
    for source in ["google_calendar", "google", "gcal"] {
        assert_eq!(
            source_channel_for(source),
            CommsSourceChannel::Other,
            "{source} is not a wired calendar capability"
        );
    }
}
