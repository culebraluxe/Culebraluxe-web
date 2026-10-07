//! The completion certificate: the human-readable signed record.
//!
//! The sealed PDF carries the signatures; this is the page that says WHAT was signed (by fingerprint), by WHOM, WHEN,
//! with WHICH consent, and in what order things happened — the record a signer, a broker or a court can read without
//! the system. It is drawn with the same `pdf` primitives as the Forms pipeline, and its bytes are deterministic for
//! identical inputs (the timestamp travels in, never `now()`), so a future cryptography layer can seal them.

use crate::vault::pdf::{Content, Pdf, Rgb};
use db::{FinalizeEvent, FinalizeField, FinalizeRecipient};
use model::forms_font::{encode, text_width, StandardFont};

const PAGE_WIDTH: f64 = 612.0;
const PAGE_HEIGHT: f64 = 792.0;
const MARGIN: f64 = 54.0;
const CONTENT_WIDTH: f64 = PAGE_WIDTH - 2.0 * MARGIN;
const BOTTOM: f64 = 72.0;
const TOP: f64 = 740.0;
const BAND: f64 = 118.0;

const NAVY: Rgb = Rgb::from_bytes(3, 15, 35);
const GOLD: Rgb = Rgb::from_bytes(198, 161, 91);
const BODY: Rgb = Rgb::from_bytes(45, 55, 75);
const FAINT: Rgb = Rgb::from_bytes(120, 130, 150);
const IVORY: Rgb = Rgb::from_bytes(236, 230, 214);
const PAPER: Rgb = Rgb::from_bytes(247, 244, 237);
const WHITE: Rgb = Rgb::from_bytes(255, 255, 255);
const GREEN: Rgb = Rgb::from_bytes(21, 128, 61);

const SERIF: &str = "FSerif";
const SANS: &str = "FSans";
const SANS_BOLD: &str = "FSansBold";
const MONO: &str = "FMono";

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

fn font_for(name: &str) -> StandardFont {
    match name {
        SANS_BOLD => StandardFont::HelveticaBold,
        SERIF => StandardFont::TimesRoman,
        // No metrics for Courier in the forms font tables: wrapping measures it as Helvetica, which is narrower, but
        // the only mono text here is hashes and ids, which never wrap and fit with room to spare.
        _ => StandardFont::Helvetica,
    }
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

/// Everything the certificate states. `finalized_at` is RFC3339 text carried in.
pub struct Certificate<'a> {
    pub signature_request_id: &'a str,
    pub transaction_document_id: &'a str,
    pub document_title: Option<&'a str>,
    pub recipients: &'a [FinalizeRecipient],
    pub fields: &'a [FinalizeField],
    pub events: &'a [FinalizeEvent],
    pub finalized_at: &'a str,
    /// SHA-256 of the document as it was sent for signature, and of the sealed copy. The pair is what lets anyone
    /// check which bytes were signed.
    pub original_sha256: Option<&'a str>,
    pub sealed_sha256: Option<&'a str>,
    pub page_count: Option<u32>,
}

/// One drawn page and where its next line goes.
struct Pages {
    pages: Vec<Content>,
    cursor: f64,
}

impl Pages {
    fn new() -> Self {
        let mut first = Content::new();
        // The brand band: navy, a gold rule under it, the name and the title in it.
        first.rect(0.0, PAGE_HEIGHT - BAND, PAGE_WIDTH, BAND, NAVY);
        first.rect(0.0, PAGE_HEIGHT - BAND - 3.0, PAGE_WIDTH, 3.0, GOLD);
        first.text(
            SANS_BOLD,
            8.5,
            MARGIN,
            PAGE_HEIGHT - 40.0,
            GOLD,
            &codes("C U L E B R A L U X E   ·   S E C U R E   S I G N I N G"),
        );
        first.text(
            SERIF,
            30.0,
            MARGIN,
            PAGE_HEIGHT - 80.0,
            WHITE,
            &codes("Certificate of Completion"),
        );
        first.text(
            SANS,
            9.0,
            MARGIN,
            PAGE_HEIGHT - 101.0,
            IVORY,
            &codes("The record of an electronic signing, kept with the signed document."),
        );
        Self {
            pages: vec![first],
            cursor: PAGE_HEIGHT - BAND - 34.0,
        }
    }

    fn fresh_page(&mut self) {
        let mut next = Content::new();
        next.rect(0.0, PAGE_HEIGHT - 12.0, PAGE_WIDTH, 12.0, NAVY);
        self.pages.push(next);
        self.cursor = TOP;
    }

