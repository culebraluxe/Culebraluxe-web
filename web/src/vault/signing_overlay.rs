//! Field overlay for the signed PDF: recipient values drawn onto the
//! original document bytes at their percentage geometry.
//!
//! The Forms pipeline only GENERATES pdfs; it cannot edit one. lopdf parses
//! the original, appends one content stream per touched page (Helvetica
//! base-14, WinAnsi-encoded, navy ink), and saves. Page order comes from
//! `get_pages`, which traverses the page tree in document order, so field
//! page numbers stay meaningful on real documents.

use lopdf::{dictionary, Document, Object, Stream};

/// One drawable value: PDF points are derived from the page's own MediaBox.
pub struct OverlayField {
    /// 1-based page number in document order.
    pub page_number: i32,
    /// 0–100, top-left origin, as stored on `signature_field`.
    pub x_percent: f64,
    pub y_percent: f64,
    /// The box the value belongs in, as a percent of the page. Zero falls back to a text line at the anchor.
    pub width_percent: f64,
    pub height_percent: f64,
    /// A signature is drawn as one: italic serif on a rule, with a caption.
    pub signature: bool,
    pub text: String,
}

/// How many pages a PDF has, or `None` when the bytes are not a readable PDF.
pub fn page_count(bytes: &[u8]) -> Option<u32> {
    Document::load_mem(bytes)
        .ok()
        .map(|document| document.get_pages().len() as u32)
        .filter(|count| *count > 0)
}

fn navy() -> &'static str {
    "0.012 0.059 0.137"
}

/// Draw every field onto a copy of `original`, returning fresh PDF bytes.
/// Pure apart from parsing: no I/O, no fonts to embed (base-14 Helvetica
/// lives in every reader), WinAnsi-encoded like the Forms pipeline.
pub fn overlay_fields(original: &[u8], fields: &[OverlayField]) -> Result<Vec<u8>, String> {
    let mut document = Document::load_mem(original)
        .map_err(|error| format!("original PDF unreadable: {error}"))?;
    let pages = document.get_pages();
    if pages.is_empty() {
        return Err("original PDF has no pages".into());
    }

    // Group field indices by page so each touched page grows one stream.
    let mut by_page: std::collections::BTreeMap<i32, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, field) in fields.iter().enumerate() {
        if !pages.contains_key(&(field.page_number as u32)) {
            return Err(format!(
                "field targets page {} of {}",
                field.page_number,
                pages.len()
            ));
        }
        by_page.entry(field.page_number).or_default().push(index);
    }

    for (page_number, indices) in &by_page {
        let page_id = pages[&(*page_number as u32)];
        let (width, height) = page_size(&document, page_id)?;
        let mut operations: Vec<u8> = Vec::new();
        for index in indices {
            let field = &fields[*index];
            let x = field.x_percent.clamp(0.0, 100.0) / 100.0 * width;
            // Percentage geometry runs top-left; PDF runs bottom-left.
            let top = (1.0 - field.y_percent.clamp(0.0, 100.0) / 100.0) * height;
            let box_width = field.width_percent.clamp(0.0, 100.0) / 100.0 * width;
            let box_height = field.height_percent.clamp(0.0, 100.0) / 100.0 * height;
            let text = encoded(&field.text);
            if field.signature && box_width > 0.0 && box_height > 0.0 {
                let bottom = top - box_height;
                let size = (box_height * 0.6).clamp(12.0, 20.0);
                // The rule, the name above it, a caption below it.
                operations.extend_from_slice(
                    format!(
                        "{} RG 0.6 w {} {} m {} {} l S\n",
                        navy(),
                        point(x),
                        point(bottom + 3.0),
                        point(x + box_width),
                        point(bottom + 3.0),
                    )
                    .as_bytes(),
                );
                operations.extend_from_slice(
                    format!(
                        "BT /DSF2 {} Tf {} rg {} {} Td (",
                        point(size),
                        navy(),
                        point(x + 2.0),
                        point(bottom + 7.0),
                    )
                    .as_bytes(),
                );
                operations.extend_from_slice(&crate::vault::pdf::escape_bytes(&text));
                operations.extend_from_slice(b") Tj ET\n");
                operations.extend_from_slice(
                    format!(
                        "BT /DSF1 6 Tf 0.45 0.5 0.58 rg {} {} Td (Electronically signed) Tj ET\n",
                        point(x + 2.0),
                        point(bottom - 4.0),
                    )
                    .as_bytes(),
                );
            } else {
                // A value sits on the box's text line (or at the anchor when the box is unknown).
                let baseline = if box_height > 0.0 {
                    top - box_height * 0.7
                } else {
                    top
                };
                operations.extend_from_slice(
                    format!(
                        "BT /DSF1 11 Tf {} rg {} {} Td (",
                        navy(),
                        point(x),
                        point(baseline),
                    )
                    .as_bytes(),
                );
                operations.extend_from_slice(&crate::vault::pdf::escape_bytes(&text));
                operations.extend_from_slice(b") Tj ET\n");
            }
        }
        let stream_id =
            document.add_object(Object::Stream(Stream::new(dictionary! {}, operations)));
        append_content(&mut document, page_id, stream_id)?;
    }

    let mut out = Vec::new();
    document
        .save_to(&mut out)
        .map_err(|error| format!("signed PDF unreadable after overlay: {error}"))?;
    Ok(out)
}

