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
    /// The signer's chosen appearance: 0 classic (italic), 1 refined (italic, open letter-spacing), 2 bold italic.
    pub style: u8,
    /// The small line under a signature ("Electronically signed 2026-10-07").
    pub caption: Option<String>,
    pub text: String,
    /// The signer's own signature, initials or mark as a PNG (what they saw on the signing page): drawn instead of the
    /// typeset `text`, fitted into the block. A PNG that cannot be read falls back to the text.
    pub image: Option<Vec<u8>>,
    /// Draw the signature rule and caption under an image. A block the TEMPLATE placed already prints its own line;
    /// one placed by default has nothing printed under it.
    pub ruled: bool,
}

/// The largest signature picture the seal accepts, in pixels: a drawn or typed mark never needs more.
pub const MAX_SIGNATURE_PIXELS: (u32, u32) = (2400, 1000);

/// Check that `bytes` are a PNG of a sensible size and decode them. Used at the door (a signer's answer) and at the
/// seal, so what is stored is always something the seal can draw.
pub fn decode_signature_png(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("the signature picture is not a PNG".into());
    }
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|error| format!("the signature picture could not be read: {error}"))?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    if width == 0
        || height == 0
        || width > MAX_SIGNATURE_PIXELS.0
        || height > MAX_SIGNATURE_PIXELS.1
    {
        return Err(format!(
            "the signature picture is {width}x{height}; at most {}x{} is accepted",
            MAX_SIGNATURE_PIXELS.0, MAX_SIGNATURE_PIXELS.1
        ));
    }
    Ok(decoded)
}