    /// Make `height` points available, starting a page if they are not, and return the baseline for the next line.
    fn bump(&mut self, height: f64) -> f64 {
        if self.cursor - height < BOTTOM {
            self.fresh_page();
        }
        self.cursor -= height;
        self.cursor
    }

    fn gap(&mut self, height: f64) {
        self.cursor -= height;
    }

    fn current(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page is always open")
    }

    fn section(&mut self, text: &str) {
        self.gap(10.0);
        let y = self.bump(16.0);
        self.current()
            .text(SANS_BOLD, 8.5, MARGIN, y, GOLD, &codes(&spaced(text)));
        self.current().line(
            (MARGIN, y - 5.0),
            (MARGIN + CONTENT_WIDTH, y - 5.0),
            0.6,
            Rgb::from_bytes(222, 214, 192),
        );
        self.gap(4.0);
    }

    /// Wrapped body text at `x`, one baseline per line.
    fn paragraph(&mut self, font: &str, size: f64, color: Rgb, x: f64, width: f64, text: &str) {
        for line in wrapped(font_for(font), text, size, width) {
            let y = self.bump(size + 4.0);
            self.current().text(font, size, x, y, color, &codes(&line));
        }
    }

    /// `label` in the left column, `value` wrapped in the right.
    fn pair(&mut self, label: &str, font: &str, value: &str) {
        const LABEL_WIDTH: f64 = 112.0;
        let lines = wrapped(font_for(font), value, 9.0, CONTENT_WIDTH - LABEL_WIDTH);
        for (index, line) in lines.iter().enumerate() {
            let y = self.bump(13.0);
            if index == 0 {
                self.current().text(
                    SANS_BOLD,
                    8.0,
                    MARGIN,
                    y,
                    FAINT,
                    &codes(&label.to_uppercase()),
                );
            }
            self.current()
                .text(font, 9.0, MARGIN + LABEL_WIDTH, y, BODY, &codes(line));
        }
    }
}

