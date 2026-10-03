//! The form document composer: a template plus its values become PDF bytes and the signature geometry.
//!
//! PORTED FROM `lib/forms/pdf.ts`, NUMBER FOR NUMBER. That composer is the shipped renderer and this one produces the
//! same document: the same page size, margins, type sizes and colours, the same greedy measured wrap, the same overview
//! grid, section numbering and signature blocks with the same anchor rectangles. Those rectangles are what BoldSign is
//! told to place signatures on, so a difference of a point is a difference in a legal document.
//!
//! WHAT IS NOT HERE: signatures applied after issuance. The TypeScript composer composes already-authorized signature
//! images and records their evidence; that belongs with the signature transport, which is already Rust and has its own
//! story. What is here is what issuance needs — the document, and where the signatures go.

use crate::vault::pdf::{resources, Content, Pdf, PdfError, Rgb};
use model::forms_applied_signature::{
    format_broker_initials, format_broker_signature_date, AppliedSignatureEvidence,
    FormAppliedSignature,
};
use model::forms_font::{encode, text_width, StandardFont};
use model::forms_template::{
    TemplateDefinition, TemplateFieldDefinition, TemplateFieldType, TemplatePresentation,
    TemplateSectionDefinition, TemplateSectionSegment, TemplateSignatureGroup,
};
use std::collections::BTreeMap;
mod draw_overview;
mod page_width;
#[allow(unused_imports)]
pub use draw_overview::*;
#[allow(unused_imports)]
pub use page_width::*;

// ---------------------------------------------------------------- the layout, as the TypeScript composer declares it

#[cfg(test)]
mod tests {
    use super::*;
    use model::forms_template::TemplateLibrary;
    use std::path::Path;

