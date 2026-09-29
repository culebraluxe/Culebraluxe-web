//! Moved from `forms_render.rs` (move only): PAGE_WIDTH, PAGE_HEIGHT, MARGIN_X, CONTENT_WIDTH, HEADER_TOP, BODY_TOP, FOOTER_Y, BODY_BOTTOM, BODY_SIZE, BODY_LEADING, SECTION_GAP, LOGO_TARGET_HEIGHT, HEADER_TITLE_SIZE, SIGNATURE_BLOCK_HEIGHT, NAVY, NAVY_SOFT, GOLD, INK, MUTED, RULE, PAPER_TINT, DETERMINISTIC_DATE, COORDINATE_SPACE, pdf_safe, measured_width, interpolate_section_text, document_body_text, resolve_document_body, overview_fields, split_long_word, wrap_text, AnchorKind, as_str, SignatureAnchor, Participant, Logo, from_png, codes, Composer, EmbeddedSignature, new, draw_header, draw_footers, parse_body_blocks, signature_block_display_name.

#[allow(unused_imports)]
use super::*;

pub(super) const PAGE_WIDTH: f64 = 612.0;
pub(super) const PAGE_HEIGHT: f64 = 792.0;
pub(super) const MARGIN_X: f64 = 52.0;
pub(super) const CONTENT_WIDTH: f64 = 508.0;
pub(super) const HEADER_TOP: f64 = 34.0;
pub(super) const BODY_TOP: f64 = 672.0;
pub(super) const FOOTER_Y: f64 = 34.0;
pub(super) const BODY_BOTTOM: f64 = 58.0;
pub(super) const BODY_SIZE: f64 = 10.35;
pub(super) const BODY_LEADING: f64 = 14.7;
pub(super) const SECTION_GAP: f64 = 15.0;
pub(super) const LOGO_TARGET_HEIGHT: f64 = 42.0;
pub(super) const HEADER_TITLE_SIZE: f64 = 12.5;
pub(super) const SIGNATURE_BLOCK_HEIGHT: f64 = 104.0;

pub(super) const NAVY: Rgb = Rgb::from_bytes(3, 15, 35);
pub(super) const NAVY_SOFT: Rgb = Rgb::from_bytes(47, 70, 94);
pub(super) const GOLD: Rgb = Rgb::from_bytes(198, 161, 91);
pub(super) const INK: Rgb = Rgb::from_bytes(28, 31, 35);
pub(super) const MUTED: Rgb = Rgb::from_bytes(98, 104, 111);
pub(super) const RULE: Rgb = Rgb::from_bytes(213, 216, 220);
pub(super) const PAPER_TINT: Rgb = Rgb::from_bytes(247, 248, 249);

/// Deterministic: the same template and values produce the same bytes, so the document's checksum means something.
pub(super) const DETERMINISTIC_DATE: &str = "D:20000101000000Z";

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
pub(super) fn measured_width(font: StandardFont, value: &str, size: f64) -> f64 {
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
pub(super) fn split_long_word(word: &str, font: StandardFont, size: f64, max_width: f64) -> Vec<String> {
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

    pub(super) fn aspect(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }
}

/// Text as WinAnsi codes. `pdf_safe` admits exactly the WinAnsi repertoire, so the fallback is unreachable; drawing
/// nothing would be a silent lie, so an impossible character draws as `?`.
pub(super) fn codes(text: &str) -> Vec<u8> {
    let safe = pdf_safe(text);
    match encode(&safe) {
        Ok(codes) => codes,
        Err(_) => vec![b'?'; safe.chars().count()],
    }
}

// ---------------------------------------------------------------- the composer

/// One document being composed: the pages written so far, where the cursor is, and the anchors found on the way.
pub(super) struct Composer {
    pub(super) pages: Vec<Content>,
    pub(super) cursor_y: f64,
    pub(super) page_index: usize,
    pub(super) anchors: Vec<SignatureAnchor>,
    pub(super) logo_width: f64,
    pub(super) title: String,
    pub(super) issuer: String,
    pub(super) template_id: String,
    pub(super) template_version: i32,
    pub(super) issued_version: i32,
    pub(super) presentation: TemplatePresentation,
    /// The material already signed locally, with the image each one draws. It is what turns a slot into a signature.
    pub(super) applied: Vec<(FormAppliedSignature, EmbeddedSignature)>,
    /// What the document records about each signature it drew.
    pub(super) applied_evidence: Vec<AppliedSignatureEvidence>,
}

/// A locally applied signature's decoded image: the XObject's name and its own pixel size, which is what the fit needs.
#[derive(Debug, Clone)]
pub struct EmbeddedSignature {
    pub resource_name: String,
    pub width: u32,
    pub height: u32,
}

impl Composer {
    pub(super) fn new(
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

    pub(super) fn page(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page is always open")
    }

    /// Begin a page and draw its header — every page carries the header, including the first.
    pub(super) fn add_page(&mut self) -> Result<(), PdfError> {
        self.pages.push(Content::new());
        self.page_index = self.pages.len() - 1;
        self.cursor_y = BODY_TOP;
        self.draw_header()
    }
}

impl Composer {
    /// The wordmark, the title on one canonical baseline, and the gold hairline beneath them.
    pub(super) fn draw_header(&mut self) -> Result<(), PdfError> {
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
    pub(super) fn draw_footers(&mut self) {
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
    pub(super) fn ensure_space(&mut self, height: f64) -> Result<(), PdfError> {
        if self.cursor_y - height >= BODY_BOTTOM {
            return Ok(());
        }
        self.add_page()
    }

    /// The small uppercase label the overview and the signature blocks are introduced with.
    pub(super) fn upper_label(&mut self, value: &str, x: f64, y: f64) {
        let text = codes(&value.to_uppercase());
        self.page().text("FSansBold", 6.8, x, y, MUTED, &text);
    }

    /// The issuer and the version this document was issued as, above the overview.
    pub(super) fn draw_document_metadata(&mut self) -> Result<(), PdfError> {
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
pub(super) fn parse_body_blocks(body: &str) -> Vec<(String, String)> {
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
pub(super) fn signature_block_display_name(
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
