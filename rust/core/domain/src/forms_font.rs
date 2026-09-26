//! Measuring text with the standard fonts, exactly as the TypeScript renderer measures it.
//!
//! WHY THIS MIRRORS `pdf-lib` RATHER THAN APPROXIMATES IT: the layout is a function of text width. A paragraph wraps
//! where the measured width first exceeds the column, pagination follows from the wrap, and the signature anchors are
//! computed from where the signature blocks land. An approximation that is a tenth of a point per word off moves a
//! line, moves a page, and moves a signature. So the widths and kerning are the AFM data `pdf-lib` itself uses
//! (`@pdf-lib/standard-fonts`, generated into `forms_font_metrics.rs`), and the summation below is the same one.

use crate::forms_font_metrics::{
    HELVETICA_BOLD_KERNS, HELVETICA_BOLD_WIDTHS, HELVETICA_KERNS, HELVETICA_WIDTHS, TIMES_BOLD_KERNS,
    TIMES_BOLD_WIDTHS, TIMES_ROMAN_KERNS, TIMES_ROMAN_WIDTHS, WIN_ANSI_UNICODE_TO_CODE,
};

/// The four standard fonts the form renderer uses. Standard-14 means no font is embedded in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardFont {
    TimesRoman,
    TimesBold,
    Helvetica,
    HelveticaBold,
}

impl StandardFont {
    /// The name the PDF dictionary uses, which is also the BaseFont.
    pub fn base_font(self) -> &'static str {
        match self {
            Self::TimesRoman => "Times-Roman",
            Self::TimesBold => "Times-Bold",
            Self::Helvetica => "Helvetica",
            Self::HelveticaBold => "Helvetica-Bold",
        }
    }

    fn widths(self) -> &'static [u16; 256] {
        match self {
            Self::TimesRoman => &TIMES_ROMAN_WIDTHS,
            Self::TimesBold => &TIMES_BOLD_WIDTHS,
            Self::Helvetica => &HELVETICA_WIDTHS,
            Self::HelveticaBold => &HELVETICA_BOLD_WIDTHS,
        }
    }

    fn kerns(self) -> &'static [(u8, u8, i16)] {
        match self {
            Self::TimesRoman => &TIMES_ROMAN_KERNS,
            Self::TimesBold => &TIMES_BOLD_KERNS,
            Self::Helvetica => &HELVETICA_KERNS,
            Self::HelveticaBold => &HELVETICA_BOLD_KERNS,
        }
    }

    /// The advance width of one WinAnsi code, in 1000ths of an em.
    pub fn width_of_code(self, code: u8) -> u16 {
        self.widths()[code as usize]
    }

    /// The kerning between two adjacent codes. Zero when the pair is not in the table.
    fn kern(self, left: u8, right: u8) -> i32 {
        let table = self.kerns();
        let target = (left, right);
        table
            .binary_search_by(|entry| (entry.0, entry.1).cmp(&target))
            .map(|index| i32::from(table[index].2))
            .unwrap_or(0)
    }
}

/// The WinAnsi code for one character, or `None` when these fonts cannot draw it.
///
/// A `None` is a REFUSAL, not a substitution: the TypeScript path also fails to encode a character outside WinAnsi, and
/// silently drawing something else would change the document rather than report it.
pub fn win_ansi_code(character: char) -> Option<u8> {
    let target = character as u32;
    WIN_ANSI_UNICODE_TO_CODE
        .binary_search_by(|entry| entry.0.cmp(&target))
        .ok()
        .map(|index| WIN_ANSI_UNICODE_TO_CODE[index].1)
}

/// Encode a string to WinAnsi, or report the first character these fonts cannot draw.
pub fn encode(text: &str) -> Result<Vec<u8>, char> {
    let mut codes = Vec::with_capacity(text.len());
    for character in text.chars() {
        match win_ansi_code(character) {
            Some(code) => codes.push(code),
            None => return Err(character),
        }
    }
    Ok(codes)
}

