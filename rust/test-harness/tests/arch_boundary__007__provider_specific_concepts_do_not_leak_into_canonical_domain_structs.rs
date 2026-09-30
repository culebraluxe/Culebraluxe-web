//! ARCH.BOUNDARY — provider-specific concepts do not leak into canonical domain structs (TST-ARCH-BOUNDARY-007).
//!
//! Contract: `rust/core/domain` holds the canonical records — `Property`, `Media`, `Person`, `Calendar`, `Wbs` and
//! their neighbours — in provider-neutral terms. A provider's name belongs at the edge (`rust/integrations`, which
//! owns the transports), not inside the struct every other layer reads.
//!
//! The honest state of the tree today is that this contract is **partly violated**, and this test says so instead of
//! passing quietly. Reading every canonical module (`rust/core/domain/src/*.rs` bar the six provider-named ones)
//! finds **13** declarations that name a provider:
//!
//!   - `calendar.rs:47,59` — `CreateAppleCalendarEventRequest`, `UpdateAppleCalendarEventRequest`
//!   - `media.rs:17,18,51,52,65,66` and `public_listing.rs:71` — `mux_asset_id` / `mux_playback_id`
//!   - `relationship_evidence.rs:27` — `known_apple_contact`
//!   - `wbs.rs:217,228,235` — `AppleReminderUpsertRequest`, `AppleReminderCommandReceipt`, `AppleReminderLanding`
//!
//! Those are recorded in [`BASELINE`] as **debt, not blessing**: the test is a ratchet in both directions. A new
//! provider token in a canonical struct fails (that is the leak the contract exists to stop), and an entry that is no
//! longer found fails too (so the baseline may only shrink, never quietly widen). The wanted change is a
//! provider-neutral shape — `Playback { asset_id, playback_id }`, `ReminderUpsertRequest` — with the Mux/Apple names
//! kept in `rust/integrations`; the taxonomy row stays open until that is done.
//!
//! What this test does not cover, stated so nobody reads more into a green run: function names, string literals and
//! behaviour (`if provider == …`) are out of scope — the subject is the *shape* of the canonical structs.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test arch_boundary__007__provider_specific_concepts_do_not_leak_into_canonical_domain_structs

use std::path::Path;

use test_harness::source;

/// Provider and delivery-platform names that must not name a field or type of a canonical domain record.
const PROVIDER_TOKENS: [&str; 28] = [
    "mux",
    "apple",
    "gmail",
    "google",
    "whatsapp",
    "boldsign",
    "docusign",
    "dropbox",
    "neon",
    "vercel",
    "ofx",
    "qbo",
    "quickbooks",
    "twilio",
    "stripe",
    "openai",
    "anthropic",
    "icloud",
    "imap",
    "smtp",
    "zoom",
    "slack",
    "notion",
    "airtable",
    "mailchimp",
    "sendgrid",
    "plaid",
    "xero",
];

/// The provider-named modules the domain already keeps, as the *first* path component under `core/domain/src`.
///
/// A module named after a provider is a provider adapter living inside the domain. The list is pinned so a seventh
/// one cannot appear unnoticed, and so removing one is a deliberate edit here rather than a silent drift: a module
/// that no longer exists fails the "every pin still exists" half of the test.
const PROVIDER_MODULES: [&str; 6] = [
    "apple_calls.rs",
    "apple_messages",
    "apple_messages.rs",
    "applemail",
    "applemail.rs",
    "gmail.rs",
];

/// Every distinct provider-named declaration that exists today, as `module kind identifier`. Debt, not a blessing.
///
/// The 13 declaration sites found in the tree collapse to 9 entries because a shared struct shape repeats a field
/// name across three of `media.rs`'s records; the test compares the *set*, so a second record reusing `mux_asset_id`
/// is not a new leak while a new name in a canonical struct is.
const BASELINE: [&str; 9] = [
    "calendar.rs type CreateAppleCalendarEventRequest",
    "calendar.rs type UpdateAppleCalendarEventRequest",
    "media.rs field mux_asset_id",
    "media.rs field mux_playback_id",
    "public_listing.rs field mux_playback_id",
    "relationship_evidence.rs field known_apple_contact",
    "wbs.rs type AppleReminderCommandReceipt",
    "wbs.rs type AppleReminderLanding",
    "wbs.rs type AppleReminderUpsertRequest",
];

