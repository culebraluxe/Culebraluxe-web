//! DOCS.FORMS.EXECUTION — applied signature (TST-DOCS-FORMS-EXECUTION-004).
//!
//! Contract: a locally applied signature is provenance, never bytes-in-a-row. The pure half
//! (`model::forms_applied_signature`, `middle/model/src/forms_applied_signature.rs`) strictly recovers
//! execution-slot evidence from an issued snapshot — a checksum, a valid instant, the canonical consent
//! basis and real geometry, or the entry is ignored and can never satisfy a slot — and prints the issuance
//! boundary in the brokerage's own Puerto Rico day. The composition half (`web::vault::forms_render`,
//! `render_form` in `web/src/vault/forms_render/draw_overview.rs`) applies the signature to the slot it
//! belongs to: that slot's provider anchors disappear (a signed line is never offered to the envelope) and
//! the document records the evidence instead — while a signature for a role the template never declares
//! refuses to render at all.
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The composer IS the production composition seam
//! and runs deterministically in-process; no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__004__applied_signature

use std::collections::BTreeMap;

use model::forms_applied_signature::{
    format_broker_initials, format_broker_signature_date, parse_applied_signature_slot_ids,
    AppliedSignatureImageMimeType, FormAppliedSignature, BROKER_SIGNATURE_CONSENT_BASIS,
    BROKER_SIGNATURE_DATE_SEMANTIC,
};
use model::forms_template::TemplateLibrary;
use serde_json::json;
use test_harness::source;
use web::vault::forms_render::{render_form, Participant};

