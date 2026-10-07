//! The completion certificate: the human-readable signed record.
//!
//! A signing purist would overlay strokes onto the original PDF bytes. This
//! crate cannot do that yet — parsing a PDF needs a parser this tree does
//! not carry — so the completion record is what DocuSign itself ships beside
//! the document: a certificate page stating WHAT was signed, by WHOM, WHEN,
//! with WHICH consent, drawn with the same `pdf` primitives as the Forms
//! pipeline. The bytes are deterministic for identical inputs (the timestamp
//! travels in, never `now()`), so a future cryptography layer can seal them.

use crate::vault::pdf::{Content, Pdf, Rgb};
use db::{FinalizeEvent, FinalizeField, FinalizeRecipient};
use model::forms_font::{encode, text_width, StandardFont};

const PAGE_WIDTH: f64 = 612.0;
const PAGE_HEIGHT: f64 = 792.0;
const MARGIN: f64 = 54.0;
const CONTENT_WIDTH: f64 = PAGE_WIDTH - 2.0 * MARGIN;
const BOTTOM: f64 = 64.0;

const NAVY: Rgb = Rgb::from_bytes(3, 15, 35);
const GOLD: Rgb = Rgb::from_bytes(198, 161, 91);
const BODY: Rgb = Rgb::from_bytes(45, 55, 75);
const FAINT: Rgb = Rgb::from_bytes(120, 130, 150);

const SERIF: &str = "FSerif";
const SERIF_BOLD: &str = "FSerifBold";
const SANS: &str = "FSans";
const SANS_BOLD: &str = "FSansBold";