/// The provider token an identifier names, if any.
///
/// Boundaries are `_` and a lower-to-upper case change, so `AppleMessagesHandle` and `mux_asset_id` both name a
/// provider while `Pineapple`, `applesauce` and `media` do not.
fn provider_token(identifier: &str) -> Option<String> {
    for part in identifier.split('_') {
        let mut segment = String::new();
        let mut previous_lowercase = false;
        for character in part.chars() {
            if character.is_uppercase() && previous_lowercase && !segment.is_empty() {
                if let Some(token) = named_provider(&segment) {
                    return Some(token);
                }
                segment.clear();
            }
            previous_lowercase = character.is_lowercase() || character.is_numeric();
            segment.push(character);
        }
        if let Some(token) = named_provider(&segment) {
            return Some(token);
        }
    }
    None
}

/// The token `segment` spells, ignoring case — `MUX` and `mux` are the same provider.
fn named_provider(segment: &str) -> Option<String> {
    let lowered = segment.to_lowercase();
    PROVIDER_TOKENS
        .iter()
        .find(|token| **token == lowered)
        .map(|token| (*token).to_string())
}

/// A provider-named declaration on this line, as (`type`|`field`, identifier).
///
/// A declaration, not a mention: `pub mux_asset_id: Option<String>,` and `pub struct AppleReminderLanding {` are
/// findings; `let id = mux_asset_id;`, `pub fn mux_client()`, a comment or a `use` are not.
fn provider_declaration(line: &str) -> Option<(&'static str, String)> {
    let code = source::code_of(line);
    let after_pub = code.trim_start().strip_prefix("pub")?;
    let rest = if let Some(after_open) = after_pub.strip_prefix('(') {
        // `pub(crate) field: …` — the declaration starts after the closing parenthesis.
        after_open.split_once(')')?.1.trim_start()
    } else {
        // `pub field: …` — an identifier that merely begins with `pub` (say `publication`) is not a declaration.
        after_pub.strip_prefix(' ')?.trim_start()
    };

    if let Some(item) = rest
        .strip_prefix("struct ")
        .or_else(|| rest.strip_prefix("enum "))
    {
        let identifier = item
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .next()
            .unwrap_or("");
        return provider_token(identifier).map(|_| ("type", identifier.to_string()));
    }

    // A field: an identifier, then a colon, with no call, tuple or assignment between them.
    let (before_colon, _after_colon) = rest.split_once(':')?;
    if before_colon.contains('(') || before_colon.contains('=') || before_colon.contains(' ') {
        return None;
    }
    provider_token(before_colon).map(|_| ("field", before_colon.to_string()))
}

/// Whether a *module* is named after a provider: `apple_messages.rs`, `applemail` and `gmail.rs` all are.
///
/// Substring, not [`provider_token`], and the difference is deliberate: a module name is prose about the thing
/// (`applemail` is Apple's mail, `gmail` is the provider), while an identifier is read by the code around it, so
/// there a token must own a whole `_`/case-delimited segment — otherwise `Pineapple` would be a finding.
fn module_names_provider(module: &str) -> bool {
    let lowered = module.to_lowercase();
    PROVIDER_TOKENS.iter().any(|token| lowered.contains(token))
}