/// Add a decoded signature picture to the document as an image with a soft mask (its transparency), returning the
/// object and its pixel size.
fn embed_signature_image(
    document: &mut Document,
    decoded: &image::RgbaImage,
) -> (lopdf::ObjectId, u32, u32) {
    let (width, height) = decoded.dimensions();
    let mut colour = Vec::with_capacity((width * height * 3) as usize);
    let mut alpha = Vec::with_capacity((width * height) as usize);
    for pixel in decoded.pixels() {
        colour.extend_from_slice(&pixel.0[0..3]);
        alpha.push(pixel.0[3]);
    }
    let mut mask = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(width),
            "Height" => i64::from(height),
            "ColorSpace" => "DeviceGray",
            "BitsPerComponent" => 8,
        },
        alpha,
    );
    let _ = mask.compress();
    let mask_id = document.add_object(Object::Stream(mask));
    let mut image = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(width),
            "Height" => i64::from(height),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "SMask" => Object::Reference(mask_id),
        },
        colour,
    );
    let _ = image.compress();
    (document.add_object(Object::Stream(image)), width, height)
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
        // The pictures this page needs, embedded once each and named for the content stream.
        let mut page_images: Vec<(String, lopdf::ObjectId)> = Vec::new();
        let mut placed: std::collections::BTreeMap<usize, (String, u32, u32)> =
            std::collections::BTreeMap::new();
        for index in indices {
            if let Some(bytes) = fields[*index].image.as_deref() {
                if let Ok(decoded) = decode_signature_png(bytes) {
                    let (id, w, h) = embed_signature_image(&mut document, &decoded);
                    let name = format!("DSIm{index}");
                    page_images.push((name.clone(), id));
                    placed.insert(*index, (name, w, h));
                }
            }
        }
        for index in indices {
            let field = &fields[*index];
            let x = field.x_percent.clamp(0.0, 100.0) / 100.0 * width;
            // Percentage geometry runs top-left; PDF runs bottom-left.
            let top = (1.0 - field.y_percent.clamp(0.0, 100.0) / 100.0) * height;
            let box_width = field.width_percent.clamp(0.0, 100.0) / 100.0 * width;
            let box_height = field.height_percent.clamp(0.0, 100.0) / 100.0 * height;
            let text = encoded(&field.text);
            if let (Some((name, px_w, px_h)), true) =
                (placed.get(index), box_width > 0.0 && box_height > 0.0)
            {
                // The signer's own picture, as large as the block allows without distortion, standing on the line.
                let bottom = top - box_height;
                let aspect = f64::from(*px_w) / f64::from(*px_h);
                let mut draw_h = box_height;
                let mut draw_w = draw_h * aspect;
                if draw_w > box_width {
                    draw_w = box_width;
                    draw_h = draw_w / aspect;
                }
                let lift = if field.ruled {
                    4.0
                } else {
                    (box_height - draw_h) / 2.0
                };
                operations.extend_from_slice(
                    format!(
                        "q {} 0 0 {} {} {} cm /{name} Do Q\n",
                        point(draw_w),
                        point(draw_h),
                        point(x + 1.0),
                        point(bottom + lift),
                    )
                    .as_bytes(),
                );
                if field.ruled {
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
                    let caption =
                        encoded(field.caption.as_deref().unwrap_or("Electronically signed"));
                    operations.extend_from_slice(
                        format!(
                            "BT /DSF1 6 Tf 0.45 0.5 0.58 rg {} {} Td (",
                            point(x + 2.0),
                            point(bottom - 4.0),
                        )
                        .as_bytes(),
                    );
                    operations.extend_from_slice(&crate::vault::pdf::escape_bytes(&caption));
                    operations.extend_from_slice(b") Tj ET\n");
                }
            } else if field.signature && box_width > 0.0 && box_height > 0.0 {
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
                let (font, spacing) = match field.style {
                    1 => ("DSF2", 0.9),
                    2 => ("DSF3", 0.0),
                    _ => ("DSF2", 0.0),
                };
                operations.extend_from_slice(
                    format!(
                        "BT /{font} {} Tf {} Tc {} rg {} {} Td (",
                        point(size),
                        point(spacing),
                        navy(),
                        point(x + 2.0),
                        point(bottom + 9.5),
                    )
                    .as_bytes(),
                );
                operations.extend_from_slice(&crate::vault::pdf::escape_bytes(&text));
                operations.extend_from_slice(b") Tj ET\n");
                let caption = encoded(field.caption.as_deref().unwrap_or("Electronically signed"));
                operations.extend_from_slice(
                    format!(
                        "BT /DSF1 6 Tf 0.45 0.5 0.58 rg {} {} Td (",
                        point(x + 2.0),
                        point(bottom - 4.0),
                    )
                    .as_bytes(),
                );
                operations.extend_from_slice(&crate::vault::pdf::escape_bytes(&caption));
                operations.extend_from_slice(b") Tj ET\n");
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
        append_content(&mut document, page_id, stream_id, &page_images)?;
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
    images: &[(String, lopdf::ObjectId)],
) -> Result<(), String> {
    // Ensure the page sees Helvetica as /DSF1, merging with whatever the
    // original Resources already carry. Reads happen before any mutation
    // so the borrow checker sees distinct phases.
    let font_ref = Object::Reference(stream_font_id(document, "Helvetica"));
    let signature_font_ref = Object::Reference(stream_font_id(document, "Times-Italic"));
    let bold_signature_font_ref = Object::Reference(stream_font_id(document, "Times-BoldItalic"));
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
    // The fonts the overlay draws with are ADDED to the page's own `/Font` dictionary (in whatever form the original wrote
    // it: indirect or inline). Replacing it would take the document's own fonts away, and every original word with them.
    merge_resource_entries(
        document,
        resources_id,
        "Font",
        vec![
            ("DSF1".to_owned(), font_ref),
            ("DSF2".to_owned(), signature_font_ref),
            ("DSF3".to_owned(), bold_signature_font_ref),
        ],
    )?;
    if !images.is_empty() {
        merge_resource_entries(
            document,
            resources_id,
            "XObject",
            images
                .iter()
                .map(|(name, id)| (name.clone(), Object::Reference(*id)))
                .collect(),
        )?;
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

/// Add `entries` to the resource sub-dictionary `key` (`Font`, `XObject`), keeping everything the original already named
/// there. The sub-dictionary may be an indirect object, an inline dictionary, or absent; each is handled in place.
fn merge_resource_entries(
    document: &mut Document,
    resources_id: lopdf::ObjectId,
    key: &str,
    entries: Vec<(String, Object)>,
) -> Result<(), String> {
    enum Slot {
        Indirect(lopdf::ObjectId),
        Inline(lopdf::Dictionary),
        Absent,
    }
    let slot = {
        let resources = document
            .get_object(resources_id)
            .map_err(|_| "resources vanished".to_string())?
            .as_dict()
            .map_err(|_| "resources are not a dictionary".to_string())?;
        match resources.get(key.as_bytes()) {
            Ok(Object::Reference(id)) => Slot::Indirect(*id),
            Ok(Object::Dictionary(inline)) => Slot::Inline(inline.clone()),
            _ => Slot::Absent,
        }
    };
    match slot {
        Slot::Indirect(id) => {
            let dictionary = document
                .get_object_mut(id)
                .map_err(|_| format!("{key} dictionary vanished"))?
                .as_dict_mut()
                .map_err(|_| format!("{key} dictionary is not a dictionary"))?;
            for (name, object) in entries {
                dictionary.set(name, object);
            }
        }
        Slot::Inline(mut dictionary) => {
            for (name, object) in entries {
                dictionary.set(name, object);
            }
            document
                .get_object_mut(resources_id)
                .map_err(|_| "resources vanished".to_string())?
                .as_dict_mut()
                .map_err(|_| "resources are not a dictionary".to_string())?
                .set(key, Object::Dictionary(dictionary));
        }
        Slot::Absent => {
            let mut dictionary = lopdf::Dictionary::new();
            for (name, object) in entries {
                dictionary.set(name, object);
            }
            document
                .get_object_mut(resources_id)
                .map_err(|_| "resources vanished".to_string())?
                .as_dict_mut()
                .map_err(|_| "resources are not a dictionary".to_string())?
                .set(key, Object::Dictionary(dictionary));
        }
    }
    Ok(())
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
                style: 2,
                caption: Some("Electronically signed 2026-10-07".into()),
                text: "María Rivera".into(),
                image: None,
                ruled: true,
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
                style: 0,
                caption: None,
                text: "x".into(),
                image: None,
                ruled: false,
            }],
        )
        .expect_err("page 9 does not exist");
        assert!(refused.contains('9'));
    }

    #[test]
    fn overlay_refuses_unreadable_bytes() {
        assert!(overlay_fields(b"not a pdf", &[]).is_err());
    }

    /// A small PNG with a transparent background and one dark stroke: stands in for a typed or drawn signature.
    fn signature_png(width: u32, height: u32) -> Vec<u8> {
        let mut canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
        for x in 0..width {
            canvas.put_pixel(x, height / 2, image::Rgba([3, 15, 35, 255]));
        }
        let mut out = Vec::new();
        canvas
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encodes");
        out
    }

    #[test]
    fn a_signature_picture_is_embedded_as_an_image_with_a_soft_mask() {
        let bytes = original();
        let field = |image: Option<Vec<u8>>, ruled: bool| OverlayField {
            page_number: 1,
            x_percent: 10.0,
            y_percent: 70.0,
            width_percent: 40.0,
            height_percent: 5.0,
            signature: true,
            style: 0,
            caption: Some("Electronically signed July 3, 2026".into()),
            text: "María Rivera".into(),
            image,
            ruled,
        };
        let out = overlay_fields(&bytes, &[field(Some(signature_png(600, 150)), true)])
            .expect("overlays");
        let document = Document::load_mem(&out).expect("reparses");
        let page = *document.get_pages().get(&1).unwrap();
        let images: Vec<_> = document
            .get_page_images(page)
            .expect("the page lists its images");
        assert_eq!(
            images.len(),
            1,
            "the signature is a real picture on the page"
        );
        assert_eq!((images[0].width, images[0].height), (600, 150));
        // With no picture the typeset fallback still draws, and nothing is embedded.
        let plain = overlay_fields(&bytes, &[field(None, true)]).expect("overlays");
        let plain = Document::load_mem(&plain).unwrap();
        assert!(plain
            .get_page_images(*plain.get_pages().get(&1).unwrap())
            .unwrap()
            .is_empty());
        // A picture that is not a PNG falls back to the text rather than failing the seal.
        let broken = overlay_fields(&bytes, &[field(Some(b"not a png".to_vec()), true)]).unwrap();
        let broken = Document::load_mem(&broken).unwrap();
        assert!(broken
            .get_page_images(*broken.get_pages().get(&1).unwrap())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_door_refuses_what_is_not_a_sensible_png() {
        assert!(decode_signature_png(&signature_png(600, 150)).is_ok());
        assert!(decode_signature_png(b"GIF89a....")
            .unwrap_err()
            .contains("not a PNG"));
        let huge = signature_png(MAX_SIGNATURE_PIXELS.0 + 1, 10);
        assert!(decode_signature_png(&huge).unwrap_err().contains("at most"));
        // A PNG header with garbage after it does not decode.
        let mut truncated = signature_png(60, 20);
        truncated.truncate(40);
        assert!(decode_signature_png(&truncated).is_err());
    }

    /// The seal ADDS fonts and pictures to the page; it must never take the original's own away. The original's text
    /// ("Original", in /F1) has to still resolve its font afterwards — a replaced font dictionary printed a signed
    /// agreement with every word of the agreement missing.
    #[test]
    fn the_originals_own_fonts_survive_the_seal() {
        let bytes = original();
        let out = overlay_fields(
            &bytes,
            &[OverlayField {
                page_number: 1,
                x_percent: 10.0,
                y_percent: 70.0,
                width_percent: 40.0,
                height_percent: 5.0,
                signature: true,
                style: 0,
                caption: None,
                text: "María Rivera".into(),
                image: Some(signature_png(600, 150)),
                ruled: true,
            }],
        )
        .expect("overlays");
        let document = Document::load_mem(&out).expect("reparses");
        let page = *document.get_pages().get(&1).unwrap();
        let fonts: Vec<String> = document
            .get_page_fonts(page)
            .expect("the page lists its fonts")
            .keys()
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect();
        assert!(
            fonts.contains(&"F1".to_owned()),
            "the original's font is still there: {fonts:?}"
        );
        assert!(
            fonts.contains(&"DSF1".to_owned()),
            "and the overlay's was added beside it: {fonts:?}"
        );
        // The page's own text is still readable through a parser.
        let text = document.extract_text(&[1]).unwrap_or_default();
        assert!(
            text.contains("Original"),
            "the original's words survive: {text:?}"
        );
    }
}
