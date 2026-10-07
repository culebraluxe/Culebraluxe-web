//! A small PDF writer: objects, streams, pages, base-14 fonts, images — and nothing else.
//!
//! WHY IT EXISTS. The form renderer must produce the document bytes in Rust: composition is the one piece of the forms
//! path that was never ported. The vault's artifact port is attached in five places with a stub answering "Vault artifact
//! renderer is not configured on this transport", while everything else — the command that versions the document, the
//! participants, the receipts, the signature transport — is already here.
//!
//! WHAT IT IS NOT: a PDF library. It writes what the renderer draws — text in the four standard fonts, coloured rules and
//! panels, images, page labels — in plain object syntax with Flate-compressed streams. No font embedding (base-14 fonts
//! live in the reader by design), no subsetting, no encryption, no interactive forms.
//!
//! COORDINATES ARE PDF'S: origin bottom-left, y growing upward — which is what the layout tokens and every y value in the
//! TypeScript composer already assume.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fmt::Write as _;
use std::io::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfError {
    pub message: String,
}

impl PdfError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PdfError {}

/// A colour as the layout declares it: three components in 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

impl Rgb {
    /// From the 0..=255 triple the brand tokens are written in.
    pub const fn from_bytes(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red: red as f64 / 255.0,
            green: green as f64 / 255.0,
            blue: blue as f64 / 255.0,
        }
    }

    pub(crate) fn operator(&self) -> String {
        format!(
            "{} {} {}",
            decimal(self.red),
            decimal(self.green),
            decimal(self.blue)
        )
    }
}