fn page_size(document: &Document, page_id: lopdf::ObjectId) -> Result<(f64, f64), String> {
    let object = document
        .get_object(page_id)
        .map_err(|error| format!("page unreadable: {error}"))?;
    let dictionary = object
        .as_dict()
        .map_err(|_| "page is not a dictionary".to_string())?;
    let media_box = dictionary
        .get(b"MediaBox")
        .map_err(|_| "page has no MediaBox".to_string())?;
    let numbers = media_box
        .as_array()
        .map_err(|_| "MediaBox is not an array".to_string())?;
    let at = |index: usize| -> Result<f64, String> {
        numbers
            .get(index)
            .and_then(|object| {
                object
                    .as_float()
                    .ok()
                    .map(f64::from)
                    .or_else(|| object.as_i64().ok().map(|n| n as f64))
            })
            .ok_or_else(|| "MediaBox is not numeric".to_string())
    };
    let (x0, y0, x1, y1) = (at(0)?, at(1)?, at(2)?, at(3)?);
    if x1 <= x0 || y1 <= y0 {
        return Err("MediaBox is empty".into());
    }
    Ok((x1 - x0, y1 - y0))
}

fn append_content(
    document: &mut Document,
    page_id: lopdf::ObjectId,
    stream_id: lopdf::ObjectId,
) -> Result<(), String> {
    // Ensure the page sees Helvetica as /DSF1, merging with whatever the
    // original Resources already carry. Reads happen before any mutation
    // so the borrow checker sees distinct phases.
    let font_ref = Object::Reference(stream_font_id(document, "Helvetica"));
    let signature_font_ref = Object::Reference(stream_font_id(document, "Times-Italic"));
    let resources_id = {
        let page = document
            .get_object(page_id)
            .map_err(|_| "page vanished".to_string())?;
        let dictionary = page
            .as_dict()
            .map_err(|_| "page is not a dictionary".to_string())?;
        match dictionary.get(b"Resources") {
            Ok(Object::Reference(id)) => Some(*id),
            Ok(_) => return Err("page Resources are inline; refusing to rewrite".into()),
            Err(_) => None,
        }
    };
    let resources_id = match resources_id {
        Some(id) => id,
        None => {
            let id = document.add_object(Object::Dictionary(dictionary! {}));
            let page = document
                .get_object_mut(page_id)
                .map_err(|_| "page vanished".to_string())?;
            page.as_dict_mut()
                .map_err(|_| "page is not a dictionary".to_string())?
                .set("Resources", Object::Reference(id));
            id
        }
    };
    let font_dict_id = {
        let resources = document
            .get_object(resources_id)
            .map_err(|_| "resources vanished".to_string())?;
        let resources = resources
            .as_dict()
            .map_err(|_| "resources are not a dictionary".to_string())?;
        match resources.get(b"Font") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };
    match font_dict_id {
        Some(id) => {
            let font_dict = document
                .get_object_mut(id)
                .map_err(|_| "font dictionary vanished".to_string())?;
            font_dict
                .as_dict_mut()
                .map_err(|_| "font dictionary is not a dictionary".to_string())?
                .set("DSF1", font_ref);
            document
                .get_object_mut(id)
                .map_err(|_| "font dictionary vanished".to_string())?
                .as_dict_mut()
                .map_err(|_| "font dictionary is not a dictionary".to_string())?
                .set("DSF2", signature_font_ref);
        }
        None => {
            let id = document.add_object(Object::Dictionary(dictionary! {
                "DSF1" => font_ref,
                "DSF2" => signature_font_ref,
            }));
            let resources = document
                .get_object_mut(resources_id)
                .map_err(|_| "resources vanished".to_string())?;
            resources
                .as_dict_mut()
                .map_err(|_| "resources are not a dictionary".to_string())?
                .set("Font", Object::Reference(id));
        }
    }
    // Append our stream to the page's Contents, whatever shape it has.
    let page = document
        .get_object_mut(page_id)
        .map_err(|_| "page vanished".to_string())?;
    let dictionary = page
        .as_dict_mut()
        .map_err(|_| "page is not a dictionary".to_string())?;
    match dictionary.get(b"Contents") {
        Ok(Object::Reference(existing)) => {
            let existing = *existing;
            dictionary.set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(existing),
                    Object::Reference(stream_id),
                ]),
            );
            Ok(())
        }
        Ok(Object::Array(existing)) => {
            let mut contents = existing.clone();
            contents.push(Object::Reference(stream_id));
            dictionary.set("Contents", Object::Array(contents));
            Ok(())
        }
        _ => {
            dictionary.set("Contents", Object::Reference(stream_id));
            Ok(())
        }
    }
}