    fn repository_path(relative: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    fn library() -> TemplateLibrary {
        TemplateLibrary::load_from_dir(&repository_path("../middle/model/forms/templates"))
            .expect("the repository's templates load")
    }

    fn logo() -> Option<Logo> {
        // THE WORDMARK IS AT THE REPOSITORY ROOT's `public/`, which is ONE level up from `web` (the crate sits at the
        // top of the web tier, so a single `..` is the root): the extra level this used to climb belonged to `rust/`.
        let bytes = std::fs::read(repository_path("../public/brand/CLLOGO.png")).ok()?;
        Logo::from_png(&bytes)
    }

    fn values() -> BTreeMap<String, String> {
        [
            ("sellerName", "Lisa Penfield"),
            ("sellerCivilStatus", "Single"),
            ("sellerResidenceAddress", "Calle 1, Culebra, PR 00775"),
            ("brokerName", "Lisa Penfield"),
            ("property", "Casa Luar"),
            ("propertyLocation", "Culebra, Puerto Rico"),
            ("catastroNumber", "123-456-789"),
            ("listPrice", "1250000"),
            ("commission", "5%"),
            ("startDate", "2026-01-15"),
            ("endDate", "2027-01-15"),
            ("listingType", "Exclusive Right to Sell"),
        ]
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
    }

    fn participants() -> Vec<Participant> {
        vec![
            Participant {
                role: "SELLER".to_string(),
                slot_id: Some("slot-1".to_string()),
                name: "Seller".to_string(),
            },
            Participant {
                role: "SELLER_BROKER".to_string(),
                slot_id: None,
                name: "Broker".to_string(),
            },
        ]
    }

    #[test]
    fn the_prose_carries_the_values_and_the_section_labels() {
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let body = document_body_text(template, &values(), &BTreeMap::new());
        assert!(
            body.contains("Lisa Penfield"),
            "the seller's name is interpolated"
        );
        assert!(body.contains("Casa Luar"), "the property is interpolated");
        assert!(
            body.contains("January 15, 2026"),
            "a date is formatted, not raw ISO"
        );
        assert!(body.contains("$1,250,000"), "money is formatted");
        assert!(
            body.contains("Parties, Appointment and Partnership"),
            "a section's label leads its block"
        );
    }

    #[test]
    fn an_edited_body_replaces_the_template_only_when_it_is_marked_edited() {
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let mut sections = BTreeMap::new();
        sections.insert("body".to_string(), "Hand-written terms.".to_string());
        assert!(
            document_body_text(template, &values(), &sections).len() > 100,
            "an unmarked body does not override the template"
        );
        sections.insert("bodyEdited".to_string(), "true".to_string());
        assert_eq!(
            resolve_document_body(template, &values(), &sections),
            "Hand-written terms."
        );
    }

    #[test]
    fn money_and_dates_render_the_way_the_repository_formats_them() {
        assert_eq!(format_money("1250000"), "$1,250,000");
        assert_eq!(format_money("1234.5"), "$1,234.5");
        assert_eq!(format_money(""), "");
        assert_eq!(format_date("2026-01-15"), "January 15, 2026");
        assert_eq!(format_date(" 2026-12-03 "), "December 3, 2026");
        assert_eq!(format_date("not a date"), "not a date");
        // Each substitution `pdf_safe` makes, on its own so the expectation cannot be misread.
        assert_eq!(pdf_safe("\u{2019}"), "'");
        assert_eq!(pdf_safe("\u{201c}\u{201d}"), "\"\"");
        assert_eq!(pdf_safe("\u{2013}\u{2014}"), "--");
        assert_eq!(pdf_safe("\u{2026}"), "...");
        assert_eq!(pdf_safe("a\u{a0}b"), "a b");
        assert_eq!(pdf_safe("caf\u{e9}"), "caf\u{e9}");
        assert_eq!(pdf_safe("\u{1f389}"), "?");
        assert_eq!(pdf_safe("party \u{1f389}"), "party ?");
    }

    /// EVERY field answered plausibly, driven by the template itself: this is what proves a template with 34 fields
    /// renders all of them rather than the handful a hand-written fixture would remember.
    fn values_for(template: &TemplateDefinition) -> BTreeMap<String, String> {
        use model::forms_template::TemplateFieldType;
        let mut values = BTreeMap::new();
        for field in &template.fields {
            let value = match field.field_type {
                TemplateFieldType::Money => "1250000".to_string(),
                TemplateFieldType::Date => "2026-01-15".to_string(),
                TemplateFieldType::Select => field
                    .options
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Option".to_string()),
                _ => format!("Sample {}", field.label),
            };
            values.insert(field.name.clone(), value);
        }
        values
    }

    /// One participant per declared signature group, named by the field that group is bound to.
    fn participants_for(
        template: &TemplateDefinition,
        values: &BTreeMap<String, String>,
    ) -> Vec<Participant> {
        template
            .signature_groups
            .iter()
            .map(|group| Participant {
                role: group.role.clone(),
                slot_id: Some(format!("{}:1", group.role)),
                name: group
                    .field
                    .as_ref()
                    .and_then(|name| values.get(name))
                    .cloned()
                    .unwrap_or_else(|| group.label.clone()),
            })
            .collect()
    }

    /// A signature image, encoded here so the test owns its own bytes: opaque AND transparent pixels, so the soft mask is
    /// exercised rather than assumed.
    fn signature_png() -> Vec<u8> {
        let mut image = image::RgbaImage::new(2, 2);
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 1, image::Rgba([200, 200, 200, 0]));
        image.put_pixel(1, 1, image::Rgba([198, 161, 91, 128]));
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("the test's own PNG encodes");
        bytes
    }

    /// A locally applied signature for the test: the broker's own, on the slot issuance would assign her.
    fn applied_broker_signature() -> FormAppliedSignature {
        use model::forms_applied_signature::{
            AppliedSignatureImageMimeType, BROKER_SIGNATURE_CONSENT_BASIS,
            BROKER_SIGNATURE_DATE_SEMANTIC,
        };
        FormAppliedSignature {
            role: "SELLER_BROKER".to_string(),
            slot_id: Some("SELLER_BROKER:1".to_string()),
            signer_name: "Lisa Penfield".to_string(),
            credential_line: "Real Estate Broker License #: C-9931".to_string(),
            signer_app_user_id: "user-1".to_string(),
            image_bytes: signature_png(),
            image_mime_type: AppliedSignatureImageMimeType::Png,
            asset_media_id: "media-1".to_string(),
            asset_checksum_sha256: "a".repeat(64),
            applied_at: "2026-07-04T01:30:00Z".to_string(),
            consent_basis: BROKER_SIGNATURE_CONSENT_BASIS.to_string(),
            date_semantic: BROKER_SIGNATURE_DATE_SEMANTIC.to_string(),
        }
    }

