//! DOCS.FORMS.TEMPLATE — font metrics (TST-DOCS-FORMS-TEMPLATE-005).
//!
//! Contract: the form renderer measures text with the four standard fonts' AFM metrics — the same
//! tables `pdf-lib` measures with — so a paragraph wraps where the TypeScript renderer wrapped it
//! and the signature anchors land where they were measured to land. The production boundary is
//! `model::forms_font` (`middle/model/src/forms_font.rs`) over the generated tables in
//! `middle/model/src/forms_font_metrics.rs`: every glyph's advance plus the kerning between it and
//! the next, scaled by `size / 1000`, with a WinAnsi encoding that REFUSES a character these fonts
//! cannot draw rather than substituting one.
//!
//! Level: L0 Pure — the production measurement functions, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__005__font_metrics

use model::forms_font::{encode, text_width, win_ansi_code, StandardFont};

fn close(measured: f64, expected: f64) {
    assert!(
        (measured - expected).abs() < 0.0005,
        "expected {expected}, measured {measured}"
    );
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-005); the file and the assay use it.
fn docs_forms_template_005__font_metrics() {
    // 1. The four standard fonts and the names the PDF dictionary uses (the BaseFont).
    assert_eq!(StandardFont::TimesRoman.base_font(), "Times-Roman");
    assert_eq!(StandardFont::TimesBold.base_font(), "Times-Bold");
    assert_eq!(StandardFont::Helvetica.base_font(), "Helvetica");
    assert_eq!(StandardFont::HelveticaBold.base_font(), "Helvetica-Bold");

    // 2. The advances are the published AFM widths, in 1000ths of an em.
    assert_eq!(StandardFont::TimesRoman.width_of_code(b'A'), 722);
    assert_eq!(StandardFont::TimesRoman.width_of_code(b' '), 250);
    assert_eq!(StandardFont::TimesRoman.width_of_code(b'1'), 500);
    assert_eq!(StandardFont::Helvetica.width_of_code(b'A'), 667);
    assert_eq!(StandardFont::Helvetica.width_of_code(b' '), 278);
    assert_eq!(StandardFont::Helvetica.width_of_code(b'1'), 556);
    assert_eq!(StandardFont::Helvetica.width_of_code(b'i'), 222);
    assert_eq!(StandardFont::TimesBold.width_of_code(b'W'), 1000);

    // 3. A run measures what pdf-lib's own `widthOfTextAtSize` measures — the figures the TypeScript
    //    renderer was measured against, and the contract every wrap and signature anchor follows.
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

    // 4. Kerning is applied between adjacent codes: "AV" is tighter than its two advances alone.
    let advance = f64::from(StandardFont::Helvetica.width_of_code(b'A'))
        + f64::from(StandardFont::Helvetica.width_of_code(b'V'));
    let run = text_width(StandardFont::Helvetica, "AV", 1000.0);
    assert!(
        run < advance,
        "AV should kern tighter than its two advances alone: {run} vs {advance}"
    );

    // 5. WinAnsi encoding: accented Latin-1 and the em dash are drawable.
    assert_eq!(win_ansi_code('\u{e9}'), Some(0xe9), "é is WinAnsi 0xE9");
    assert!(
        win_ansi_code('\u{2014}').is_some(),
        "an em dash is drawable"
    );
    assert!(encode("café").is_ok(), "accented Latin-1 text is drawable");

    // 6. NEGATIVE: a character outside WinAnsi is REFUSED, not substituted. The refusal keeps a
    //    missing glyph visible instead of silently shortening a line and moving everything after
    //    it — and a run containing one has no width at all rather than a guessed one.
    assert_eq!(win_ansi_code('\u{1f389}'), None);
    assert_eq!(encode("party \u{1f389}"), Err('\u{1f389}'));
    assert_eq!(
        text_width(StandardFont::TimesRoman, "\u{1f389}", 10.0),
        0.0,
        "a run with an undrawable character has no width rather than a guessed one"
    );
}
