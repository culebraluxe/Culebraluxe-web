//! DOCS.FORMS.TEMPLATE — template parser (TST-DOCS-FORMS-TEMPLATE-001).
//!
//! Contract: the production template parser is the hand-written, dependency-free XML parser in
//! `model::forms_template` (tokenizer/tree in `middle/model/src/forms_template/show_all_value.rs`, the
//! document parser `parse_template_xml` in `middle/model/src/forms_template/parse_section.rs`). It accepts
//! exactly the declared grammar — one `<form id version title>` root with `<field>`, `<section>`,
//! `<participants>` and `<signatures>` children — and refuses everything outside it WITH A REASON, because a
//! template that is wrong must be rejected, never silently rendered as a different document. The library seam
//! (`TemplateLibrary::load_from_dir`) scans a directory of versioned XML files and resolves any `(id, version)`
//! exactly, so a live record keeps rendering under the version it was issued with.
//!
//! Level: L0 Pure — the production parser on in-memory documents and the repository's real authoring
//! directory; no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__001__template_parser

use model::forms_template::{parse_template_xml, TemplateLibrary, TemplatePresentation};
use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-001); the file and the assay use it.
fn docs_forms_template_001__template_parser() {
    // 1. A complete document parses: the root attributes, one child of every kind, and the defaults the
    //    grammar declares (documentType defaults to the title, the issuer to the brokerage, the presentation
    //    to a report).
    let template = parse_template_xml(
        r#"<form id="T" version="2" title="Test Agreement">
             <field id="listPrice" label="Asking Price" type="money" required="true"/>
             <section id="partnership" title="Partnership">Hello <value field="listPrice"/></section>
             <participants><participant role="SELLER" multiple="true"/></participants>
             <signatures><signature-group role="SELLER" field="listPrice" initials="true"/></signatures>
           </form>"#,
    )
    .expect("a complete template parses");
    assert_eq!(template.id, "T");
    assert_eq!(template.version, 2);
    assert_eq!(template.document_type_label, "Test Agreement");
    assert_eq!(template.rendering.issuer, "CulebraLuxe Real Estate");
    assert_eq!(
        template.rendering.presentation,
        TemplatePresentation::Report
    );
    assert_eq!(template.fields.len(), 1);
    assert_eq!(template.sections.len(), 1);
    assert_eq!(template.participants.len(), 1);
    assert_eq!(template.signature_groups.len(), 1);

    // An explicit documentType, issuer and presentation win over the defaults.
    let explicit = parse_template_xml(
        r#"<form id="T" version="1" title="T" documentType="Listing Agreement" issuer="Culebraluxe Realty" presentation="agreement">
             <field id="a" label="A" type="text"/>
           </form>"#,
    )
    .expect("explicit rendering attributes parse");
    assert_eq!(explicit.document_type_label, "Listing Agreement");
    assert_eq!(explicit.rendering.issuer, "Culebraluxe Realty");
    assert_eq!(
        explicit.rendering.presentation,
        TemplatePresentation::Agreement
    );

    // 2. NEGATIVE/REFUSAL CASES — every way the document can be wrong is refused with a reason, so no
    //    malformed template can reach the composer. Each expected fragment names the contract's own words.
    let refusals: [(&str, &str); 9] = [
        // A foreign root element.
        (
            r#"<letter id="T" version="1" title="T"/>"#,
            "root must be <form>",
        ),
        // A version that is not a number.
        (
            r#"<form id="T" version="abc" title="T"><field id="a" label="A" type="text"/></form>"#,
            "version must be a positive integer",
        ),
        // Version zero is not positive.
        (
            r#"<form id="T" version="0" title="T"><field id="a" label="A" type="text"/></form>"#,
            "version must be a positive integer",
        ),
        // The title is required.
        (
            r#"<form id="T" version="1"><field id="a" label="A" type="text"/></form>"#,
            "requires attribute \"title\"",
        ),
        // An unknown element inside <form>.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/><bogus/></form>"#,
            "Unknown element",
        ),
        // Free text directly under <form> is not content the grammar owns.
        (
            r#"<form id="T" version="1" title="T">words<field id="a" label="A" type="text"/></form>"#,
            "Unexpected text content directly under <form>",
        ),
        // A template with no fields declares nothing to fill.
        (
            r#"<form id="T" version="1" title="T"/>"#,
            "at least one <field>",
        ),
        // An unclosed document.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/>"#,
            "",
        ),
        // Two roots.
        (
            r#"<form id="T" version="1" title="T"><field id="a" label="A" type="text"/></form><form id="U" version="1" title="U"><field id="b" label="B" type="text"/></form>"#,
            "multiple root",
        ),
    ];
    for (xml, expected) in refusals {
        let error = parse_template_xml(xml).expect_err("the document must be refused");
        assert!(
            error.message.contains(expected),
            "expected a refusal mentioning \"{expected}\", got \"{}\"",
            error.message
        );
    }

    // 3. THE LIBRARY SEAM, on the repository's real authoring directory: every authored file parses, the
    //    families resolve, an exact version is found, an unknown version is NOT (a record pointing at a
    //    missing version must surface, never silently re-render under another).
    let root = source::workspace_root();
    let library = TemplateLibrary::load_from_dir(&root.join("middle/model/forms/templates"))
        .expect("the repository's templates load");
    assert!(
        library.all().len() >= 9,
        "expected the nine authored versions, found {}",
        library.all().len()
    );
    for family in [
        "OFFER-01",
        "LISTING-01",
        "PR-PNS",
        "PR-PNS-AMD",
        "SHOW-INFO",
        "SHOW-RPT",
    ] {
        assert!(
            library.families().iter().any(|(id, _)| *id == family),
            "template family {family} is missing from the directory"
        );
    }
    let listing = library
        .version("LISTING-01", 4)
        .expect("LISTING-01 v4 resolves exactly");
    assert_eq!(listing.version, 4);
    assert!(
        library.version("LISTING-01", 99).is_none(),
        "an unknown version must NOT resolve"
    );
    // `newest` is the HEAD of the family in `middle/model/forms/templates`, which is v5 today — the
    // assertion pinned v4 while the directory had already moved on, so it failed for a reason that was
    // never about the parser (2026-10-08: found when this case was first executed). Pin the real head
    // rather than dropping the claim: `newest` must be the highest version present, not the highest the
    // case happens to name.
    assert_eq!(library.newest("LISTING-01").map(|t| t.version), Some(5));

    // 4. NEGATIVE: a directory that does not exist is an error naming the path, never an empty library a
    //    render would silently accept.
    let missing =
        TemplateLibrary::load_from_dir(&root.join("middle/model/forms/no-such-directory"));
    let error = missing.expect_err("a missing directory is refused");
    assert!(
        error.message.contains("Cannot read the template directory"),
        "the refusal names the directory: {}",
        error.message
    );
}