/// A character the base-14 fonts cannot draw becomes `?`: the record must
/// never fail on a name, and a visible placeholder beats silent loss.
fn sanitized(text: &str) -> String {
    text.chars()
        .map(|character| {
            if encode(&character.to_string()).is_ok() {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn codes(text: &str) -> Vec<u8> {
    encode(&sanitized(text)).unwrap_or_default()
}

fn wrapped(font: StandardFont, text: &str, size: f64, max_width: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in sanitized(text).split_whitespace() {
        let trial = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if text_width(font, &trial, size) <= max_width {
            current = trial;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct Pages {
    pages: Vec<Content>,
    cursor: f64,
}

impl Pages {
    fn new() -> Self {
        Self {
            pages: vec![Content::new()],
            cursor: 716.0,
        }
    }

    fn bump(&mut self, height: f64) -> f64 {
        if self.cursor - height < BOTTOM {
            self.pages.push(Content::new());
            self.cursor = 716.0;
        }
        self.cursor -= height;
        self.cursor
    }

    fn current(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page is always open")
    }

    fn rule(&mut self, color: Rgb) {
        let y = self.bump(14.0);
        self.current().line(
            (MARGIN, y + 10.0),
            (MARGIN + CONTENT_WIDTH, y + 10.0),
            1.0,
            color,
        );
    }

    fn heading(&mut self, text: &str) {
        let y = self.bump(30.0);
        self.current()
            .text(SERIF, 22.0, MARGIN, y, NAVY, &codes(text));
    }

    fn section(&mut self, text: &str) {
        let y = self.bump(30.0);
        self.current()
            .text(SANS_BOLD, 9.0, MARGIN, y, GOLD, &codes(text));
    }

    fn body(&mut self, font: &str, size: f64, color: Rgb, text: &str) {
        let standard = if font == SANS_BOLD {
            StandardFont::HelveticaBold
        } else {
            StandardFont::Helvetica
        };
        for line in wrapped(standard, text, size, CONTENT_WIDTH) {
            let y = self.bump(size + 5.0);
            self.current()
                .text(font, size, MARGIN, y, color, &codes(&line));
        }
    }
}

fn value_text(value: &Option<serde_json::Value>) -> String {
    match value {
        None => "—".into(),
        Some(serde_json::Value::String(text)) => {
            let trimmed: String = text.chars().take(140).collect();
            if text.chars().count() > 140 {
                format!("{trimmed}…")
            } else {
                trimmed
            }
        }
        Some(other) => {
            let text = other.to_string();
            let trimmed: String = text.chars().take(140).collect();
            if text.chars().count() > 140 {
                format!("{trimmed}…")
            } else {
                trimmed
            }
        }
    }
}

/// Draw the completion record. `finalized_at` is RFC3339 text carried in, so
/// identical envelopes render identical bytes.
pub fn render_completion_certificate(
    signature_request_id: &str,
    transaction_document_id: &str,
    recipients: &[FinalizeRecipient],
    fields: &[FinalizeField],
    events: &[FinalizeEvent],
    finalized_at: &str,
) -> Result<Vec<u8>, String> {
    let mut pdf = Pdf::new();
    let serif = pdf.font("Times-Roman");
    let serif_bold = pdf.font("Times-Bold");
    let sans = pdf.font("Helvetica");
    let sans_bold = pdf.font("Helvetica-Bold");
    let page_tree = pdf.reserve();
    let resources = pdf.dictionary(&crate::vault::pdf::resources(
        &[
            (SERIF, serif),
            (SERIF_BOLD, serif_bold),
            (SANS, sans),
            (SANS_BOLD, sans_bold),
        ],
        &[],
    ));

    let mut pages = Pages::new();
    pages.rule(GOLD);
    pages.heading("Certificate of Completion");
    pages.body(
        SANS,
        10.0,
        BODY,
        "CulebraLuxe native signing · signature-audit-trail-v1",
    );
    pages.body(SANS, 9.0, FAINT, &format!("Request {signature_request_id}"));
    pages.body(
        SANS,
        9.0,
        FAINT,
        &format!("Document {transaction_document_id}"),
    );
    pages.body(SANS, 9.0, FAINT, &format!("Completed {finalized_at}"));

    pages.section("RECIPIENTS");
    for recipient in recipients {
        pages.body(
            SANS_BOLD,
            11.0,
            NAVY,
            &format!("{} · {}", recipient.name, recipient.email),
        );
        pages.body(
            SANS,
            9.0,
            BODY,
            &format!(
                "{} · order {} · step {} · {}",
                recipient.role, recipient.signer_order, recipient.signing_step, recipient.state
            ),
        );
        match (
            &recipient.consent_version,
            &recipient.consent_sha256,
            &recipient.consent_accepted_at,
        ) {
            (Some(version), Some(sha), Some(at)) => {
                let short: String = sha.chars().take(16).collect();
                pages.body(
                    SANS,
                    8.5,
                    FAINT,
                    &format!("Consent {version} · sha256 {short}… · {at}"),
                );
            }
            _ => pages.body(SANS, 8.5, FAINT, "No consent recorded."),
        }
    }

    pages.section("FIELDS");
    for field in fields {
        pages.body(
            SANS_BOLD,
            10.0,
            NAVY,
            &format!(
                "{} ({})",
                field.label.as_deref().unwrap_or(&field.field_key),
                field.field_type
            ),
        );
        pages.body(
            SANS,
            9.0,
            BODY,
            &format!(
                "Page {} · owner {} · {} · {}",
                field.page_number,
                short_id(&field.recipient_id),
                if field.required {
                    "required"
                } else {
                    "optional"
                },
                value_text(&field.value),
            ),
        );
        if let Some(at) = &field.completed_at {
            pages.body(SANS, 8.5, FAINT, &format!("Completed {at}"));
        }
    }

    pages.section("EVENT LEDGER");
    for event in events {
        pages.body(
            SANS,
            9.0,
            BODY,
            &format!(
                "{} · {}{}",
                event.occurred_at,
                event.event_type,
                event
                    .actor_id
                    .as_deref()
                    .map(|actor| format!(" · {actor}"))
                    .unwrap_or_default(),
            ),
        );
    }

    let mut page_refs = Vec::with_capacity(pages.pages.len());
    for content in pages.pages {
        page_refs.push(
            pdf.page(
                PAGE_WIDTH,
                PAGE_HEIGHT,
                page_tree,
                resources,
                &content.into_bytes(),
            )
            .map_err(|error| error.to_string())?,
        );
    }
    let info = pdf.info(
        "Certificate of Completion",
        "CulebraLuxe",
        "Native signing completion record",
        "CulebraLuxe Signing",
        "CulebraLuxe Signing",
        &pdf_date(finalized_at),
    );
    pdf.finish(page_tree, &page_refs, Some(info))
        .map_err(|error| error.to_string())
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// A PDF date string from RFC3339 text; falls back to a fixed stamp rather
/// than failing the record on a malformed clock reading.
fn pdf_date(rfc3339: &str) -> String {
    let digits: String = rfc3339.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 14 {
        format!("D:{}", &digits[..14])
    } else {
        "D:20260101000000".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::{FinalizeEvent, FinalizeField, FinalizeRecipient};

    fn inputs() -> (
        Vec<FinalizeRecipient>,
        Vec<FinalizeField>,
        Vec<FinalizeEvent>,
    ) {
        (
            vec![FinalizeRecipient {
                id: "r1".into(),
                name: "María Rivera".into(),
                email: "ada@example.test".into(),
                role: "signer".into(),
                signer_order: 1,
                signing_step: 1,
                state: "completed".into(),
                consent_version: Some("v1".into()),
                consent_sha256: Some("abc".into()),
                consent_accepted_at: Some("2026-01-01T00:00:00+00:00".into()),
            }],
            vec![FinalizeField {
                field_key: "sig-a".into(),
                field_type: "signature".into(),
                label: None,
                page_number: 1,
                recipient_id: "r1".into(),
                required: true,
                position_x: 10.0,
                position_y: 10.0,
                width: 30.0,
                height: 10.0,
                value: Some(serde_json::json!({"signature": "x"})),
                completed_at: Some("2026-01-02T00:00:00+00:00".into()),
            }],
            vec![FinalizeEvent {
                event_type: "recipient_completed".into(),
                occurred_at: "2026-01-02T00:00:00+00:00".into(),
                actor_id: None,
                evidence: serde_json::json!({}),
            }],
        )
    }

    #[test]
    fn certificate_is_a_pdf_and_deterministic() {
        let (recipients, fields, events) = inputs();
        let first = render_completion_certificate(
            "req-1",
            "doc-1",
            &recipients,
            &fields,
            &events,
            "2026-01-03T00:00:00+00:00",
        )
        .expect("renders");
        assert!(first.starts_with(b"%PDF-1.4"), "a real PDF document");
        let second = render_completion_certificate(
            "req-1",
            "doc-1",
            &recipients,
            &fields,
            &events,
            "2026-01-03T00:00:00+00:00",
        )
        .expect("renders");
        assert_eq!(first, second, "identical inputs render identical bytes");
    }

    /// The reader's verdict, borrowed from `pdf.rs`: a structurally plausible
    /// file that QuickLook cannot render is not a document.
    #[test]
    #[cfg(target_os = "macos")]
    fn quicklook_renders_the_certificate() {
        let (recipients, fields, events) = inputs();
        let bytes = render_completion_certificate(
            "req-1",
            "doc-1",
            &recipients,
            &fields,
            &events,
            "2026-01-03T00:00:00+00:00",
        )
        .expect("renders");
        let directory = std::env::temp_dir().join(format!("cl-cert-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temp directory");
        let path = directory.join("certificate.pdf");
        std::fs::write(&path, bytes).expect("the certificate is written");
        let output = std::process::Command::new("/usr/bin/qlmanage")
            .args(["-t", "-s", "400", "-o"])
            .arg(&directory)
            .arg(&path)
            .output()
            .expect("qlmanage runs");
        let rendered = directory.join("certificate.pdf.png");
        assert!(
            rendered.exists(),
            "QuickLook could not render the certificate (status {:?})",
            output.status.code()
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
