//! The geometry the document and its screens share: a rectangle in PDF points.
//!
//! MOVED here from the server's composer for the same reason `format_money` was: the applied-signature EVIDENCE records
//! where a signature landed, that evidence is written into the issued snapshot, and the screen reads it back. One
//! definition, so the recorded rectangle and the read rectangle cannot disagree.

use serde::{Deserialize, Serialize};

/// A rectangle in PDF points, measured from the page's bottom-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// The largest box with this image's proportions that fits inside `self`, inset by nothing.
    ///
    /// This is `pdf-lib`'s `scaleToFit`, and it is what keeps a wide signature from being stretched by the slot.
    pub fn scale_to_fit(&self, width: f64, height: f64, image_width: f64, image_height: f64) -> (f64, f64) {
        if image_width <= 0.0 || image_height <= 0.0 {
            return (width, height);
        }
        let scale = (width / image_width).min(height / image_height);
        (image_width * scale, image_height * scale)
    }
}
