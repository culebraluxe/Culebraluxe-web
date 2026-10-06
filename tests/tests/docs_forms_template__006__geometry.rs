//! DOCS.FORMS.TEMPLATE — geometry (TST-DOCS-FORMS-TEMPLATE-006).
//!
//! Contract: the rectangle a signature (or any image) is placed in is scaled to fit — the largest
//! box with the image's own proportions that fits inside the slot, never stretched. The
//! production boundary is `Rect::scale_to_fit` (`middle/model/src/forms_geometry.rs`), which the
//! applied-signature evidence and the renderer share so the recorded rectangle and the drawn
//! rectangle cannot disagree. It is `pdf-lib`'s `scaleToFit`: the scale is the smaller of the
//! two dimension ratios, so the image may be enlarged to fill the slot as well as shrunk.
//!
//! Level: L0 Pure — the production geometry function, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_template__006__geometry

use model::forms_geometry::Rect;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-TEMPLATE-006); the file and the assay use it.
fn docs_forms_template_006__geometry() {
    let slot = Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    // 1. A wide image in a square slot: the width is the limiting dimension, and the fitted
    //    height keeps the image's proportions — the signature is not stretched.
    let (width, height) = slot.scale_to_fit(100.0, 100.0, 200.0, 50.0);
    assert!((width - 100.0).abs() < 1e-9, "the width fills the slot: {width}");
    assert!((height - 25.0).abs() < 1e-9, "the height keeps the proportions: {height}");
    assert!(width <= slot.width && height <= slot.height);

    // 2. A tall image in a square slot: the height is the limiting dimension.
    let (width, height) = slot.scale_to_fit(100.0, 100.0, 50.0, 200.0);
    assert!((width - 25.0).abs() < 1e-9, "the width keeps the proportions: {width}");
    assert!((height - 100.0).abs() < 1e-9, "the height fills the slot: {height}");

    // 3. A small image is ENLARGED to fill the slot along its limiting dimension — scaleToFit
    //    scales up as well as down, so a low-resolution signature still fills its box.
    let (width, height) = slot.scale_to_fit(100.0, 100.0, 40.0, 30.0);
    assert!((width - 100.0).abs() < 1e-9, "the width fills the slot: {width}");
    assert!((height - 75.0).abs() < 1e-9, "the height keeps the proportions: {height}");

    // 4. Proportions survive every fit: the fitted box's ratio is the image's ratio.
    let (width, height) = slot.scale_to_fit(100.0, 100.0, 640.0, 480.0);
    assert!(
        (width / height - 640.0 / 480.0).abs() < 1e-9,
        "the fitted box keeps the image's proportions: {width}x{height}"
    );

    // 5. NEGATIVE: a degenerate image (zero width or height) has no proportions to keep. The slot
    //    is returned unchanged rather than dividing by zero and producing an infinite or NaN box
    //    that would poison every coordinate computed from it.
    for (image_width, image_height) in [(0.0, 100.0), (100.0, 0.0), (0.0, 0.0)] {
        let (width, height) = slot.scale_to_fit(100.0, 100.0, image_width, image_height);
        assert!(
            (width - 100.0).abs() < 1e-9 && (height - 100.0).abs() < 1e-9,
            "a degenerate image ({image_width}x{image_height}) must return the slot unchanged"
        );
    }
}