/// Letter-space a heading ("RECIPIENTS" -> "R E C I P I E N T S"): the base-14 fonts have no tracking control.
fn spaced(text: &str) -> String {
    text.chars()
        .map(|character| character.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// `2026-10-07T00:07:58.398613+00:00` -> `2026-10-07 00:07:58 UTC`.
fn utc(rfc3339: &str) -> String {
    let date = rfc3339.get(..10).unwrap_or(rfc3339);
    match rfc3339.get(11..19) {
        Some(time) => format!("{date} {time} UTC"),
        None => date.to_owned(),
    }
}

fn role_label(role: &str) -> &'static str {
    match role {
        "approver" => "Approver",
        _ => "Signer",
    }
}

/// The words for a ledger event; unknown kinds keep their own name rather than vanishing.
fn event_label(event_type: &str) -> String {
    match event_type {
        "recipient_access_issued" => "Signing link issued".into(),
        "recipient_access_revoked" => "Signing links closed".into(),
        "recipient_opened" => "Opened the signing page".into(),
        "consent_accepted" => "Agreed to sign electronically".into(),
        "field_completed" => "Completed a field".into(),
        "recipient_completed" => "Finished signing".into(),
        "recipient_declined" => "Declined to sign".into(),
        "envelope_finalized" | "finalized" => "Document sealed".into(),
        other => {
            let mut label = other.replace('_', " ");
            if let Some(first) = label.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

/// How a signer chose their signature to look (the signing page's three appearances).
fn appearance(value: &Option<serde_json::Value>) -> Option<&'static str> {
    match value
        .as_ref()
        .and_then(|value| value.get("style"))
        .and_then(serde_json::Value::as_u64)
    {
        Some(0) => Some("Classic"),
        Some(1) => Some("Refined"),
        Some(2) => Some("Bold"),
        _ => None,
    }
}

/// Draw the completion record.
pub fn render_completion_certificate(certificate: &Certificate<'_>) -> Result<Vec<u8>, String> {
    let mut pdf = Pdf::new();
    let serif = pdf.font("Times-Roman");
    let sans = pdf.font("Helvetica");
    let sans_bold = pdf.font("Helvetica-Bold");
    let mono = pdf.font("Courier");
    let page_tree = pdf.reserve();
    let resources = pdf.dictionary(&crate::vault::pdf::resources(
        &[
            (SERIF, serif),
            (SANS, sans),
            (SANS_BOLD, sans_bold),
            (MONO, mono),
        ],
        &[],
    ));

    let mut pages = Pages::new();

    // THE DOCUMENT -----------------------------------------------------------------------------------------------
    pages.section("THE DOCUMENT");
    let title = certificate
        .document_title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("Untitled document");
    pages.paragraph(SERIF, 17.0, NAVY, MARGIN, CONTENT_WIDTH, title);
    pages.gap(4.0);
    {
        // A status chip: the one fact most readers come for.
        let y = pages.bump(20.0);
        pages.current().rect(MARGIN, y - 5.0, 84.0, 17.0, GREEN);
        pages
            .current()
            .text(SANS_BOLD, 8.5, MARGIN + 9.0, y, WHITE, &codes("COMPLETED"));
        pages.current().text(
            SANS,
            9.0,
            MARGIN + 96.0,
            y,
            BODY,
            &codes(&format!(
                "Every required party signed · {}",
                utc(certificate.finalized_at)
            )),
        );
    }
    pages.gap(6.0);
    if let Some(count) = certificate.page_count {
        pages.pair(
            "Pages",
            SANS,
            &format!("{count} {}", if count == 1 { "page" } else { "pages" }),
        );
    }
    pages.pair("Envelope", MONO, certificate.signature_request_id);
    pages.pair("Document", MONO, certificate.transaction_document_id);
    if let Some(hash) = certificate.original_sha256 {
        pages.pair("Original SHA-256", MONO, hash);
    }
    if let Some(hash) = certificate.sealed_sha256 {
        pages.pair("Signed SHA-256", MONO, hash);
    }
    if certificate.original_sha256.is_some() {
        pages.gap(2.0);
        pages.paragraph(
            SANS,
            8.0,
            FAINT,
            MARGIN,
            CONTENT_WIDTH,
            "The SHA-256 values are fingerprints of the document's bytes. Compute the fingerprint of the file you hold; if it matches, it is the document that was signed.",
        );
    }

    // THE SIGNERS ------------------------------------------------------------------------------------------------
    pages.section("PARTIES");
    for recipient in certificate.recipients {
        // A card is kept whole: it needs about 78 points.
        if pages.cursor - 78.0 < BOTTOM {
            pages.fresh_page();
        }
        let top = pages.cursor;
        pages.gap(2.0);
        let name_y = pages.bump(15.0);
        pages.current().text(
            SANS_BOLD,
            12.0,
            MARGIN + 12.0,
            name_y,
            NAVY,
            &codes(&recipient.name),
        );
        let role = format!(
            "{} · {}",
            role_label(&recipient.role),
            if recipient.state == "completed" {
                "Signed"
            } else {
                "Did not complete"
            }
        );
        let role_width = text_width(StandardFont::HelveticaBold, &role, 8.5);
        pages.current().text(
            SANS_BOLD,
            8.5,
            MARGIN + CONTENT_WIDTH - role_width,
            name_y,
            if recipient.state == "completed" {
                GREEN
            } else {
                FAINT
            },
            &codes(&role),
        );
        let email_y = pages.bump(13.0);
        pages.current().text(
            SANS,
            9.0,
            MARGIN + 12.0,
            email_y,
            BODY,
            &codes(&recipient.email),
        );
        if let Some(at) = &recipient.completed_at {
            let y = pages.bump(13.0);
            pages.current().text(
                SANS,
                9.0,
                MARGIN + 12.0,
                y,
                BODY,
                &codes(&format!("Signed {}", utc(at))),
            );
        }
        if let (Some(version), Some(sha), Some(at)) = (
            &recipient.consent_version,
            &recipient.consent_sha256,
            &recipient.consent_accepted_at,
        ) {
            let short: String = sha.chars().take(12).collect();
            let y = pages.bump(13.0);
            pages.current().text(
                SANS,
                8.5,
                MARGIN + 12.0,
                y,
                FAINT,
                &codes(&format!(
                    "Agreed to sign electronically {} · consent {version} · ref {short}",
                    utc(at)
                )),
            );
        } else {
            let y = pages.bump(13.0);
            pages.current().text(
                SANS,
                8.5,
                MARGIN + 12.0,
                y,
                FAINT,
                &codes("No consent recorded."),
            );
        }
        let style = certificate
            .fields
            .iter()
            .filter(|field| field.recipient_id == recipient.id && field.field_type == "signature")
            .find_map(|field| appearance(&field.value));
        let answered = certificate
            .fields
            .iter()
            .filter(|field| field.recipient_id == recipient.id && field.completed_at.is_some())
            .count();
        let total = certificate
            .fields
            .iter()
            .filter(|field| field.recipient_id == recipient.id)
            .count();
        let mut summary = format!("{answered} of {total} fields completed");
        if let Some(style) = style {
            summary.push_str(&format!(" · signature appearance: {style}"));
        }
        let y = pages.bump(13.0);
        pages
            .current()
            .text(SANS, 8.5, MARGIN + 12.0, y, FAINT, &codes(&summary));
        // The card's gold edge, drawn now that its height is known.
        let bottom = pages.cursor - 4.0;
        pages
            .current()
            .rect(MARGIN, bottom, 3.0, top - bottom, GOLD);
        pages.gap(10.0);
    }

    // THE LEDGER -------------------------------------------------------------------------------------------------
    pages.section("WHAT HAPPENED, IN ORDER");
    let names: std::collections::BTreeMap<&str, &str> = certificate
        .recipients
        .iter()
        .map(|recipient| (recipient.id.as_str(), recipient.name.as_str()))
        .collect();
    for event in certificate.events {
        let who = event
            .recipient_id
            .as_deref()
            .and_then(|id| names.get(id).copied())
            .map(str::to_owned);
        let mut what = event_label(&event.event_type);
        if let Some(who) = who {
            what = format!("{who} · {what}");
        }
        let mut detail = Vec::new();
        for (key, label) in [("ipAddress", "from"), ("userAgent", "device")] {
            if let Some(value) = event
                .evidence
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let value: String = value.chars().take(90).collect();
                detail.push(format!("{label} {value}"));
            }
        }
        if let Some(reason) = event
            .evidence
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            detail.push(format!("reason: {reason}"));
        }
        let y = pages.bump(13.0);
        pages.current().text(
            MONO,
            8.0,
            MARGIN,
            y,
            FAINT,
            &codes(&utc(&event.occurred_at)),
        );
        let lines = wrapped(StandardFont::Helvetica, &what, 9.0, CONTENT_WIDTH - 150.0);
        for (index, line) in lines.iter().enumerate() {
            let baseline = if index == 0 { y } else { pages.bump(12.0) };
            pages
                .current()
                .text(SANS, 9.0, MARGIN + 150.0, baseline, BODY, &codes(line));
        }
        if !detail.is_empty() {
            for line in wrapped(
                StandardFont::Helvetica,
                &detail.join(" · "),
                8.0,
                CONTENT_WIDTH - 150.0,
            ) {
                let baseline = pages.bump(11.0);
                pages
                    .current()
                    .text(SANS, 8.0, MARGIN + 150.0, baseline, FAINT, &codes(&line));
            }
        }
    }

    // FOOTERS ----------------------------------------------------------------------------------------------------
    let total = pages.pages.len();
    let reference = short_id(certificate.signature_request_id);
    for (index, content) in pages.pages.iter_mut().enumerate() {
        content.rect(0.0, 0.0, PAGE_WIDTH, 36.0, PAPER);
        content.text(
            SANS,
            7.5,
            MARGIN,
            15.0,
            FAINT,
            &codes(&format!(
                "CulebraLuxe native signing · certificate {reference} · generated {}",
                utc(certificate.finalized_at)
            )),
        );
        let label = format!("Page {} of {}", index + 1, total);
        let width = text_width(StandardFont::Helvetica, &label, 7.5);
        content.text(
            SANS,
            7.5,
            MARGIN + CONTENT_WIDTH - width,
            15.0,
            FAINT,
            &codes(&label),
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
        &pdf_date(certificate.finalized_at),
    );
    pdf.finish(page_tree, &page_refs, Some(info))
        .map_err(|error| error.to_string())
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
                consent_sha256: Some("abcdef0123456789abcdef".into()),
                consent_accepted_at: Some("2026-01-01T00:00:00+00:00".into()),
                completed_at: Some("2026-01-02T00:00:00+00:00".into()),
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
                value: Some(serde_json::json!({"style": 2, "name": "María Rivera"})),
                completed_at: Some("2026-01-02T00:00:00+00:00".into()),
            }],
            vec![
                FinalizeEvent {
                    event_type: "consent_accepted".into(),
                    occurred_at: "2026-01-01T00:00:00+00:00".into(),
                    actor_id: None,
                    recipient_id: Some("r1".into()),
                    evidence: serde_json::json!({"ipAddress": "203.0.113.9", "userAgent": "Safari"}),
                },
                FinalizeEvent {
                    event_type: "recipient_completed".into(),
                    occurred_at: "2026-01-02T00:00:00+00:00".into(),
                    actor_id: None,
                    recipient_id: Some("r1".into()),
                    evidence: serde_json::json!({}),
                },
            ],
        )
    }

    fn render(
        recipients: &[FinalizeRecipient],
        fields: &[FinalizeField],
        events: &[FinalizeEvent],
    ) -> Vec<u8> {
        render_completion_certificate(&Certificate {
            signature_request_id: "11111111-2222-3333-4444-555555555555",
            transaction_document_id: "66666666-7777-8888-9999-000000000000",
            document_title: Some("Listing Agreement · Casa del Mar"),
            recipients,
            fields,
            events,
            finalized_at: "2026-01-03T00:00:00+00:00",
            original_sha256: Some("a".repeat(64).as_str()),
            sealed_sha256: Some("b".repeat(64).as_str()),
            page_count: Some(6),
        })
        .expect("renders")
    }

    #[test]
    fn certificate_is_a_pdf_and_deterministic() {
        let (recipients, fields, events) = inputs();
        let first = render(&recipients, &fields, &events);
        assert!(first.starts_with(b"%PDF-1.4"), "a real PDF document");
        assert_eq!(
            first,
            render(&recipients, &fields, &events),
            "identical inputs render identical bytes"
        );
    }

    /// The text of every page, read back through a PDF parser (the streams are compressed, so a byte scan sees nothing).
    fn page_texts(bytes: &[u8]) -> Vec<String> {
        let document = lopdf::Document::load_mem(bytes).expect("the certificate parses");
        document
            .get_pages()
            .keys()
            .map(|number| document.extract_text(&[*number]).unwrap_or_default())
            .collect()
    }

    #[test]
    fn accented_names_and_the_middle_dot_survive_into_the_page() {
        let (recipients, fields, events) = inputs();
        let text = page_texts(&render(&recipients, &fields, &events)).join("\n");
        assert!(
            text.contains("María Rivera"),
            "the accented name is drawn intact: {text}"
        );
        assert!(
            text.contains("Listing Agreement \u{b7} Casa del Mar"),
            "the middle dot survives: {text}"
        );
        assert!(!text.contains('\u{fffd}'));
    }

    #[test]
    fn a_long_ledger_runs_onto_more_pages_each_with_its_own_footer() {
        let (recipients, fields, mut events) = inputs();
        let template = events[0].clone();
        for _ in 0..120 {
            events.push(template.clone());
        }
        let texts = page_texts(&render(&recipients, &fields, &events));
        assert!(texts.len() >= 2, "120 events do not fit on one page");
        for (index, text) in texts.iter().enumerate() {
            assert!(
                text.contains(&format!("Page {} of {}", index + 1, texts.len())),
                "page {} states its place: {text}",
                index + 1
            );
        }
    }

    #[test]
    fn the_certificate_states_the_fingerprints_and_the_outcome() {
        let (recipients, fields, events) = inputs();
        let text = page_texts(&render(&recipients, &fields, &events)).join("\n");
        assert!(text.contains(&"a".repeat(64)) && text.contains(&"b".repeat(64)));
        assert!(text.contains("COMPLETED"));
        assert!(text.contains("signature appearance: Bold"));
        assert!(
            text.contains("from 203.0.113.9"),
            "the recorded address is shown: {text}"
        );
    }

    #[test]
    fn the_ledger_uses_words_and_names_the_person() {
        assert_eq!(
            event_label("consent_accepted"),
            "Agreed to sign electronically"
        );
        assert_eq!(event_label("something_new"), "Something new");
        assert_eq!(
            utc("2026-10-07T00:07:58.398613+00:00"),
            "2026-10-07 00:07:58 UTC"
        );
        assert_eq!(
            appearance(&Some(serde_json::json!({"style": 1}))),
            Some("Refined")
        );
        assert_eq!(appearance(&Some(serde_json::json!("proof-strokes"))), None);
    }

    /// The reader's verdict, borrowed from `pdf.rs`: a structurally plausible
    /// file that QuickLook cannot render is not a document.
    #[test]
    #[cfg(target_os = "macos")]
    fn quicklook_renders_the_certificate() {
        let (recipients, fields, events) = inputs();
        let bytes = render(&recipients, &fields, &events);
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