/// A number as PDF wants it: no exponent, no trailing noise, never `-0`.
pub fn decimal(value: f64) -> String {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    let mut text = format!("{rounded:.4}");
    while text.contains('.') && (text.ends_with('0') || text.ends_with('.')) {
        text.pop();
    }
    if text.is_empty() || text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

/// The object writer. References are 1-based, as PDF numbers them.
#[derive(Debug, Default)]
pub struct Pdf {
    objects: Vec<Vec<u8>>,
}

impl Pdf {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    fn add(&mut self, body: Vec<u8>) -> u32 {
        self.objects.push(body);
        self.objects.len() as u32
    }

    /// Reserve a reference now and fill it later: pages are written before the node that lists them, so the page tree's
    /// own reference has to exist first.
    pub fn reserve(&mut self) -> u32 {
        self.add(Vec::new())
    }

    pub fn replace(&mut self, reference: u32, body: Vec<u8>) {
        if let Some(slot) = self.objects.get_mut(reference as usize - 1) {
            *slot = body;
        }
    }

    /// A base-14 font with WinAnsi encoding. Not embedded: those fourteen fonts live in the reader.
    /// A plain DICTIONARY object.
    ///
    /// `/Resources` MUST POINT AT ONE. Written as a stream, the entry is unreadable to a viewer: it then resolves no fonts
    /// and no images, and draws only the operators that need neither — the rules and the boxes, with every glyph and the
    /// wordmark missing. That is what "the page is white with gold and grey boxes" is.
    pub fn dictionary(&mut self, body: &str) -> u32 {
        self.add(body.as_bytes().to_vec())
    }

    pub fn font(&mut self, base_font: &str) -> u32 {
        self.add(
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{base_font} /Encoding /WinAnsiEncoding >>"
            )
            .into_bytes(),
        )
    }

    pub(crate) fn stream(&mut self, dictionary_extra: &str, data: &[u8]) -> Result<u32, PdfError> {
        let compressed = deflate(data)?;
        let mut body: Vec<u8> = Vec::with_capacity(compressed.len() + 128);
        {
            let mut text = TextBuffer { buffer: &mut body };
            write!(
                text,
                "<< /Length {} /Filter /FlateDecode {dictionary_extra} >>\nstream\n",
                compressed.len()
            )
            .map_err(|error| PdfError::new(error.to_string()))?;
        }
        body.extend_from_slice(&compressed);
        body.extend_from_slice(b"\nendstream");
        Ok(self.add(body))
    }

    /// An image XObject. `alpha`, when present, becomes a soft mask, so transparency survives.
    pub fn image(
        &mut self,
        width: u32,
        height: u32,
        components: u8,
        samples: &[u8],
        alpha: Option<&[u8]>,
    ) -> Result<u32, PdfError> {
        let expected = width as usize * height as usize * components as usize;
        if samples.len() != expected {
            return Err(PdfError::new(format!(
                "image samples are {} bytes but {width}x{height}x{components} needs {expected}",
                samples.len()
            )));
        }
        let colour_space = match components {
            1 => "/DeviceGray",
            3 => "/DeviceRGB",
            other => {
                return Err(PdfError::new(format!(
                    "unsupported image component count {other}"
                )))
            }
        };
        let mask = match alpha {
            None => String::new(),
            Some(alpha) => {
                if alpha.len() != width as usize * height as usize {
                    return Err(PdfError::new(
                        "the alpha channel is not one byte per pixel".to_string(),
                    ));
                }
                let mask_reference = self.image(width, height, 1, alpha, None)?;
                format!(" /SMask {mask_reference} 0 R")
            }
        };
        let extra = format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace {colour_space} /BitsPerComponent 8{mask}"
        );
        self.stream(&extra, samples)
    }

    /// One page. `parent` is the reserved page-tree reference; `content` is the page's content stream.
    pub fn page(
        &mut self,
        width: f64,
        height: f64,
        parent: u32,
        resources: u32,
        content: &[u8],
    ) -> Result<u32, PdfError> {
        let contents = self.stream("", content)?;
        let body = format!(
            "<< /Type /Page /Parent {parent} 0 R /MediaBox [0 0 {} {}] /Resources {resources} 0 R /Contents {contents} 0 R >>",
            decimal(width),
            decimal(height)
        );
        Ok(self.add(body.into_bytes()))
    }

    /// The document information dictionary. `at` is a PDF date string (`D:YYYYMMDDHHmmSSZ`).
    pub fn info(
        &mut self,
        title: &str,
        author: &str,
        subject: &str,
        creator: &str,
        producer: &str,
        at: &str,
    ) -> u32 {
        let body = format!(
            "<< /Title ({}) /Author ({}) /Subject ({}) /Creator ({}) /Producer ({}) /CreationDate ({at}) /ModDate ({at}) >>",
            escape_text(title),
            escape_text(author),
            escape_text(subject),
            escape_text(creator),
            escape_text(producer)
        );
        self.add(body.into_bytes())
    }

    /// Fill the reserved page-tree reference, write the catalog, and serialise the whole document with its
    /// cross-reference table and trailer.
    pub fn finish(
        &mut self,
        page_tree: u32,
        pages: &[u32],
        info: Option<u32>,
    ) -> Result<Vec<u8>, PdfError> {
        let kids: String = pages
            .iter()
            .map(|reference| format!("{reference} 0 R "))
            .collect();
        self.replace(
            page_tree,
            format!("<< /Type /Pages /Kids [ {kids}] /Count {} >>", pages.len()).into_bytes(),
        );
        let catalog = self.add(format!("<< /Type /Catalog /Pages {page_tree} 0 R >>").into_bytes());

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets: Vec<usize> = Vec::with_capacity(self.objects.len() + 1);
        offsets.push(0);
        for (index, object) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            let mut text = TextBuffer { buffer: &mut out };
            write!(text, "{} 0 obj\n", index + 1)
                .map_err(|error| PdfError::new(error.to_string()))?;
            drop(text);
            out.extend_from_slice(object);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_at = out.len();
        {
            let mut text = TextBuffer { buffer: &mut out };
            write!(
                text,
                "xref\n0 {}\n0000000000 65535 f \n",
                self.objects.len() + 1
            )
            .map_err(|error| PdfError::new(error.to_string()))?;
            for offset in offsets.iter().skip(1) {
                write!(text, "{offset:010} 00000 n \n")
                    .map_err(|error| PdfError::new(error.to_string()))?;
            }
            let info_entry = match info {
                Some(reference) => format!(" /Info {reference} 0 R"),
                None => String::new(),
            };
            write!(
                text,
                "trailer\n<< /Size {} /Root {catalog} 0 R{info_entry} >>\nstartxref\n{xref_at}\n%%EOF\n",
                self.objects.len() + 1
            )
            .map_err(|error| PdfError::new(error.to_string()))?;
        }
        Ok(out)
    }
}

/// Escape a literal string for a PDF literal object: backslash and the two parentheses. WinAnsi bytes above 127 are
/// written raw, which is precisely what that encoding is for.
pub fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

/// Escape already-encoded text bytes for a text-showing operator.
pub fn escape_bytes(codes: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(codes.len());
    for byte in codes {
        match *byte {
            b'\\' | b'(' | b')' => {
                out.push(b'\\');
                out.push(*byte);
            }
            0x0a | 0x0d => out.push(b' '),
            other => out.push(other),
        }
    }
    out
}