/// The first path component of `path` below `src`, which is how a provider-named module is recognised at any depth.
fn top_component(path: &Path, src: &Path) -> String {
    path.strip_prefix(src)
        .ok()
        .and_then(|rest| rest.components().next())
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-007); the file and the assay use it.
fn arch_boundary_007__provider_specific_concepts_do_not_leak_into_canonical_domain_structs() {
    let src = source::rust_root().join("core/domain/src");
    let files = source::sources_under(&src);
    assert!(
        files.len() >= 40,
        "the domain source tree is the subject of this contract; only {} files were found under {}",
        files.len(),
        source::relative(&src)
    );

    let mut provider_modules: Vec<String> = Vec::new();
    let mut findings: Vec<String> = Vec::new();

    for path in &files {
        let module = top_component(path, &src);
        assert!(
            !module.is_empty(),
            "{} has no path below src",
            source::relative(path)
        );

        if module_names_provider(&module) {
            if !provider_modules.contains(&module) {
                provider_modules.push(module.clone());
            }
            // A provider-named module is where provider records belong; its own fields are not this contract's subject.
            continue;
        }

        let text = source::read(path);
        for (number, line) in text.lines().enumerate() {
            if let Some((kind, identifier)) = provider_declaration(line) {
                findings.push(format!(
                    "{module} {kind} {identifier} ({}:{})",
                    source::relative(path),
                    number + 1
                ));
            }
        }
    }

    // 1. No seventh provider-named module appears, and no pinned module has vanished.
    let mut found_modules = provider_modules;
    found_modules.sort();
    let mut pinned_modules = PROVIDER_MODULES.to_vec();
    pinned_modules.sort();
    assert_eq!(
        found_modules, pinned_modules,
        "the domain's provider-named modules changed; a new one is another leak and a missing one is a stale pin"
    );

    // 2. The canonical modules hold exactly the provider-named declarations already recorded — no more, no less.
    let mut declared: Vec<String> = findings
        .iter()
        .map(|finding| finding.split(" (").next().unwrap_or(finding).to_string())
        .collect();
    declared.sort();
    declared.dedup();
    let mut baseline: Vec<String> = BASELINE.iter().map(|item| item.to_string()).collect();
    baseline.sort();

    let added: Vec<String> = declared
        .iter()
        .filter(|item| !baseline.contains(*item))
        .cloned()
        .collect();
    let removed: Vec<String> = baseline
        .iter()
        .filter(|item| !declared.contains(*item))
        .cloned()
        .collect();
    assert!(
        added.is_empty(),
        "a provider-specific concept leaked into a canonical domain struct; keep the provider's name at the edge \
         (`rust/integrations`) or extend BASELINE with a written reason:\n{}",
        added
            .iter()
            .map(|item| format!("  + {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        removed.is_empty(),
        "BASELINE lists provider-named declarations that are no longer there — the ratchet may only shrink by \
         deleting the entry, never by leaving it stale:\n{}",
        removed
            .iter()
            .map(|item| format!("  - {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // 3. The scan really read the subject: the Mux ids on `Media` are the leak everyone can point at, so finding none
    //    of them would mean the walker or the detector went blind.
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("media.rs field mux_asset_id")),
        "the scan must still see `Media::mux_asset_id`; it found {} declaration(s) in total: {findings:?}",
        findings.len()
    );

    // 4. The negative controls: the detector fires on the violation and stays quiet on a near-miss.
    assert_eq!(
        provider_token("mux_asset_id"),
        Some("mux".to_string()),
        "a snake_case provider field is a finding"
    );
    assert_eq!(
        provider_token("AppleMessagesHandle"),
        Some("apple".to_string()),
        "a CamelCase provider type is a finding"
    );
    assert_eq!(
        provider_token("known_apple_contact"),
        Some("apple".to_string())
    );
    assert_eq!(
        provider_token("Pineapple"),
        None,
        "a word that merely contains a provider's letters is not a finding"
    );
    assert_eq!(provider_token("applesauce"), None);
    assert_eq!(provider_token("PineappleInc"), None);
    assert_eq!(
        provider_token("media"),
        None,
        "`media` is the domain's own noun, not a provider"
    );
    assert_eq!(provider_token("update_media_asset"), None);
    assert!(
        module_names_provider("applemail"),
        "a module name is prose: `applemail` is Apple's mail"
    );
    assert!(module_names_provider("gmail.rs"));
    assert!(module_names_provider("apple_messages.rs"));
    assert!(
        !module_names_provider("media.rs"),
        "`media` is the domain's own noun, not a provider"
    );
    assert!(!module_names_provider("public_listing.rs"));
    assert!(!module_names_provider("relationship_evidence.rs"));
    assert_eq!(
        provider_declaration("    pub mux_stream_key: String,"),
        Some(("field", "mux_stream_key".to_string())),
        "a new provider field must be caught"
    );
    assert_eq!(
        provider_declaration("pub(crate) mux_asset_id: Option<String>,"),
        Some(("field", "mux_asset_id".to_string())),
        "a restricted-visibility field is still a field"
    );
    assert_eq!(
        provider_declaration("    pub struct MuxPlayback {"),
        Some(("type", "MuxPlayback".to_string())),
        "a new provider type must be caught"
    );
    assert_eq!(
        provider_declaration("        let asset = mux_asset_id;"),
        None,
        "a mention is not a declaration"
    );
    assert_eq!(
        provider_declaration("    pub fn mux_client() -> Client {"),
        None,
        "function names are out of scope, as the header says"
    );
    assert_eq!(
        provider_declaration("    // mux_asset_id stays until the row is closed"),
        None,
        "a comment is not code"
    );
}
