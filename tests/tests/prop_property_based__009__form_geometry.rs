//! PROP.PROPERTY_BASED — form geometry (TST-PROP-PROPERTY-BASED-009).
//!
//! Contract: a signature image lands in its slot through `scale_to_fit`
//! (`pdf-lib`'s rule) — the largest box with the image's proportions that
//! fits inside, so a wide signature is never stretched by the slot. A
//! degenerate image (zero or negative dimensions) leaves the slot
//! untouched; the fitted box always fits within the slot and always keeps
//! the image's aspect ratio; the rectangle itself round-trips through its
//! camelCase wire form, so recorded evidence and the reading screen agree.
//!
//! Level: L0 Pure — the executable boundary is `model::forms_geometry`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__009__form_geometry

use model::forms_geometry::Rect;
use proptest::prelude::*;

/// Positive finite dimensions — no NaN, no infinities, no negatives.
fn dimension() -> impl Strategy<Value = f64> {
    (1u32..4000).prop_map(|n| f64::from(n) / 4.0)
}

fn rect() -> impl Strategy<Value = Rect> {
    (dimension(), dimension(), dimension(), dimension()).prop_map(|(x, y, width, height)| Rect {
        x,
        y,
        width,
        height,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The fitted box fits, keeps proportions, never stretches; degenerate
    /// images are identity; the wire form round-trips.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_009__form_geometry(
        slot in rect(),
        image_width in dimension(),
        image_height in dimension(),
        degenerate in prop_oneof![Just(0.0f64), Just(-3.5f64)],
    ) {
        // Fixed positive: the documented wide-signature case — min(0.5, 0.34).
        let wide = Rect { x: 52.0, y: 100.0, width: 200.0, height: 34.0 };
        let (w, h) = wide.scale_to_fit(200.0, 34.0, 400.0, 100.0);
        prop_assert!((w - 136.0).abs() < 1e-9, "wide image width: {}", w);
        prop_assert!((h - 34.0).abs() < 1e-9, "wide image height: {}", h);
        // Fixed positive: an exact-fit image keeps its size.
        let (w, h) = wide.scale_to_fit(200.0, 34.0, 200.0, 34.0);
        prop_assert!((w - 200.0).abs() < 1e-9 && (h - 34.0).abs() < 1e-9);
        // Fixed negatives: a degenerate image leaves the slot untouched.
        prop_assert_eq!(wide.scale_to_fit(200.0, 34.0, 0.0, 100.0), (200.0, 34.0));
        prop_assert_eq!(wide.scale_to_fit(200.0, 34.0, 400.0, 0.0), (200.0, 34.0));
        prop_assert_eq!(wide.scale_to_fit(200.0, 34.0, -400.0, 100.0), (200.0, 34.0));

        // Property: the fitted box fits inside the slot on both axes.
        let (fit_w, fit_h) = slot.scale_to_fit(slot.width, slot.height, image_width, image_height);
        prop_assert!(fit_w <= slot.width + 1e-9, "fitted width overflows: {}", fit_w);
        prop_assert!(fit_h <= slot.height + 1e-9, "fitted height overflows: {}", fit_h);
        prop_assert!(fit_w >= 0.0 && fit_h >= 0.0, "fitted box must be non-negative");
        // Property: the fit keeps the image's aspect ratio (no stretching).
        let ratio = image_width / image_height;
        prop_assert!(
            ((fit_w / fit_h) - ratio).abs() < 1e-9,
            "aspect ratio drifted: {}x{} vs image {}x{}",
            fit_w,
            fit_h,
            image_width,
            image_height
        );
        // Property: the fit touches at least one slot edge — it is the
        // LARGEST such box, not merely a fitting one.
        let touches_w = (fit_w - slot.width).abs() < 1e-9;
        let touches_h = (fit_h - slot.height).abs() < 1e-9;
        prop_assert!(touches_w || touches_h, "fit must fill one axis: {}x{}", fit_w, fit_h);
        // Property: degenerate image dimensions are identity on any slot.
        prop_assert_eq!(
            slot.scale_to_fit(slot.width, slot.height, degenerate, image_height),
            (slot.width, slot.height)
        );
        prop_assert_eq!(
            slot.scale_to_fit(slot.width, slot.height, image_width, degenerate),
            (slot.width, slot.height)
        );
        // Property: the rectangle round-trips through its camelCase wire form.
        let wire = serde_json::to_value(&slot).expect("serializes");
        let back: Rect = serde_json::from_value(wire).expect("deserializes");
        prop_assert_eq!(back, slot);
    }
}
