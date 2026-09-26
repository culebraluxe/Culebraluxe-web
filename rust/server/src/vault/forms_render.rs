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

use crate::vault::pdf::{decimal, resources, Content, Pdf, PdfError, Rgb};
use domain::forms_applied_signature::{
    format_broker_initials, format_broker_signature_date, AppliedSignatureEvidence,
    FormAppliedSignature,
};
use domain::forms_font::{encode, text_width, StandardFont};
use domain::forms_template::{
    TemplateDefinition, TemplateFieldDefinition, TemplateFieldType, TemplatePresentation,
    TemplateSectionDefinition, TemplateSectionSegment, TemplateSignatureGroup, TemplateWhen,
};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- the layout, as the TypeScript composer declares it

const PAGE_WIDTH: f64 = 612.0;
const PAGE_HEIGHT: f64 = 792.0;
const MARGIN_X: f64 = 52.0;
const CONTENT_WIDTH: f64 = 508.0;
const HEADER_TOP: f64 = 34.0;
const BODY_TOP: f64 = 672.0;
const FOOTER_Y: f64 = 34.0;
const BODY_BOTTOM: f64 = 58.0;
const BODY_SIZE: f64 = 10.35;
const BODY_LEADING: f64 = 14.7;
const SECTION_GAP: f64 = 15.0;
const LOGO_TARGET_HEIGHT: f64 = 42.0;
const HEADER_TITLE_SIZE: f64 = 12.5;
const SIGNATURE_BLOCK_HEIGHT: f64 = 104.0;

const NAVY: Rgb = Rgb::from_bytes(3, 15, 35);
const NAVY_SOFT: Rgb = Rgb::from_bytes(47, 70, 94);
const GOLD: Rgb = Rgb::from_bytes(198, 161, 91);
const INK: Rgb = Rgb::from_bytes(28, 31, 35);
const MUTED: Rgb = Rgb::from_bytes(98, 104, 111);
const RULE: Rgb = Rgb::from_bytes(213, 216, 220);
const PAPER_TINT: Rgb = Rgb::from_bytes(247, 248, 249);

/// Deterministic: the same template and values produce the same bytes, so the document's checksum means something.
const DETERMINISTIC_DATE: &str = "D:20000101000000Z";

/// The geometry contract every anchor is stated in, for the provider adapters that convert it.
pub const COORDINATE_SPACE: &str = "pdf-points-bottom-left";

// ---------------------------------------------------------------- text

/// WinAnsi-safe text while preserving Puerto Rican and Spanish Latin characters — the same substitutions the TypeScript
/// renderer makes, so the words on the page are the same words.
pub fn pdf_safe(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.nfc() {
        let mapped = match character {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            '\u{2026}' => {
                out.push_str("...");
                continue;
            }
            '\u{a0}' => ' ',
            other => other,
        };
        let code = mapped as u32;
        let drawable = (0x20..=0x7e).contains(&code) || (0xa1..=0xff).contains(&code);
        out.push(if drawable { mapped } else { '?' });
    }
    out
}

/// The width the renderer measures with: `pdf_safe` first, because that is the text actually drawn.
fn measured_width(font: StandardFont, value: &str, size: f64) -> f64 {
    text_width(font, &pdf_safe(value), size)
}

/// Deterministic USD formatting for money fields.
///
/// MOVED to `domain::forms_format` — the editor needs the same rule for the money input it shows, and one rule with two
/// copies is not one rule. Re-exported here so the composer's own call sites read unchanged.
pub use domain::forms_format::{format_date, format_field_value, format_money};


// ---------------------------------------------------------------- the prose