/// A base-14 font object every touched page shares. Created once per call;
/// unreferenced on pages we never touch, which PDF readers ignore.
fn stream_font_id(document: &mut Document, base_font: &str) -> lopdf::ObjectId {
    document.add_object(Object::Dictionary(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => base_font,
        "Encoding" => "WinAnsiEncoding",
    }))
}

fn point(value: f64) -> String {
    format!("{:.2}", value)
}

/// WinAnsi bytes for literal-string operators; anything unencodable
/// becomes `?` rather than failing the whole document for one name.
fn encoded(text: &str) -> Vec<u8> {
    use model::forms_font::encode;
    let mut codes = Vec::with_capacity(text.len());
    for character in text.chars() {
        match encode(&character.to_string()) {
            Ok(mut bytes) => codes.append(&mut bytes),
            Err(_) => codes.push(b'?'),
        }
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-page original drawn with this tree's own pdf primitives, so the
    /// overlay test owns both sides of the contract.
    fn original() -> Vec<u8> {
        use crate::vault::pdf::{Content, Pdf, Rgb};
        let mut pdf = Pdf::new();
        let font = pdf.font("Helvetica");
        let tree = pdf.reserve();
        let resources = pdf.dictionary(&crate::vault::pdf::resources(&[("F1", font)], &[]));
        let mut content = Content::new();
        content.text(
            "F1",
            12.0,
            54.0,
            700.0,
            Rgb::from_bytes(3, 15, 35),
            &model::forms_font::encode("Original").unwrap(),
        );
        let page = pdf
            .page(612.0, 792.0, tree, resources, &content.into_bytes())
            .unwrap();
        let info = pdf.info("T", "A", "S", "C", "P", "D:20260101000000");
        pdf.finish(tree, &[page], Some(info)).unwrap()
    }

    #[test]
    fn overlay_draws_values_onto_the_original_pages() {
        let bytes = original();
        let out = overlay_fields(
            &bytes,
            &[OverlayField {
                page_number: 1,
                x_percent: 10.0,
                y_percent: 10.0,
                width_percent: 38.0,
                height_percent: 7.0,
                signature: true,
                text: "María Rivera".into(),
            }],
        )
        .expect("overlays");
        assert!(out.starts_with(b"%PDF-"), "still a PDF document");
        assert!(out.len() > bytes.len(), "the overlay adds content");
        // Re-parsing proves structural survival, not just a magic prefix.
        let reparsed = Document::load_mem(&out).expect("reparses");
        assert_eq!(reparsed.get_pages().len(), 1);
        // "í" is 0xED in WinAnsi, which is not UTF-8: it must reach the page as that byte, not as U+FFFD.
        assert!(
            out.windows(3).any(|w| w == [b'M', b'a', b'r'])
                && out.windows(2).any(|w| w == [b'r', 0xED]),
            "the accented name is drawn byte-exact"
        );
        assert!(!out.windows(3).any(|w| w == [0xEF, 0xBF, 0xBD]));
    }

    #[test]
    fn overlay_refuses_unknown_pages() {
        let bytes = original();
        let refused = overlay_fields(
            &bytes,
            &[OverlayField {
                page_number: 9,
                x_percent: 10.0,
                y_percent: 10.0,
                width_percent: 0.0,
                height_percent: 0.0,
                signature: false,
                text: "x".into(),
            }],
        )
        .expect_err("page 9 does not exist");
        assert!(refused.contains('9'));
    }

    #[test]
    fn overlay_refuses_unreadable_bytes() {
        assert!(overlay_fields(b"not a pdf", &[]).is_err());
    }
}
