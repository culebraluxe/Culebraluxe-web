//! Moved from `forms_render.rs` (move only): draw_overview, draw_body, INITIALS_WIDTH, draw_signature_block, RenderedForm, SignatureImage, from_bytes, render_form.

#[allow(unused_imports)]
use super::*;

impl Composer {
    /// The overview: a two-column grid of the values that matter before the prose, on a tinted panel with a gold edge.
    pub(super) fn draw_overview(
        &mut self,
        fields: &[(TemplateFieldDefinition, String)],
    ) -> Result<(), PdfError> {
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
            self.page().rect(MARGIN_X, panel_y, 2.0, row_height, GOLD);
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
    pub(super) fn draw_cell(
        &mut self,
        item: &(TemplateFieldDefinition, String),
        x: f64,
        lines: &[String],
    ) {
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
    pub(super) fn draw_body(&mut self, body: &str) -> Result<(), PdfError> {
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
            let body_lines = wrap_text(
                &paragraph,
                StandardFont::TimesRoman,
                BODY_SIZE,
                CONTENT_WIDTH,
            );
            let keep_with = 14.0 + (body_lines.len().min(2) as f64) * BODY_LEADING;
            self.ensure_space(keep_with + SECTION_GAP)?;
            let heading_codes = codes(&heading.to_uppercase());
            let y = self.cursor_y;
            self.page()
                .text("FSansBold", 8.2, MARGIN_X, y, NAVY, &heading_codes);
            self.cursor_y -= 15.0;
            for line in &body_lines {
                self.ensure_space(BODY_LEADING)?;
                let text = codes(line);
                let y = self.cursor_y;
                self.page()
                    .text("FBody", BODY_SIZE, MARGIN_X, y, INK, &text);
                self.cursor_y -= BODY_LEADING;
            }
            self.cursor_y -= SECTION_GAP;
        }
        Ok(())
    }

    /// The signature blocks, and the anchor rectangles the provider will place signatures in.
    pub(super) fn draw_signatures(
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
        self.page()
            .text("FBodyBold", 12.0, MARGIN_X, y, NAVY, &heading);
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
                let exact: Vec<&(FormAppliedSignature, EmbeddedSignature)> =
                    match slot_id.as_deref() {
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
pub(super) const INITIALS_WIDTH: f64 = 64.0;

impl Composer {
    /// One signer's block: the label, three rules, and either the anchor rectangles a provider places signatures in, or
    /// — when the brokerage has already signed this slot locally — the signature, its date and its initials.
    pub(super) fn draw_signature_block(
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
                self.page().text(
                    "FSans",
                    8.6,
                    date_rect.x + 2.0,
                    date_rect.y + 5.0,
                    INK,
                    &text,
                );
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

    pub(super) fn add_anchor(
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
pub(super) struct SignatureImage {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) components: u8,
    pub(super) samples: Vec<u8>,
    pub(super) alpha: Option<Vec<u8>>,
}

impl SignatureImage {
    /// Decode the two formats the protected media store accepts.
    ///
    /// PNG keeps its transparency as a soft mask; a JPEG has none, so it is drawn as it is. Both are decoded through the
    /// `image` crate rather than passed through in their own encoding, so the renderer has exactly one image path.
    pub(super) fn from_bytes(
        mime: model::forms_applied_signature::AppliedSignatureImageMimeType,
        bytes: &[u8],
    ) -> Result<Self, PdfError> {
        use model::forms_applied_signature::AppliedSignatureImageMimeType;
        let decoded = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| {
                PdfError::new(format!("the signature image could not be read: {error}"))
            })?
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
        Some(logo) => {
            Some(pdf.image(logo.width, logo.height, 3, &logo.colour, Some(&logo.alpha))?)
        }
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
        let image = SignatureImage::from_bytes(signature.image_mime_type, &signature.image_bytes)?;
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