/// The resource dictionary body: the fonts the document draws with, and any images.
pub fn resources(fonts: &[(&str, u32)], images: &[(&str, u32)]) -> String {
    let mut font_entries = String::new();
    for (name, reference) in fonts {
        let _ = write!(font_entries, "/{name} {reference} 0 R ");
    }
    let mut image_entries = String::new();
    for (name, reference) in images {
        let _ = write!(image_entries, "/{name} {reference} 0 R ");
    }
    if image_entries.is_empty() {
        format!("<< /Font << {font_entries}>> >>")
    } else {
        format!("<< /Font << {font_entries}>> /XObject << {image_entries}>> >>")
    }
}

/// Flate, at the best ratio: an issued document is stored and shipped, and this is the only compression it gets.
fn deflate(data: &[u8]) -> Result<Vec<u8>, PdfError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(data)
        .map_err(|error| PdfError::new(format!("could not compress a stream: {error}")))?;
    encoder
        .finish()
        .map_err(|error| PdfError::new(format!("could not compress a stream: {error}")))
}

/// Writing ASCII into a byte buffer — all this writer emits outside a stream's payload.
struct TextBuffer<'a> {
    buffer: &'a mut Vec<u8>,
}

impl std::fmt::Write for TextBuffer<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.buffer.extend_from_slice(text.as_bytes());
        Ok(())
    }
}

/// The drawing operations, in one place so a layout reads as layout.
#[derive(Debug, Default)]
pub struct Content {
    /// Bytes, not a `String`: WinAnsi text above 127 (é, ñ, ·) is not UTF-8, and a lossy conversion turns each such
    /// glyph into U+FFFD.
    operators: Vec<u8>,
}

impl Content {
    pub fn new() -> Self {
        Self {
            operators: Vec::new(),
        }
    }

    /// Text at a baseline position, already encoded to WinAnsi.
    pub fn text(&mut self, font: &str, size: f64, x: f64, y: f64, colour: Rgb, codes: &[u8]) {
        let _ = write!(
            self.operators,
            "BT /{font} {} Tf {} rg {} {} Td (",
            decimal(size),
            colour.operator(),
            decimal(x),
            decimal(y)
        );
        self.operators.extend_from_slice(&escape_bytes(codes));
        self.operators.extend_from_slice(b") Tj ET\n");
    }