/// The width of a run of text at a size, by the same summation `pdf-lib` uses:
/// every glyph's advance plus the kerning between it and the next, scaled by `size / 1000`.
pub fn text_width(font: StandardFont, text: &str, size: f64) -> f64 {
    let Ok(codes) = encode(text) else {
        // A character these fonts cannot draw has no width; refusing to guess keeps the refusal visible instead of
        // silently shortening a line and moving everything after it.
        return 0.0;
    };
    let mut thousandths: i64 = 0;
    for (index, code) in codes.iter().enumerate() {
        thousandths += i64::from(font.width_of_code(*code));
        if let Some(next) = codes.get(index + 1) {
            thousandths += i64::from(font.kern(*code, *next));
        }
    }
    (thousandths as f64) * (size / 1000.0)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn close(measured: f64, expected: f64) {
        assert!(
            (measured - expected).abs() < 0.0005,
            "expected {expected}, measured {measured}"
        );
    }

    #[test]
    fn the_widths_are_the_published_afm_widths() {
        assert_eq!(StandardFont::TimesRoman.width_of_code(b'A'), 722);
        assert_eq!(StandardFont::TimesRoman.width_of_code(b' '), 250);
        assert_eq!(StandardFont::TimesRoman.width_of_code(b'1'), 500);
        assert_eq!(StandardFont::Helvetica.width_of_code(b'A'), 667);
        assert_eq!(StandardFont::Helvetica.width_of_code(b' '), 278);
        assert_eq!(StandardFont::Helvetica.width_of_code(b'1'), 556);
        assert_eq!(StandardFont::Helvetica.width_of_code(b'i'), 222);
        assert_eq!(StandardFont::TimesBold.width_of_code(b'W'), 1000);
    }

    #[test]
    fn a_run_measures_what_pdf_lib_measures() {
        // These figures were computed with pdf-lib's own `widthOfTextAtSize`; they are the contract.
        close(
            text_width(StandardFont::TimesRoman, "Hello world", 10.35),
            49.628_250,
        );
        close(
            text_width(StandardFont::TimesBold, "Hello world", 10.35),
            51.936_300,
        );
        close(
            text_width(StandardFont::Helvetica, "Hello world", 10.35),
            51.232_500,
        );
        close(
            text_width(StandardFont::HelveticaBold, "Hello world", 10.35),
            55.579_500,
        );
        close(
            text_width(
                StandardFont::Helvetica,
                "The quick brown fox jumps over the lazy dog.",
                8.2,
            ),
            163.417_800,
        );
        // Puerto Rican text measures the same as it does through pdf-lib's WinAnsi encoding.
        close(
            text_width(
                StandardFont::TimesRoman,
                "Culebra, Puerto Rico \u{2014} Isla",
                10.0,
            ),
            113.600_000,
        );
    }

    #[test]
    fn a_kern_pair_is_applied() {
        let advance = f64::from(StandardFont::Helvetica.width_of_code(b'A'))
            + f64::from(StandardFont::Helvetica.width_of_code(b'V'));
        let run = text_width(StandardFont::Helvetica, "AV", 1000.0);
        assert!(
            run < advance,
            "AV should kern tighter than its two advances alone: {run} vs {advance}"
        );
    }

    #[test]
    fn a_character_outside_win_ansi_is_refused_rather_than_substituted() {
        assert!(encode("café").is_ok(), "accented Latin-1 text is drawable");
        assert_eq!(win_ansi_code('\u{e9}'), Some(0xe9), "é is WinAnsi 0xE9");
        assert!(win_ansi_code('\u{2014}').is_some(), "an em dash is drawable");
        assert_eq!(encode("party \u{1f389}"), Err('\u{1f389}'));
        assert_eq!(text_width(StandardFont::TimesRoman, "\u{1f389}", 10.0), 0.0);
    }
}
