//! Derivatives, made with `image-rs` (pure Rust, so the container needs no new binary — no libvips, no Dockerfile
//! change).
//!
//! WHY THIS EXISTS AT ALL: the original is kept and never discarded, but the original is not servable. A 13 MB
//! photograph cannot come back through the gateway's ~4.5 MB response cap any more than it could go up through the
//! ~4.5 MB request cap. So every upload also produces a `web` copy small enough to actually travel, and a `thumb`
//! for the grid. Readers get the derivative; the original stays as the record.
//!
//! Nothing here is allowed to trust its input. Decoding happens only after a header check, because a decode is where
//! a hostile or merely enormous file costs real memory in a container we do not control.

use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use std::io::Cursor;

/// The long edge the web copy is tried at, largest first. A photograph wider than this loses detail the web copy
/// does not need; a narrower one is left alone, because `thumbnail` never upscales.
const WEB_EDGES: [u32; 4] = [3600, 3200, 2800, 2400];
/// The ceiling the web copy must fit under, chosen to sit under the gateway's ~4.5 MB cap with room to spare.
const WEB_MAX_BYTES: usize = 3_500_000;
const WEB_QUALITY: u8 = 90;
const THUMB_EDGE: u32 = 400;
const THUMB_QUALITY: u8 = 85;
/// The CARD copy: what a listing card or a gallery tile shows. The web copy (3600px, 2-3 MB) is for the full-screen
/// photograph; a card sent that was ten times the bytes it needed. Measured 2026-09-28: cards were 1.6-2.7 MB each.
const CARD_EDGE: u32 = 1200;
const CARD_QUALITY: u8 = 82;
/// Refused before any decoding happens. A 13 MB JPEG is nothing; a decompression bomb is the same size and asks for
/// many gigabytes once decoded.
const MAX_PIXELS: u64 = 80_000_000;

#[derive(Debug, Clone)]
pub struct Derivative {
    pub kind: &'static str,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Turns one original into the two copies that get attached to it.
///
/// Returns `Err` with a sentence meant to be shown to a person — the caller surfaces it rather than a code, because
/// "this file is not an image we can read" is worth more to whoever chose the file than a status number.
pub fn derive_web_and_thumb(original: &[u8]) -> Result<Vec<Derivative>, String> {
    let decoded = decode(original)?;

    let thumb = fit(&decoded, THUMB_EDGE);
    let thumb_bytes = encode_jpeg(&thumb, THUMB_QUALITY)?;
    let card = card_of(&decoded)?;

    // Step down until the copy is small enough to be served. If even the smallest step is too large, the smallest
    // one is used and the caller is told, rather than failing an upload that is otherwise fine.
    let mut web = fit(&decoded, WEB_EDGES[0]);
    let mut web_bytes = encode_jpeg(&web, WEB_QUALITY)?;
    for edge in WEB_EDGES.iter().skip(1) {
        if web_bytes.len() <= WEB_MAX_BYTES {
            break;
        }
        web = fit(&decoded, *edge);
        web_bytes = encode_jpeg(&web, WEB_QUALITY)?;
    }

    Ok(vec![
        Derivative {
            kind: "web",
            width: web.width(),
            height: web.height(),
            bytes: web_bytes,
        },
        card,
        Derivative {
            kind: "thumb",
            width: thumb.width(),
            height: thumb.height(),
            bytes: thumb_bytes,
        },
    ])
}

/// Only the card copy — for photographs stored before cards existed (the backfill).
pub fn derive_card(original: &[u8]) -> Result<Derivative, String> {
    card_of(&decode(original)?)
}

fn card_of(decoded: &image::DynamicImage) -> Result<Derivative, String> {
    let card = fit(decoded, CARD_EDGE);
    let bytes = encode_jpeg(&card, CARD_QUALITY)?;
    Ok(Derivative {
        kind: "card",
        width: card.width(),
        height: card.height(),
        bytes,
    })
}

/// The original, checked and decoded, turned the way it was shot.
fn decode(original: &[u8]) -> Result<image::DynamicImage, String> {
    let reader = ImageReader::new(Cursor::new(original))
        .with_guessed_format()
        .map_err(|error| format!("this file could not be read as an image ({error})"))?;
    if reader.format().is_none() {
        return Err("this file is not an image in a format we can read".to_string());
    }

    // The guard runs on the HEADER, before a single pixel is decoded.
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("this image's dimensions could not be read ({error})"))?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "this image is {width}x{height}, too large to process safely"
        ));
    }

    let mut decoder = ImageReader::new(Cursor::new(original))
        .with_guessed_format()
        .map_err(|error| format!("this file could not be read as an image ({error})"))?
        .into_decoder()
        .map_err(|error| format!("this image could not be decoded ({error})"))?;
    // TURNED THE WAY IT WAS SHOT. A phone stores the sensor's pixels as they came off it — landscape — and a tag
    // saying how to turn them. The copies made here carry no tag, so the turn has to happen in the pixels: without it
    // every portrait photograph came out on its side.
    let orientation = image::ImageDecoder::orientation(&mut decoder)
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("this image could not be decoded ({error})"))?;
    decoded.apply_orientation(orientation);
    Ok(decoded)
}