    #[test]
    fn the_purchase_and_sale_agreement_renders_every_field_and_every_mark() {
        let library = library();
        // THE HARDEST TEMPLATE: PR-PNS v3, thirty-four fields and five signature groups, each carrying initials.
        let template = library.version("PR-PNS", 3).expect("PR-PNS v3");
        assert!(
            template.fields.len() >= 30,
            "the fixture is the real template, not a smaller stand-in"
        );
        let values = values_for(template);
        assert_eq!(
            values.len(),
            template.fields.len(),
            "every field is answered, so a field the renderer dropped would be missed"
        );

        let rendered = render_form(
            template,
            &values,
            &BTreeMap::new(),
            1,
            &participants_for(template, &values),
            None,
            &[],
        )
        .expect("the agreement renders");

        // Every declared group is drawn, and draws its own marks: a signature line, its initials line and its date line.
        let roles: std::collections::BTreeSet<&str> = rendered
            .signature_anchors
            .iter()
            .map(|anchor| anchor.role.as_str())
            .collect();
        assert_eq!(roles.len(), template.signature_groups.len());
        let expected_marks: usize = template
            .signature_groups
            .iter()
            .map(|group| if group.initials { 3 } else { 2 })
            .sum();
        assert_eq!(
            rendered.signature_anchors.len(),
            expected_marks,
            "each group draws exactly its signature, initials and date marks"
        );
        assert!(rendered.page_count >= 1);

        // AND WITH HER OWN SIGNATURE APPLIED: that slot's three marks are replaced by the signature and its date, and the
        // document records the provenance instead. Signing the same line twice is the failure this prevents.
        let applied = applied_broker_signature();
        let signed = render_form(
            template,
            &values,
            &BTreeMap::new(),
            1,
            &participants_for(template, &values),
            None,
            std::slice::from_ref(&applied),
        )
        .expect("the agreement renders with her signature on it");

        assert_eq!(
            signed.signature_anchors.len(),
            expected_marks - 3,
            "a slot that is already signed is not offered to the provider"
        );
        assert!(
            !signed
                .signature_anchors
                .iter()
                .any(|anchor| anchor.role == "SELLER_BROKER"),
            "the broker's slot carries no provider marks once she has signed it"
        );
        assert_eq!(signed.applied_evidence.len(), 1);
        let evidence = &signed.applied_evidence[0];
        assert_eq!(evidence.slot_id.as_deref(), Some("SELLER_BROKER:1"));
        assert_eq!(
            evidence.rendered_date, "July 3, 2026",
            "the issuance instant is printed in the brokerage's own day"
        );
        assert_eq!(evidence.rendered_initials.as_deref(), Some("LP"));
        assert!(evidence.signature_rect.width > 0.0 && evidence.date_rect.height > 0.0);
        assert_ne!(
            signed.bytes, rendered.bytes,
            "a signature that does not change the bytes was never drawn"
        );
    }

    /// THE DOCUMENT AS A VIEWER SEES IT. Rectangles draw while every glyph is missing when the page's resources declare
    /// no font, or when the text is drawn in the background's own colour — both look identical to "white text on white".
    #[test]
    fn the_rendered_document_actually_carries_text_and_the_wordmark() {
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let rendered = render_form(
            template,
            &values(),
            &BTreeMap::new(),
            1,
            &participants(),
            logo().as_ref(),
            &[],
        )
        .expect("the agreement renders");
        let pdf = String::from_utf8_lossy(&rendered.bytes).to_string();

        assert!(
            pdf.contains("/Type /Font"),
            "the document carries no font object, so no text can be drawn"
        );
        // THE RESOURCES ARE A DICTIONARY, so they are plain text in the file. A page whose `/Resources` is a stream resolves
        // nothing: the rules draw and every glyph and the wordmark vanish.
        assert!(
            pdf.contains("/Font <<"),
            "the page resources declare no font, so a viewer draws the rules and nothing else"
        );
        assert!(
            pdf.contains("/XObject <<"),
            "the wordmark is not in the page resources, so the header loses it"
        );
        assert!(
            pdf.contains("/Subtype /Image"),
            "the document carries no image object for the wordmark"
        );
        // THE ARTIFACT ITSELF, written where a human and a standard tool can look at it. Counting text operators inside a
        // compressed stream with a hand-written inflater proves nothing about a document; opening it does.
        std::fs::write("/tmp/culebraluxe-form-render.pdf", &rendered.bytes)
            .expect("the rendered document is written for inspection");
        assert!(
            rendered.bytes.len() > 20_000,
            "the document is suspiciously small ({} bytes)",
            rendered.bytes.len()
        );
    }