    pub fn line(&mut self, from: (f64, f64), to: (f64, f64), thickness: f64, colour: Rgb) {
        let _ = write!(
            self.operators,
            "{} RG {} w {} {} m {} {} l S\n",
            colour.operator(),
            decimal(thickness),
            decimal(from.0),
            decimal(from.1),
            decimal(to.0),
            decimal(to.1)
        );
    }

    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64, colour: Rgb) {
        let _ = write!(
            self.operators,
            "{} rg {} {} {} {} re f\n",
            colour.operator(),
            decimal(x),
            decimal(y),
            decimal(width),
            decimal(height)
        );
    }

    pub fn image(&mut self, name: &str, x: f64, y: f64, width: f64, height: f64) {
        let _ = write!(
            self.operators,
            "q {} 0 0 {} {} {} cm /{name} Do Q\n",
            decimal(width),
            decimal(height),
            decimal(x),
            decimal(y)
        );
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.operators
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::forms_font::encode;

    /// The real brand logo, decoded and split into colour and alpha — the same asset the TypeScript renderer embeds.
    fn logo() -> Option<(u32, u32, Vec<u8>, Vec<u8>)> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/brand/CLLOGO.png");
        let bytes = std::fs::read(path).ok()?;
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
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
        Some((width, height, colour, alpha))
    }

    /// A one-page document using every primitive the form renderer needs: four fonts, text, a rule, a panel, the logo.
    fn document() -> Vec<u8> {
        let mut pdf = Pdf::new();
        let body = pdf.font("Times-Roman");
        let body_bold = pdf.font("Times-Bold");
        let sans = pdf.font("Helvetica");
        let sans_bold = pdf.font("Helvetica-Bold");
        let page_tree = pdf.reserve();

        let mut images: Vec<(&str, u32)> = Vec::new();
        if let Some((width, height, colour, alpha)) = logo() {
            let reference = pdf
                .image(width, height, 3, &colour, Some(&alpha))
                .expect("the logo encodes as an image");
            images.push(("Im1", reference));
        }
        let fonts = [
            ("FBody", body),
            ("FBodyBold", body_bold),
            ("FSans", sans),
            ("FSansBold", sans_bold),
        ];
        let resource_body = resources(&fonts, &images);
        let resources_ref = pdf
            .stream("", resource_body.as_bytes())
            .expect("the resource dictionary writes");

        let navy = Rgb::from_bytes(3, 15, 35);
        let gold = Rgb::from_bytes(198, 161, 91);
        let ink = Rgb::from_bytes(28, 31, 35);
        let mut content = Content::new();
        content.text(
            "FSansBold",
            12.5,
            52.0,
            712.0,
            navy,
            &encode("REAL ESTATE LISTING AGREEMENT").expect("the title is drawable"),
        );
        content.line((52.0, 705.0), (560.0, 705.0), 1.1, gold);
        content.rect(52.0, 600.0, 508.0, 40.0, Rgb::from_bytes(247, 248, 249));
        content.text(
            "FBody",
            10.35,
            52.0,
            620.0,
            ink,
            &encode("Culebra, Puerto Rico \u{2014} Isla").expect("accented text is drawable"),
        );
        content.text(
            "FBodyBold",
            10.35,
            52.0,
            604.0,
            ink,
            &encode("Broker: Lisa Penfield, #9931").expect("text is drawable"),
        );
        if !images.is_empty() {
            content.image("Im1", 460.0, 712.0, 100.0, 33.0);
        }
        let page = pdf
            .page(
                612.0,
                792.0,
                page_tree,
                resources_ref,
                &content.into_bytes(),
            )
            .expect("the page writes");
        let info = pdf.info(
            "REAL ESTATE LISTING AGREEMENT",
            "Culebraluxe LLC",
            "Listing Agreement",
            "CulebraLuxe Forms",
            "CulebraLuxe Forms",
            "D:20000101000000Z",
        );
        pdf.finish(page_tree, &[page], Some(info))
            .expect("the document serialises")
    }

    #[test]
    fn winansi_text_above_127_survives_into_the_content_stream() {
        // "é" is 0xE9 and "·" is 0xB7 in WinAnsi: neither is valid UTF-8 on its own.
        let mut content = Content::new();
        content.text(
            "F1",
            10.0,
            1.0,
            2.0,
            Rgb::from_bytes(0, 0, 0),
            &[b'a', 0xE9, 0xB7],
        );
        let bytes = content.into_bytes();
        assert!(bytes.windows(3).any(|w| w == [b'a', 0xE9, 0xB7]));
        assert!(
            !bytes.windows(3).any(|w| w == [0xEF, 0xBF, 0xBD]),
            "no U+FFFD replacement"
        );
    }

    #[test]
    fn the_written_document_is_structurally_a_pdf() {
        let bytes = document();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.starts_with("%PDF-1.4"),
            "every PDF starts with its header"
        );
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("/Type /Pages"));
        assert!(text.contains("/Type /Page "));
        assert!(text.contains("/Encoding /WinAnsiEncoding"));
        assert!(
            text.trim_end().ends_with("%%EOF"),
            "and ends with its trailer"
        );

        // Every cross-reference entry must point at the object it claims. A wrong offset opens as a damaged file.
        let start = text.find("xref").expect("an xref table");
        let mut entries = text[start..]
            .lines()
            .skip(2)
            .take_while(|line| line.ends_with(" n ") || line.ends_with(" f "));
        let first = entries.next().expect("at least one entry");
        assert!(first.ends_with(" f "), "object 0 is always free");
        let mut checked = 0;
        for (index, entry) in entries.enumerate() {
            let offset: usize = entry[..10].trim().parse().expect("a ten-digit offset");
            let marker = format!("{} 0 obj", index + 1);
            assert!(
                bytes[offset..].starts_with(marker.as_bytes()),
                "the xref entry for object {} points at {:?}",
                index + 1,
                String::from_utf8_lossy(&bytes[offset..offset + 20])
            );
            checked += 1;
        }
        assert!(
            checked >= 6,
            "expected the four fonts, the page tree and the page"
        );
    }

    /// The reader's verdict, where a reader is available. A structurally plausible file that no reader opens is not a
    /// document, and nothing else in this suite can tell the difference.
    #[test]
    #[cfg(target_os = "macos")]
    fn a_pdf_reader_accepts_the_written_document() {
        let directory = std::env::temp_dir().join(format!("cl-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temp directory");
        let path = directory.join("form.pdf");
        std::fs::write(&path, document()).expect("the document is written");

        let output = std::process::Command::new("/usr/bin/qlmanage")
            .args(["-t", "-s", "400", "-o"])
            .arg(&directory)
            .arg(&path)
            .output()
            .expect("qlmanage runs");
        let rendered = directory.join("form.pdf.png");
        assert!(
            rendered.exists(),
            "QuickLook could not render the document (status {:?})\n{}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let thumbnail = std::fs::metadata(&rendered).expect("the thumbnail exists");
        assert!(thumbnail.len() > 1000, "the thumbnail has content");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
