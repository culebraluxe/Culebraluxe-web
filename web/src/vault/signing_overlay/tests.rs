// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
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
            kind: OverlayKind::Signature,
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
            kind: OverlayKind::Signature,
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
        kind: OverlayKind::Signature,
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
    let out =
        overlay_fields(&bytes, &[field(Some(signature_png(600, 150)), true)]).expect("overlays");
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
            kind: OverlayKind::Signature,
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

#[test]
fn keeping_pages_leaves_only_those_and_refuses_a_page_that_is_not_there() {
    let bytes = original();
    let kept = keep_pages(&bytes, &[1]).expect("keeps page one");
    assert_eq!(page_count(&kept), Some(1));
    assert!(keep_pages(&bytes, &[7]).is_err());
}

/// A block the TEMPLATE placed is drawn at the brokerage's own sizes (the pre-signature's): initials in bold 10.5pt,
/// the date in 8.6pt, the signature picture fitted inside the block less a margin. One size for every party.
#[test]
fn a_template_block_is_drawn_at_the_brokerages_own_sizes() {
    let bytes = original();
    let block = |kind, text: &str, image| OverlayField {
        kind,
        page_number: 1,
        x_percent: 10.0,
        y_percent: 60.0,
        width_percent: 30.0,
        height_percent: 4.0,
        signature: kind == OverlayKind::Signature,
        style: 0,
        caption: None,
        text: text.into(),
        image,
        ruled: false,
    };
    let out = overlay_fields(
        &bytes,
        &[
            block(
                OverlayKind::Signature,
                "Ada Alvarez",
                Some(signature_png(1000, 260)),
            ),
            block(OverlayKind::Initials, "AA", None),
            block(OverlayKind::Date, "October 7, 2026", None),
        ],
    )
    .expect("overlays");
    let document = Document::load_mem(&out).unwrap();
    let page = *document.get_pages().get(&1).unwrap();
    let content = String::from_utf8_lossy(&document.get_page_content(page)).into_owned();
    assert!(
        content.contains("/DSF4 10.50 Tf"),
        "initials: Helvetica-Bold 10.5: {content}"
    );
    assert!(
        content.contains("/DSF1 8.60 Tf"),
        "date: Helvetica 8.6: {content}"
    );
    assert!(
        content.contains("0.11 0.12 0.14 rg") || content.contains("0.110 0.122 0.137 rg"),
        "the Forms ink"
    );
    // The picture is fitted into (block - margin), 4pt in: 30% of 612 = 183.6pt wide, 4% of 792 = 31.68pt tall.
    let fitted_height = 31.68 - 2.0;
    assert!(
        content.contains(&format!(
            "{:.2} 0 0 {:.2}",
            fitted_height * 1000.0 / 260.0,
            fitted_height
        )),
        "{content}"
    );
}