    #[test]
    fn the_listing_agreement_renders_with_its_signature_geometry() {
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let rendered = render_form(
            template,
            &values(),
            &BTreeMap::new(),
            1,
            &participants(),
            logo().as_ref(),
            &[],
        )
        .expect("the agreement renders");

        assert!(rendered.page_count >= 1);
        let text = String::from_utf8_lossy(&rendered.bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.trim_end().ends_with("%%EOF"));
        assert!(
            text.contains("/BaseFont /Times-Roman") && text.contains("/BaseFont /Helvetica-Bold"),
            "the standard fonts are named rather than embedded"
        );

        // Every signature group records three regions per signer, and every region is on a real page.
        let seller: Vec<&SignatureAnchor> = rendered
            .signature_anchors
            .iter()
            .filter(|anchor| anchor.role == "SELLER")
            .collect();
        assert_eq!(seller.len(), 3, "signature, initials and date");
        for kind in [AnchorKind::Signature, AnchorKind::Initial, AnchorKind::Date] {
            assert!(
                seller.iter().any(|anchor| anchor.kind == kind),
                "{kind:?} is recorded"
            );
        }
        assert!(
            rendered.signature_anchors.len() >= 6,
            "both parties sign the listing agreement"
        );
        for anchor in &rendered.signature_anchors {
            assert_eq!(anchor.page_width, 612.0);
            assert_eq!(anchor.page_height, 792.0);
            assert!(anchor.page_index < rendered.page_count);
            assert!(anchor.rect.width > 0.0 && anchor.rect.height > 0.0);
            assert!(
                anchor.rect.x + anchor.rect.width <= anchor.page_width,
                "an anchor stays inside the page"
            );
            assert!(
                anchor.rect.y + anchor.rect.height <= anchor.page_height,
                "an anchor stays inside the page"
            );
        }
    }

    #[test]
    fn the_rendered_document_is_deterministic() {
        // The issued document's checksum is what makes it evidence: the same input must produce the same bytes.
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let first = render_form(
            template,
            &values(),
            &BTreeMap::new(),
            1,
            &participants(),
            logo().as_ref(),
            &[],
        )
        .expect("renders");
        let second = render_form(
            template,
            &values(),
            &BTreeMap::new(),
            1,
            &participants(),
            logo().as_ref(),
            &[],
        )
        .expect("renders");
        assert_eq!(first.bytes, second.bytes, "the same document twice");
        assert_eq!(first.signature_anchors, second.signature_anchors);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn a_pdf_reader_accepts_the_rendered_agreement() {
        let library = library();
        let template = library.version("LISTING-01", 4).expect("LISTING-01 v4");
        let rendered = render_form(
            template,
            &values(),
            &BTreeMap::new(),
            1,
            &participants(),
            logo().as_ref(),
            &[],
        )
        .expect("renders");
        let directory = std::env::temp_dir().join(format!("cl-form-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temp directory");
        let path = directory.join("agreement.pdf");
        std::fs::write(&path, &rendered.bytes).expect("the document is written");
        let output = std::process::Command::new("/usr/bin/qlmanage")
            .args(["-t", "-s", "600", "-o"])
            .arg(&directory)
            .arg(&path)
            .output()
            .expect("qlmanage runs");
        let thumbnail_path = directory.join("agreement.pdf.png");
        assert!(
            thumbnail_path.exists(),
            "QuickLook could not render the agreement (status {:?})\n{}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let thumbnail = std::fs::metadata(&thumbnail_path).expect("the thumbnail exists");
        assert!(thumbnail.len() > 2000, "the thumbnail has content");
        let _ = std::fs::remove_dir_all(&directory);
    }
}

use unicode_normalization::UnicodeNormalization;