fn fit(image: &image::DynamicImage, edge: u32) -> image::DynamicImage {
    if image.width().max(image.height()) <= edge {
        return image.clone();
    }
    image.thumbnail(edge, edge)
}

fn encode_jpeg(image: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    // JPEG stores no alpha, and `image` refuses to encode a source that has one, so the colour type is settled here
    // rather than left to fail inside the encoder.
    let rgb = image::DynamicImage::ImageRgb8(image.to_rgb8());
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&rgb)
        .map_err(|error| format!("the resized copy could not be encoded ({error})"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut image = image::RgbImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
        }
        encode_jpeg(&image::DynamicImage::ImageRgb8(image), 95).expect("the fixture encodes")
    }

    /// The fixture with an EXIF Orientation tag, as a phone writes it: 6 is "turn 90° clockwise to view".
    fn with_orientation(jpeg: Vec<u8>, orientation: u8) -> Vec<u8> {
        let tiff: Vec<u8> = [
            b"MM\x00\x2a\x00\x00\x00\x08".as_slice(),
            &[0x00, 0x01],
            &[
                0x01,
                0x12,
                0x00,
                0x03,
                0x00,
                0x00,
                0x00,
                0x01,
                0x00,
                orientation,
                0x00,
                0x00,
            ],
            &[0x00, 0x00, 0x00, 0x00],
        ]
        .concat();
        let payload = [b"Exif\x00\x00".as_slice(), &tiff].concat();
        let length = (payload.len() + 2) as u16;
        [
            &jpeg[..2],
            &[0xff, 0xe1],
            &length.to_be_bytes(),
            &payload,
            &jpeg[2..],
        ]
        .concat()
    }

    #[test]
    fn a_portrait_photograph_is_turned_upright() {
        let shot = with_orientation(jpeg(60, 40), 6);
        let copies = derive_web_and_thumb(&shot).expect("the photograph derives");
        for copy in &copies {
            assert_eq!(
                (copy.width, copy.height),
                (40, 60),
                "the {} copy is on its side",
                copy.kind
            );
            let decoded = image::load_from_memory(&copy.bytes).expect("the copy decodes");
            assert_eq!((decoded.width(), decoded.height()), (40, 60));
        }
    }

    #[test]
    fn refuses_bytes_that_are_not_an_image() {
        // The sentence is the product here: whoever chose the file has to be able to act on it.
        let error = derive_web_and_thumb(b"this is not a photograph").expect_err("no image");
        assert!(
            error.contains("not an image"),
            "unexpected message: {error}"
        );
    }

    #[test]
    fn a_photograph_that_already_fits_is_copied_not_enlarged() {
        let original = jpeg(2000, 1400);
        let derived = derive_web_and_thumb(&original).expect("derivatives");

        assert_eq!(derived.len(), 3);
        let web = derived.iter().find(|d| d.kind == "web").expect("web copy");
        let thumb = derived.iter().find(|d| d.kind == "thumb").expect("thumb");

        // Never upscaled: a copy larger than its source would be invented detail.
        assert_eq!((web.width, web.height), (2000, 1400));
        assert!(web.bytes.len() <= WEB_MAX_BYTES);
        assert!(thumb.width.max(thumb.height) <= THUMB_EDGE);
        // Both are real JPEGs, not empty buffers.
        assert!(web.bytes.starts_with(&[0xFF, 0xD8]));
        assert!(thumb.bytes.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn a_large_photograph_steps_down_until_it_can_be_served() {
        let original = jpeg(4000, 2600);
        let derived = derive_web_and_thumb(&original).expect("derivatives");
        let web = derived.iter().find(|d| d.kind == "web").expect("web copy");

        assert!(
            web.width.max(web.height) <= WEB_EDGES[0],
            "web copy is {}px, wider than the largest step",
            web.width.max(web.height)
        );
        assert!(web.bytes.len() <= WEB_MAX_BYTES);
    }
}