fn proven_evidence() -> serde_json::Value {
    json!({
        "role": "SELLER_BROKER",
        "slotId": "SELLER_BROKER:1",
        "signerName": "Lisa Penfield",
        "credentialLine": "Real Estate Broker License #: C-9931",
        "signerAppUserId": "user-1",
        "assetMediaId": "media-1",
        "assetChecksumSha256": "a".repeat(64),
        "appliedAt": "2026-07-03T12:00:00Z",
        "consentBasis": BROKER_SIGNATURE_CONSENT_BASIS,
        "dateSemantic": BROKER_SIGNATURE_DATE_SEMANTIC,
        "renderedDate": "July 3, 2026",
        "pageIndex": 1,
        "signatureRect": { "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 },
        "dateRect": { "x": 5.0, "y": 6.0, "width": 7.0, "height": 8.0 }
    })
}

/// A signature for the composer: the brokerage's standing pre-signature on the slot issuance assigns her.
/// The image is the repository's own brand PNG — a real decodable asset, so the soft-mask path runs.
fn applied_broker_signature(image_bytes: Vec<u8>, slot_id: &str) -> FormAppliedSignature {
    FormAppliedSignature {
        role: "SELLER_BROKER".to_string(),
        slot_id: Some(slot_id.to_string()),
        signer_name: "Lisa Penfield".to_string(),
        credential_line: "Real Estate Broker License #: C-9931".to_string(),
        signer_app_user_id: "user-1".to_string(),
        image_bytes,
        image_mime_type: AppliedSignatureImageMimeType::Png,
        asset_media_id: "media-1".to_string(),
        asset_checksum_sha256: "a".repeat(64),
        applied_at: "2026-07-04T01:30:00Z".to_string(),
        consent_basis: BROKER_SIGNATURE_CONSENT_BASIS.to_string(),
        date_semantic: BROKER_SIGNATURE_DATE_SEMANTIC.to_string(),
    }
}

fn listing_values() -> BTreeMap<String, String> {
    [
        ("sellerName", "Ana Seller"),
        ("brokerName", "Lisa Penfield"),
        ("listPrice", "1250000"),
        ("commission", "4%"),
        ("startDate", "2026-01-15"),
        ("endDate", "2027-01-15"),
        ("listingType", "Exclusive Right to Sell"),
    ]
    .iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-004); the file and the assay use it.
fn docs_forms_execution_004__applied_signature() {
    let root = source::workspace_root();

    // 1. STRICT EVIDENCE RECOVERY (production code, executed): only proven evidence satisfies a slot.
    assert_eq!(
        parse_applied_signature_slot_ids(&json!([proven_evidence()])),
        vec!["SELLER_BROKER:1".to_string()],
        "proven evidence recovers its slot"
    );
    // The same slot twice is one satisfaction, not two.
    assert_eq!(
        parse_applied_signature_slot_ids(&json!([proven_evidence(), proven_evidence()])),
        vec!["SELLER_BROKER:1".to_string()]
    );
    // NEGATIVE: each broken shape is ignored — never repaired into a satisfaction.
    for (name, mutate) in [
        (
            "a checksum that is not 64 hex chars",
            Box::new(|value: &mut serde_json::Value| {
                value["assetChecksumSha256"] = json!("not-a-checksum");
            }) as Box<dyn Fn(&mut serde_json::Value)>,
        ),
        (
            "a slot id that does not belong to its role",
            Box::new(|value: &mut serde_json::Value| {
                value["slotId"] = json!("BUYER_BROKER:1");
            }),
        ),
        (
            "a consent basis that is not the canonical one",
            Box::new(|value: &mut serde_json::Value| {
                value["consentBasis"] = json!("verbal");
            }),
        ),
        (
            "a date semantic that is not issuance-requested-at",
            Box::new(|value: &mut serde_json::Value| {
                value["dateSemantic"] = json!("signed-at");
            }),
        ),
        (
            "geometry that is not a real rectangle",
            Box::new(|value: &mut serde_json::Value| {
                value["signatureRect"] = json!({ "x": 1.0, "y": 2.0, "width": 0.0, "height": 4.0 });
            }),
        ),
        (
            "an appliedAt that is not a valid instant",
            Box::new(|value: &mut serde_json::Value| {
                value["appliedAt"] = json!("last Tuesday");
            }),
        ),
    ] {
        let mut broken = proven_evidence();
        mutate(&mut broken);
        assert!(
            parse_applied_signature_slot_ids(&json!([broken])).is_empty(),
            "{name} must never satisfy a slot"
        );
    }
    assert!(parse_applied_signature_slot_ids(&json!("nonsense")).is_empty());
    assert!(parse_applied_signature_slot_ids(&json!(null)).is_empty());

    // 2. THE PRINTED BOUNDARY: the issuance date is the brokerage's own Puerto Rico day, and the initials
    //    are deterministic.
    assert_eq!(
        format_broker_signature_date("2026-07-04T01:30:00Z").expect("a valid instant"),
        "July 3, 2026",
        "01:30 UTC on the 4th is 21:30 AST on the 3rd — the brokerage's day, not UTC's"
    );
    assert!(format_broker_signature_date("not an instant").is_err());
    assert_eq!(format_broker_initials("Lisa Penfield"), "LP");
    assert_eq!(format_broker_initials("Lisa"), "LI");
    assert_eq!(format_broker_initials("  "), "");
    // Only the two image formats the protected store accepts.
    assert_eq!(
        AppliedSignatureImageMimeType::parse("image/png"),
        Some(AppliedSignatureImageMimeType::Png)
    );
    assert_eq!(
        AppliedSignatureImageMimeType::parse(" IMAGE/JPEG "),
        Some(AppliedSignatureImageMimeType::Jpeg)
    );
    assert_eq!(AppliedSignatureImageMimeType::parse("image/gif"), None);

    // 3. THE COMPOSITION (production composer, executed): with her signature applied to the slot issuance
    //    assigns her, that slot's provider marks disappear and the document records the provenance instead.
    let library = TemplateLibrary::load_from_dir(&root.join("middle/model/forms/templates"))
        .expect("the repository's templates load");
    let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
    let image = std::fs::read(root.join("public/brand/CLLOGO.png")).expect("the brand PNG reads");
    let values = listing_values();
    let participants = vec![
        Participant {
            role: "SELLER".to_string(),
            slot_id: Some("SELLER:1".to_string()),
            name: "Ana Seller".to_string(),
        },
        Participant {
            role: "SELLER_BROKER".to_string(),
            slot_id: Some("SELLER_BROKER:1".to_string()),
            name: "Lisa Penfield".to_string(),
        },
    ];
    let unsigned = render_form(
        template,
        &values,
        &BTreeMap::new(),
        1,
        &participants,
        None,
        &[],
    )
    .expect("the agreement renders unsigned");
    assert!(
        unsigned
            .signature_anchors
            .iter()
            .any(|anchor| anchor.role == "SELLER_BROKER"),
        "an unsigned broker slot waits for the provider"
    );
    assert!(unsigned.applied_evidence.is_empty());

    let applied = applied_broker_signature(image.clone(), "SELLER_BROKER:1");
    let signed = render_form(
        template,
        &values,
        &BTreeMap::new(),
        1,
        &participants,
        None,
        std::slice::from_ref(&applied),
    )
    .expect("the agreement renders with her signature on it");
    assert!(
        !signed
            .signature_anchors
            .iter()
            .any(|anchor| anchor.role == "SELLER_BROKER"),
        "a slot she has signed is never offered to the provider"
    );
    assert!(
        signed
            .signature_anchors
            .iter()
            .any(|anchor| anchor.role == "SELLER"),
        "the seller's own slot still waits for the provider"
    );
    assert_eq!(signed.applied_evidence.len(), 1);
    let evidence = &signed.applied_evidence[0];
    assert_eq!(evidence.slot_id.as_deref(), Some("SELLER_BROKER:1"));
    assert_eq!(evidence.signer_name, "Lisa Penfield");
    assert_eq!(
        evidence.rendered_date, "July 3, 2026",
        "the issuance instant prints in the brokerage's own day"
    );
    assert_eq!(evidence.rendered_initials.as_deref(), Some("LP"));
    assert!(evidence.signature_rect.width > 0.0 && evidence.date_rect.height > 0.0);
    assert_ne!(
        signed.bytes, unsigned.bytes,
        "a signature that does not change the bytes was never drawn"
    );

    // 4. NEGATIVE (composition): a signature for a role the TEMPLATE never declares refuses to render —
    //    never a signature drawn nowhere.
    let foreign = FormAppliedSignature {
        role: "BUYER_BROKER".to_string(),
        ..applied_broker_signature(image, "SELLER_BROKER:1")
    };
    let error = render_form(
        template,
        &values,
        &BTreeMap::new(),
        1,
        &participants,
        None,
        std::slice::from_ref(&foreign),
    )
    .expect_err("a signature for an undeclared role is refused");
    assert!(
        error.to_string().contains("BUYER_BROKER"),
        "the refusal names the role: {error}"
    );

    // 5. NEGATIVE (composition): with TWO broker signers on the template, a signature whose slot matches
    //    neither satisfies no slot — the exact-match rule never prints a signature on a line it does not
    //    belong to, and the anchors keep waiting for the provider.
    let two_brokers = vec![
        Participant {
            role: "SELLER_BROKER".to_string(),
            slot_id: Some("SELLER_BROKER:1".to_string()),
            name: "Lisa Penfield".to_string(),
        },
        Participant {
            role: "SELLER_BROKER".to_string(),
            slot_id: Some("SELLER_BROKER:2".to_string()),
            name: "Second Broker".to_string(),
        },
    ];
    let unmatched = applied_broker_signature(
        std::fs::read(root.join("public/brand/CLLOGO.png")).expect("the brand PNG reads"),
        "SELLER_BROKER:9",
    );
    let rendered = render_form(
        template,
        &values,
        &BTreeMap::new(),
        1,
        &two_brokers,
        None,
        std::slice::from_ref(&unmatched),
    )
    .expect("the agreement still renders");
    assert!(
        rendered.applied_evidence.is_empty(),
        "a signature that matches no slot is not recorded as evidence"
    );
    assert!(
        rendered
            .signature_anchors
            .iter()
            .filter(|anchor| anchor.role == "SELLER_BROKER")
            .count()
            >= 2,
        "both unmatched slots keep waiting for the provider"
    );
}