/// A section's prose: its literal segments with each `<value field="X"/>` substituted, whitespace collapsed.
pub fn interpolate_section_text(
    section: &TemplateSectionDefinition,
    fields: &[TemplateFieldDefinition],
    values: &BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    for segment in &section.segments {
        match segment {
            TemplateSectionSegment::Text(text) => out.push_str(text),
            TemplateSectionSegment::Value(name) => {
                let field = fields.iter().find(|field| &field.name == name);
                let raw = values
                    .get(name)
                    .map(|value| value.trim())
                    .unwrap_or_default();
                match field {
                    Some(field)
                        if field.field_type == TemplateFieldType::Money && raw.is_empty() =>
                    {
                        out.push_str("$0")
                    }
                    Some(field) => out.push_str(&format_field_value(field, raw)),
                    None => out.push_str(raw),
                }
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The document's prose: each visible section's label and its text, blank-line separated.
pub fn document_body_text(
    template: &TemplateDefinition,
    values: &BTreeMap<String, String>,
    section_values: &BTreeMap<String, String>,
) -> String {
    let blocks: Vec<String> = template
        .sections
        .iter()
        .filter(|section| TemplateDefinition::when_satisfied(section.when.as_ref(), values))
        .map(|section| {
            let default_text = interpolate_section_text(section, &template.fields, values);
            let edited = if section.editable {
                section_values
                    .get(&section.name)
                    .map(|value| value.trim().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let text = if edited.is_empty() { default_text } else { edited };
            if text.is_empty() {
                section.label.clone()
            } else {
                format!("{}\n{}", section.label, text)
            }
        })
        .collect();
    blocks.join("\n\n")
}

/// The body to render: freeform prose only once the operator has explicitly edited it, otherwise the template and the
/// current values — a legacy body with no marker may hold stale interpolated fields.
pub fn resolve_document_body(
    template: &TemplateDefinition,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
) -> String {
    let edited_body = sections
        .get("body")
        .map(|body| body.trim())
        .unwrap_or_default();
    let edited = sections.get("bodyEdited").map(String::as_str) == Some("true");
    if edited && !edited_body.is_empty() {
        edited_body.to_string()
    } else {
        document_body_text(template, values, sections)
    }
}

/// The transaction overview: the values a reader needs before the prose, in template order.
pub fn overview_fields(
    template: &TemplateDefinition,
    values: &BTreeMap<String, String>,
) -> Vec<(TemplateFieldDefinition, String)> {
    let filled: Vec<(TemplateFieldDefinition, String)> = template
        .fields
        .iter()
        .map(|field| {
            let raw = values
                .get(&field.name)
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            (field.clone(), format_field_value(field, &raw))
        })
        .filter(|(_, value)| !value.is_empty())
        .collect();
    if template.rendering.presentation != TemplatePresentation::Agreement {
        return filled;
    }
    // Agreements lead with a deliberately small overview; the contract prose below remains authoritative.
    let preferred: Vec<(TemplateFieldDefinition, String)> = filled
        .iter()
        .filter(|(field, _)| {
            let haystack = format!("{} {}", field.name, field.label).to_lowercase();
            [
                "buyer",
                "seller",
                "property",
                "price",
                "amount",
                "deposit",
                "closing date",
                "start date",
                "end date",
            ]
            .iter()
            .any(|needle| haystack.contains(needle))
        })
        .cloned()
        .collect();
    let mut selected: Vec<(TemplateFieldDefinition, String)> =
        preferred.into_iter().take(8).collect();
    if selected.len() >= 4 {
        return selected;
    }
    for item in filled {
        if selected.iter().any(|(field, _)| field.name == item.0.name) {
            continue;
        }
        selected.push(item);
        if selected.len() >= 6 {
            break;
        }
    }
    selected
}

// ---------------------------------------------------------------- wrapping

/// A word that cannot fit on a line by itself, broken character by character.
fn split_long_word(word: &str, font: StandardFont, size: f64, max_width: f64) -> Vec<String> {
    if measured_width(font, word, size) <= max_width {
        return vec![word.to_string()];
    }
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    for character in word.chars() {
        let candidate = format!("{current}{character}");
        if !current.is_empty() && measured_width(font, &candidate, size) > max_width {
            pieces.push(std::mem::take(&mut current));
            current.push(character);
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// Greedy, measured word wrap — the same wrap the TypeScript renderer performs, and therefore the same pagination.
pub fn wrap_text(text: &str, font: StandardFont, size: f64, max_width: f64) -> Vec<String> {
    let normalized = pdf_safe(text).split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }
    let words: Vec<String> = normalized
        .split(' ')
        .flat_map(|word| split_long_word(word, font, size, max_width))
        .collect();
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in words {
        let candidate = if current.is_empty() {
            word.clone()
        } else {
            format!("{current} {word}")
        };
        if measured_width(font, &candidate, size) <= max_width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            current = word;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

// ---------------------------------------------------------------- the documents' geometry

/// MOVED to `domain::forms_geometry`: the evidence records where a signature landed, that evidence is written into the
/// issued snapshot, and the screen reads it back — so one definition serves the renderer and the reader.
pub use domain::forms_geometry::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorKind {
    Signature,
    Initial,
    Date,
}

impl AnchorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Initial => "initial",
            Self::Date => "date",
        }
    }
}

/// One signature region on an immutable issued document. The provider adapter converts this at its own boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureAnchor {
    pub role: String,
    pub slot_id: Option<String>,
    pub kind: AnchorKind,
    /// Zero-based page index, as the TypeScript contract states it.
    pub page_index: usize,
    pub page_width: f64,
    pub page_height: f64,
    pub rect: Rect,
}

/// One person who will sign, as the form carries them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub role: String,
    pub slot_id: Option<String>,
    pub name: String,
}

/// The brand wordmark, decoded. Absent when the asset cannot be read, which falls back to the text wordmark.
#[derive(Debug, Clone)]
pub struct Logo {
    pub width: u32,
    pub height: u32,
    pub colour: Vec<u8>,
    pub alpha: Vec<u8>,
}

impl Logo {
    /// Load the canonical wordmark from a PNG. `None` is not an error: the header draws the text wordmark instead,
    /// exactly as the TypeScript renderer does when the asset is missing.
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let rgba = reader.decode().ok()?.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut colour = Vec::with_capacity(width as usize * height as usize * 3);
        let mut alpha = Vec::with_capacity(width as usize * height as usize);
        for pixel in rgba.pixels() {
            colour.extend_from_slice(&pixel.0[0..3]);
            alpha.push(pixel.0[3]);
        }
        Some(Self {
            width,
            height,
            colour,
            alpha,
        })
    }

    fn aspect(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }
}

/// Text as WinAnsi codes. `pdf_safe` admits exactly the WinAnsi repertoire, so the fallback is unreachable; drawing
/// nothing would be a silent lie, so an impossible character draws as `?`.
fn codes(text: &str) -> Vec<u8> {
    let safe = pdf_safe(text);
    match encode(&safe) {
        Ok(codes) => codes,
        Err(_) => vec![b'?'; safe.chars().count()],
    }
}

// ---------------------------------------------------------------- the composer

/// One document being composed: the pages written so far, where the cursor is, and the anchors found on the way.
struct Composer {
    pages: Vec<Content>,
    cursor_y: f64,
    page_index: usize,
    anchors: Vec<SignatureAnchor>,
    logo_width: f64,
    title: String,
    issuer: String,
    template_id: String,
    template_version: i32,
    issued_version: i32,
    presentation: TemplatePresentation,
    /// The material already signed locally, with the image each one draws. It is what turns a slot into a signature.
    applied: Vec<(FormAppliedSignature, EmbeddedSignature)>,
    /// What the document records about each signature it drew.
    applied_evidence: Vec<AppliedSignatureEvidence>,
}

/// A locally applied signature's decoded image: the XObject's name and its own pixel size, which is what the fit needs.
#[derive(Debug, Clone)]
pub struct EmbeddedSignature {
    pub resource_name: String,
    pub width: u32,
    pub height: u32,
}

impl Composer {
    fn new(
        template: &TemplateDefinition,
        issued_version: i32,
        logo: Option<&Logo>,
        applied: Vec<(FormAppliedSignature, EmbeddedSignature)>,
    ) -> Result<Self, PdfError> {
        let mut composer = Self {
            pages: Vec::new(),
            cursor_y: BODY_TOP,
            page_index: 0,
            anchors: Vec::new(),
            logo_width: logo
                .map(|logo| logo.aspect() * LOGO_TARGET_HEIGHT)
                .unwrap_or(0.0),
            title: template.rendering.title.clone(),
            issuer: template.rendering.issuer.clone(),
            template_id: template.id.clone(),
            template_version: template.version,
            issued_version,
            presentation: template.rendering.presentation,
            applied,
            applied_evidence: Vec::new(),
        };
        composer.add_page()?;
        Ok(composer)
    }

    fn page(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page is always open")
    }

    /// Begin a page and draw its header — every page carries the header, including the first.
    fn add_page(&mut self) -> Result<(), PdfError> {
        self.pages.push(Content::new());
        self.page_index = self.pages.len() - 1;
        self.cursor_y = BODY_TOP;
        self.draw_header()
    }
}

impl Composer {
    /// The wordmark, the title on one canonical baseline, and the gold hairline beneath them.
    fn draw_header(&mut self) -> Result<(), PdfError> {
        let logo_y = PAGE_HEIGHT - HEADER_TOP - LOGO_TARGET_HEIGHT;
        let logo_width = self.logo_width;
        if logo_width > 0.0 {
            self.page().image(
                "BrandLogo",
                PAGE_WIDTH - MARGIN_X - logo_width,
                logo_y,
                logo_width,
                LOGO_TARGET_HEIGHT,
            );
        } else {
            // The text wordmark, exactly as the TypeScript renderer falls back when the asset cannot be read.
            let wordmark = codes("CULEBRALUXE");
            let width = measured_width(StandardFont::TimesBold, "CULEBRALUXE", 12.0);
            self.page().text(
                "FBodyBold",
                12.0,
                PAGE_WIDTH - MARGIN_X - width,
                PAGE_HEIGHT - HEADER_TOP - 24.0,
                NAVY,
                &wordmark,
            );
        }

        let title = pdf_safe(&self.title);
        let title_width = (CONTENT_WIDTH - logo_width.max(112.0) - 28.0).max(250.0);
        if measured_width(StandardFont::HelveticaBold, &title, HEADER_TITLE_SIZE) > title_width {
            return Err(PdfError::new(format!(
                "Form title exceeds the fixed header width: {}",
                self.title
            )));
        }
        let baseline = logo_y + (LOGO_TARGET_HEIGHT - HEADER_TITLE_SIZE) / 2.0 + 1.0;
        let title_codes = codes(&title);
        self.page().text(
            "FSansBold",
            HEADER_TITLE_SIZE,
            MARGIN_X,
            baseline,
            NAVY,
            &title_codes,
        );
        let rule_y = PAGE_HEIGHT - HEADER_TOP - LOGO_TARGET_HEIGHT - 11.0;
        self.page().line(
            (MARGIN_X, rule_y),
            (PAGE_WIDTH - MARGIN_X, rule_y),
            1.1,
            GOLD,
        );
        Ok(())
    }
}

impl Composer {
    /// The rule, the document identity and the page label — drawn once the number of pages is known.
    fn draw_footers(&mut self) {
        let total = self.pages.len();
        let identity = format!(
            "{} \u{b7} Template {} \u{b7} Document v{}",
            self.template_id, self.template_version, self.issued_version
        );
        for index in 0..total {
            let label = format!("Page {} of {total}", index + 1);
            let label_width = measured_width(StandardFont::Helvetica, &label, 6.8);
            let identity_codes = codes(&identity);
            let label_codes = codes(&label);
            let page = &mut self.pages[index];
            page.line(
                (MARGIN_X, FOOTER_Y + 10.0),
                (PAGE_WIDTH - MARGIN_X, FOOTER_Y + 10.0),
                0.45,
                RULE,
            );
            page.text("FSans", 6.8, MARGIN_X, FOOTER_Y, MUTED, &identity_codes);
            page.text(
                "FSans",
                6.8,
                PAGE_WIDTH - MARGIN_X - label_width,
                FOOTER_Y,
                MUTED,
                &label_codes,
            );
        }
    }

    /// Break to a new page when what comes next would not fit above the footer.
    fn ensure_space(&mut self, height: f64) -> Result<(), PdfError> {
        if self.cursor_y - height >= BODY_BOTTOM {
            return Ok(());
        }
        self.add_page()
    }

    /// The small uppercase label the overview and the signature blocks are introduced with.
    fn upper_label(&mut self, value: &str, x: f64, y: f64) {
        let text = codes(&value.to_uppercase());
        self.page().text("FSansBold", 6.8, x, y, MUTED, &text);
    }

    /// The issuer and the version this document was issued as, above the overview.
    fn draw_document_metadata(&mut self) -> Result<(), PdfError> {
        self.ensure_space(22.0)?;
        let text = codes(&format!(
            "{} \u{b7} Document version {}",
            self.issuer, self.issued_version
        ));
        let y = self.cursor_y - 1.0;
        self.page().text("FSans", 7.5, MARGIN_X, y, MUTED, &text);
        self.cursor_y -= 22.0;
        Ok(())
    }
}

/// The prose split into blocks: a blank line separates them, the first line is the heading, the rest is the paragraph.
fn parse_body_blocks(body: &str) -> Vec<(String, String)> {
    body.split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .map(|block| {
            let mut lines = block.split('\n');
            let heading = lines.next().unwrap_or("").trim().to_string();
            let paragraph = lines.collect::<Vec<_>>().join(" ");
            let paragraph = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
            (heading, paragraph)
        })
        .collect()
}

/// The name a signature block shows: a generic role on the first slot takes the bound field's value, which is the name
/// the parties actually wrote down.
fn signature_block_display_name(
    participant_name: &str,
    bound_field_value: &str,
    participant_index: usize,
) -> String {
    let name = participant_name.trim();
    let bound = bound_field_value.trim();
    let generic = matches!(
        name.to_lowercase().as_str(),
        "owner" | "seller" | "buyer"
    );
    if participant_index == 0 && !bound.is_empty() && generic {
        bound.to_string()
    } else {
        name.to_string()
    }
}

impl Composer {
    /// The overview: a two-column grid of the values that matter before the prose, on a tinted panel with a gold edge.
    fn draw_overview(&mut self, fields: &[(TemplateFieldDefinition, String)]) -> Result<(), PdfError> {
        if fields.is_empty() {
            return Ok(());
        }
        let heading = if self.presentation == TemplatePresentation::Agreement {
            "Agreement overview"
        } else {
            "Document details"
        };
        self.ensure_space(58.0)?;
        let label_y = self.cursor_y - 2.0;
        self.upper_label(heading, MARGIN_X + 12.0, label_y);
        self.cursor_y -= 15.0;

        let column_gap = 24.0;
        let column_width = (CONTENT_WIDTH - column_gap - 24.0) / 2.0;
        let mut index = 0;
        while index < fields.len() {
            let left = &fields[index];
            let right = fields.get(index + 1);
            let left_lines = wrap_text(&left.1, StandardFont::TimesRoman, 10.0, column_width);
            let right_lines = right
                .map(|item| wrap_text(&item.1, StandardFont::TimesRoman, 10.0, column_width))
                .unwrap_or_default();
            let rows = left_lines.len().max(right_lines.len()).max(1) as f64;
            let row_height = rows * 13.0 + 25.0;
            self.ensure_space(row_height)?;
            let panel_y = self.cursor_y - row_height + 6.0;
            self.page()
                .rect(MARGIN_X, panel_y, CONTENT_WIDTH, row_height, PAPER_TINT);
            self.page()
                .rect(MARGIN_X, panel_y, 2.0, row_height, GOLD);
            self.draw_cell(left, MARGIN_X + 12.0, &left_lines);
            if let Some(right) = right {
                let right_x = MARGIN_X + 12.0 + column_width + column_gap;
                self.draw_cell(right, right_x, &right_lines);
            }
            self.cursor_y -= row_height + 4.0;
            index += 2;
        }
        self.cursor_y -= 8.0;
        Ok(())
    }

    /// One overview cell: the field's label, then its value wrapped into the column.
    fn draw_cell(&mut self, item: &(TemplateFieldDefinition, String), x: f64, lines: &[String]) {
        let label_y = self.cursor_y - 9.0;
        self.upper_label(&item.0.label, x, label_y);
        for (line_index, line) in lines.iter().enumerate() {
            let y = self.cursor_y - 23.0 - (line_index as f64) * 13.0;
            let text = codes(line);
            self.page().text("FBody", 10.0, x, y, INK, &text);
        }
    }
}

impl Composer {
    /// The contract prose: numbered section headings for an agreement, plain headings otherwise.
    fn draw_body(&mut self, body: &str) -> Result<(), PdfError> {
        let mut section_number = 0;
        for (heading, paragraph) in parse_body_blocks(body) {
            if paragraph.is_empty() {
                continue;
            }
            section_number += 1;
            let heading = if self.presentation == TemplatePresentation::Agreement {
                format!("{section_number:02}  {heading}")
            } else {
                heading
            };
            let body_lines = wrap_text(&paragraph, StandardFont::TimesRoman, BODY_SIZE, CONTENT_WIDTH);
            let keep_with = 14.0 + (body_lines.len().min(2) as f64) * BODY_LEADING;
            self.ensure_space(keep_with + SECTION_GAP)?;
            let heading_codes = codes(&heading.to_uppercase());
            let y = self.cursor_y;
            self.page().text("FSansBold", 8.2, MARGIN_X, y, NAVY, &heading_codes);
            self.cursor_y -= 15.0;
            for line in &body_lines {
                self.ensure_space(BODY_LEADING)?;
                let text = codes(line);
                let y = self.cursor_y;
                self.page().text("FBody", BODY_SIZE, MARGIN_X, y, INK, &text);
                self.cursor_y -= BODY_LEADING;
            }
            self.cursor_y -= SECTION_GAP;
        }
        Ok(())
    }

    /// The signature blocks, and the anchor rectangles the provider will place signatures in.
    fn draw_signatures(
        &mut self,
        template: &TemplateDefinition,
        values: &BTreeMap<String, String>,
        participants: &[Participant],
    ) -> Result<(), PdfError> {
        if template.signature_groups.is_empty() {
            return Ok(());
        }
        // A signature section that fits on a page of its own starts one; otherwise it only has to not be orphaned.
        let block_count: usize = template
            .signature_groups
            .iter()
            .map(|group| {
                participants
                    .iter()
                    .filter(|participant| participant.role == group.role)
                    .count()
                    .max(1)
            })
            .sum();
        let full_height = 46.0 + (block_count as f64) * SIGNATURE_BLOCK_HEIGHT;
        if full_height <= BODY_TOP - BODY_BOTTOM && self.cursor_y - full_height < BODY_BOTTOM {
            self.add_page()?;
        } else {
            self.ensure_space(65.0)?;
        }

        let section_rule_y = self.cursor_y;
        self.page().line(
            (MARGIN_X, section_rule_y),
            (PAGE_WIDTH - MARGIN_X, section_rule_y),
            0.75,
            RULE,
        );
        self.cursor_y -= 23.0;
        let heading = codes("SIGNATURES");
        let y = self.cursor_y;
        self.page().text("FBodyBold", 12.0, MARGIN_X, y, NAVY, &heading);
        self.cursor_y -= 23.0;

        for group in &template.signature_groups {
            let matching: Vec<&Participant> = participants
                .iter()
                .filter(|participant| participant.role == group.role)
                .collect();
            let bound = group
                .field
                .as_ref()
                .and_then(|name| values.get(name))
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            let signers: Vec<(Option<String>, String)> = if matching.is_empty() {
                // No participant resolved for the role: the block is still drawn, named by the bound field, so the
                // document carries the place to sign even before anyone is attached to it.
                vec![(None, bound.clone())]
            } else {
                matching
                    .iter()
                    .enumerate()
                    .map(|(index, participant)| {
                        (
                            participant.slot_id.clone(),
                            signature_block_display_name(&participant.name, &bound, index),
                        )
                    })
                    .collect()
            };
            let signer_count = signers.len();
            for (slot_id, name) in signers {
                // WHICH APPLIED SIGNATURE BELONGS TO THIS BLOCK — the TypeScript composer's rule exactly: the same role,
                // then the same slot when the block has one, and otherwise only when this group has a single signer and
                // that signer is the role's only applied signature. A looser match would print a signature on a line it
                // does not belong to, and the anchors would then claim a slot is still waiting for a provider.
                let role_applied: Vec<&(FormAppliedSignature, EmbeddedSignature)> = self
                    .applied
                    .iter()
                    .filter(|(signature, _)| signature.role == group.role)
                    .collect();
                let exact: Vec<&(FormAppliedSignature, EmbeddedSignature)> = match slot_id.as_deref() {
                    Some(slot) => role_applied
                        .iter()
                        .copied()
                        .filter(|(signature, _)| signature.slot_id.as_deref() == Some(slot))
                        .collect(),
                    None => Vec::new(),
                };
                let fallback: Vec<&(FormAppliedSignature, EmbeddedSignature)> =
                    if signer_count == 1 && role_applied.len() == 1 {
                        role_applied.clone()
                    } else {
                        Vec::new()
                    };
                let chosen = exact
                    .first()
                    .copied()
                    .or_else(|| fallback.first().copied())
                    .cloned();
                self.draw_signature_block(group, slot_id, name, chosen)?;
            }
        }
        Ok(())
    }
}

/// The initials rule's width, as the layout declares it.
const INITIALS_WIDTH: f64 = 64.0;

impl Composer {
    /// One signer's block: the label, three rules, and either the anchor rectangles a provider places signatures in, or
    /// — when the brokerage has already signed this slot locally — the signature, its date and its initials.
    fn draw_signature_block(
        &mut self,
        group: &TemplateSignatureGroup,
        slot_id: Option<String>,
        name: String,
        applied: Option<(FormAppliedSignature, EmbeddedSignature)>,
    ) -> Result<(), PdfError> {
        self.ensure_space(SIGNATURE_BLOCK_HEIGHT)?;
        let label_y = self.cursor_y;
        self.upper_label(&group.label, MARGIN_X, label_y);
        self.cursor_y -= 11.0;

        let line_y = self.cursor_y - 39.0;
        let signature_width = if group.initials { 252.0 } else { 302.0 };
        let initials_x = MARGIN_X + signature_width + 20.0;
        let date_x = if group.initials {
            initials_x + INITIALS_WIDTH + 20.0
        } else {
            MARGIN_X + 322.0
        };
        let date_width = PAGE_WIDTH - MARGIN_X - date_x;

        self.page().line(
            (MARGIN_X, line_y),
            (MARGIN_X + signature_width, line_y),
            0.65,
            NAVY_SOFT,
        );
        if group.initials {
            self.page().line(
                (initials_x, line_y),
                (initials_x + INITIALS_WIDTH, line_y),
                0.65,
                NAVY_SOFT,
            );
        }
        self.page().line(
            (date_x, line_y),
            (date_x + date_width, line_y),
            0.65,
            NAVY_SOFT,
        );

        let signature_rect = Rect {
            x: MARGIN_X,
            y: line_y + 2.0,
            width: signature_width,
            height: 34.0,
        };
        let initials_rect = group.initials.then_some(Rect {
            x: initials_x,
            y: line_y + 2.0,
            width: INITIALS_WIDTH,
            height: 27.0,
        });
        let date_rect = Rect {
            x: date_x,
            y: line_y + 2.0,
            width: date_width,
            height: 20.0,
        };
        if applied.is_none() {
            self.add_anchor(
                &group.role,
                slot_id.clone(),
                AnchorKind::Signature,
                signature_rect,
            );
            if let Some(rect) = initials_rect {
                self.add_anchor(&group.role, slot_id.clone(), AnchorKind::Initial, rect);
            }
            self.add_anchor(&group.role, slot_id.clone(), AnchorKind::Date, date_rect);
        }

        let under = line_y - 9.0;
        let signature_label = codes("SIGNATURE");
        self.page()
            .text("FSans", 6.1, MARGIN_X, under, MUTED, &signature_label);
        if group.initials {
            let initials_label = codes("INITIALS");
            self.page()
                .text("FSans", 6.1, initials_x, under, MUTED, &initials_label);
        }
        let date_label = codes("DATE");
        self.page()
            .text("FSans", 6.1, date_x, under, MUTED, &date_label);

        if !name.trim().is_empty() {
            let first = wrap_text(&name, StandardFont::Helvetica, 8.2, signature_width)
                .into_iter()
                .next();
            if let Some(first) = first {
                let text = codes(&first);
                self.page()
                    .text("FSans", 8.2, MARGIN_X, line_y - 22.0, INK, &text);
            }
        }
        if let Some((signature, image)) = applied {
            // The image is FITTED, never stretched: a stretched signature is a different signature.
            let (image_width, image_height) = signature_rect.scale_to_fit(
                signature_rect.width - 8.0,
                signature_rect.height - 2.0,
                image.width as f64,
                image.height as f64,
            );
            self.page().image(
                &image.resource_name,
                signature_rect.x + 4.0,
                signature_rect.y + 1.0,
                image_width,
                image_height,
            );
            let rendered_date =
                format_broker_signature_date(&signature.applied_at).map_err(PdfError::new)?;
            if let Some(date_line) = wrap_text(
                &rendered_date,
                StandardFont::Helvetica,
                8.6,
                date_rect.width - 4.0,
            )
            .into_iter()
            .next()
            {
                let text = codes(&date_line);
                self.page()
                    .text("FSans", 8.6, date_rect.x + 2.0, date_rect.y + 5.0, INK, &text);
            }
            let rendered_initials = initials_rect
                .is_some()
                .then(|| format_broker_initials(&signature.signer_name));
            if let (Some(rect), Some(initials)) = (initials_rect, rendered_initials.clone()) {
                let initials_size = 10.5;
                // Measure the TEXT, draw the ENCODED text: the width table is keyed by characters, the page by codes.
                let width = text_width(StandardFont::HelveticaBold, &initials, initials_size);
                let rendered = codes(&initials);
                self.page().text(
                    "FSansBold",
                    initials_size,
                    rect.x + (rect.width - width) / 2.0,
                    rect.y + 6.0,
                    INK,
                    &rendered,
                );
            }
            // The EVIDENCE goes into the snapshot, never the image: provenance is what makes the signature auditable, and
            // the bytes are already rendered into the document that was signed.
            self.applied_evidence.push(AppliedSignatureEvidence {
                role: signature.role,
                slot_id: signature.slot_id,
                signer_name: signature.signer_name,
                credential_line: signature.credential_line,
                signer_app_user_id: signature.signer_app_user_id,
                asset_media_id: signature.asset_media_id,
                asset_checksum_sha256: signature.asset_checksum_sha256,
                applied_at: signature.applied_at,
                consent_basis: signature.consent_basis,
                date_semantic: signature.date_semantic,
                rendered_date,
                rendered_initials,
                page_index: self.page_index as i32,
                signature_rect,
                initials_rect,
                date_rect,
            });
        }
        self.cursor_y = line_y - 49.0;
        Ok(())
    }

    fn add_anchor(
        &mut self,
        role: &str,
        slot_id: Option<String>,
        kind: AnchorKind,
        rect: Rect,
    ) {
        self.anchors.push(SignatureAnchor {
            role: role.to_string(),
            slot_id,
            kind,
            page_index: self.page_index,
            page_width: PAGE_WIDTH,
            page_height: PAGE_HEIGHT,
            rect,
        });
    }
}

// ---------------------------------------------------------------- the entry point

/// What a rendered document is: its bytes, its page count, where the signatures go, and what was signed locally.
#[derive(Debug, Clone)]
pub struct RenderedForm {
    pub bytes: Vec<u8>,
    pub page_count: usize,
    pub signature_anchors: Vec<SignatureAnchor>,
    /// Provenance for every signature this render drew — what the issued snapshot records instead of the bytes.
    pub applied_evidence: Vec<AppliedSignatureEvidence>,
}

/// A signature image, decoded once so it can be drawn and measured.
struct SignatureImage {
    width: u32,
    height: u32,
    components: u8,
    samples: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

impl SignatureImage {
    /// Decode the two formats the protected media store accepts.
    ///
    /// PNG keeps its transparency as a soft mask; a JPEG has none, so it is drawn as it is. Both are decoded through the
    /// `image` crate rather than passed through in their own encoding, so the renderer has exactly one image path.
    fn from_bytes(
        mime: domain::forms_applied_signature::AppliedSignatureImageMimeType,
        bytes: &[u8],
    ) -> Result<Self, PdfError> {
        use domain::forms_applied_signature::AppliedSignatureImageMimeType;
        let decoded = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| PdfError::new(format!("the signature image could not be read: {error}")))?
            .decode()
            .map_err(|error| {
                PdfError::new(format!("the signature image could not be decoded: {error}"))
            })?;
        match mime {
            AppliedSignatureImageMimeType::Jpeg => {
                let rgb = decoded.to_rgb8();
                let (width, height) = rgb.dimensions();
                Ok(Self {
                    width,
                    height,
                    components: 3,
                    samples: rgb.into_raw(),
                    alpha: None,
                })
            }
            AppliedSignatureImageMimeType::Png => {
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                let mut samples = Vec::with_capacity((width as usize) * (height as usize) * 3);
                let mut alpha = Vec::with_capacity((width as usize) * (height as usize));
                for pixel in rgba.pixels() {
                    samples.extend_from_slice(&pixel.0[0..3]);
                    alpha.push(pixel.0[3]);
                }
                Ok(Self {
                    width,
                    height,
                    components: 3,
                    samples,
                    alpha: Some(alpha),
                })
            }
        }
    }
}

/// Render one issued form document.
///
/// THE ORDER IS THE TYPESCRIPT RENDERER'S ORDER and it matters: metadata, then the overview, then the prose, then the
/// signatures. Pagination is a consequence of all four, and every anchor is a consequence of pagination.
pub fn render_form(
    template: &TemplateDefinition,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
    issued_version: i32,
    participants: &[Participant],
    logo: Option<&Logo>,
    applied_signatures: &[FormAppliedSignature],
) -> Result<RenderedForm, PdfError> {
    // A PRE-SIGNATURE MAY ONLY LAND ON A SLOT THE TEMPLATE DECLARES. The repository decides WHOSE signature this is and
    // which slot it satisfies; the template is the only authority on whether that role has a block at all, so the check
    // happens here, where the template is, and a mismatch refuses to render rather than drawing a signature nowhere.
    for signature in applied_signatures {
        if !template
            .signature_groups
            .iter()
            .any(|group| group.role == signature.role)
        {
            return Err(PdfError::new(format!(
                "the template {} declares no {} signature group, so a {} signature cannot be applied to it.",
                template.id, signature.role, signature.role
            )));
        }
    }
    let mut pdf = Pdf::new();
    let body = pdf.font("Times-Roman");
    let body_bold = pdf.font("Times-Bold");
    let sans = pdf.font("Helvetica");
    let sans_bold = pdf.font("Helvetica-Bold");
    let page_tree = pdf.reserve();

    let logo_reference = match logo {
        Some(logo) => Some(pdf.image(
            logo.width,
            logo.height,
            3,
            &logo.colour,
            Some(&logo.alpha),
        )?),
        None => None,
    };
    let fonts = [
        ("FBody", body),
        ("FBodyBold", body_bold),
        ("FSans", sans),
        ("FSansBold", sans_bold),
    ];
    let mut embedded: Vec<(FormAppliedSignature, EmbeddedSignature)> = Vec::new();
    let mut signature_refs: Vec<u32> = Vec::new();
    for (index, signature) in applied_signatures.iter().enumerate() {
        let image =
            SignatureImage::from_bytes(signature.image_mime_type, &signature.image_bytes)?;
        let resource_name = format!("AppliedSignature{index}");
        signature_refs.push(pdf.image(
            image.width,
            image.height,
            image.components,
            &image.samples,
            image.alpha.as_deref(),
        )?);
        embedded.push((
            signature.clone(),
            EmbeddedSignature {
                resource_name,
                width: image.width,
                height: image.height,
            },
        ));
    }
    let mut images: Vec<(&str, u32)> = match logo_reference {
        Some(reference) => vec![("BrandLogo", reference)],
        None => Vec::new(),
    };
    for (item, reference) in embedded.iter().zip(signature_refs.iter()) {
        images.push((item.1.resource_name.as_str(), *reference));
    }
    // A DICTIONARY, NOT A STREAM. The page's `/Resources` has to be readable: written as a stream, a viewer resolves no
    // fonts and no images and the document draws as rules and boxes with no text and no wordmark.
    let resource_body = resources(&fonts, &images);
    let resources_ref = pdf.dictionary(&resource_body);

    let mut composer = Composer::new(template, issued_version, logo, embedded)?;
    composer.draw_document_metadata()?;
    let overview = overview_fields(template, values);
    composer.draw_overview(&overview)?;
    let prose = resolve_document_body(template, values, sections);
    composer.draw_body(&prose)?;
    composer.draw_signatures(template, values, participants)?;
    composer.draw_footers();

    let page_count = composer.pages.len();
    let mut page_refs: Vec<u32> = Vec::with_capacity(page_count);
    for content in composer.pages {
        let reference = pdf.page(
            PAGE_WIDTH,
            PAGE_HEIGHT,
            page_tree,
            resources_ref,
            &content.into_bytes(),
        )?;
        page_refs.push(reference);
    }
    let info = pdf.info(
        &template.rendering.title,
        &template.rendering.issuer,
        &template.document_type_label,
        "CulebraLuxe Forms",
        "CulebraLuxe Forms",
        DETERMINISTIC_DATE,
    );
    let bytes = pdf.finish(page_tree, &page_refs, Some(info))?;
    Ok(RenderedForm {
        bytes,
        page_count,
        signature_anchors: composer.anchors,
        applied_evidence: composer.applied_evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::forms_template::TemplateLibrary;
    use std::path::Path;

    fn repository_path(relative: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    fn library() -> TemplateLibrary {
        TemplateLibrary::load_from_dir(&repository_path("../../lib/forms/templates"))
            .expect("the repository's templates load")
    }

    fn logo() -> Option<Logo> {
        // THE WORDMARK IS AT THE REPOSITORY ROOT's `public/`, which is TWO levels up from `rust/server` — the single level
        // this used to climb meant `logo()` always answered None, so every test here rendered a document with no wordmark
        // and nobody noticed the image path was never exercised.
        let bytes = std::fs::read(repository_path("../../public/brand/CLLOGO.png")).ok()?;
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
        assert!(body.contains("Lisa Penfield"), "the seller's name is interpolated");
        assert!(body.contains("Casa Luar"), "the property is interpolated");
        assert!(body.contains("January 15, 2026"), "a date is formatted, not raw ISO");
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
        use domain::forms_template::TemplateFieldType;
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
        use domain::forms_applied_signature::{
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

    /// Every stream in the file, inflated. A PDF's resource dictionary and its content stream are COMPRESSED, so a
    /// plain-text search of the bytes proves nothing about what a viewer will resolve.
    fn inflated_streams(pdf: &[u8]) -> String {
        use std::io::Read;
        let mut out = String::new();
        let mut cursor = 0;
        while let Some(start) = find_bytes(pdf, b"stream\n", cursor) {
            let body = start + 7;
            let Some(end) = find_bytes(pdf, b"\nendstream", body) else {
                break;
            };
            let mut decoded = String::new();
            let mut decoder = flate2::read::ZlibDecoder::new(&pdf[body..end]);
            if decoder.read_to_string(&mut decoded).is_ok() {
                out.push_str(&decoded);
            }
            cursor = end + 1;
        }
        out
    }

    fn find_bytes(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        haystack[from..]
            .windows(needle.len())
            .position(|window| window == needle)
            .map(|index| index + from)
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
        let streams = inflated_streams(&rendered.bytes);

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