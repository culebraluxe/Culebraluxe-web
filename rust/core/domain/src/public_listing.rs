//! The public site's listing copy: the one-line pitch a published listing carries.

use serde::{Deserialize, Serialize};

/// One published listing's tagline, keyed by the slug the public pages already carry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicListingCopy {
    pub slug: String,
    pub tagline: String,
}


/// One listing as the public site shows it: the key that opens it, and the facts the grid renders.
///
/// `key` is the slug when the listing has one and the row's id when it does not — the public resolver accepts either
/// (and the name), so a listing is never withheld for want of a URL.
///
/// These are plain values, not presentation: how a price or a badge is *written* belongs to the surface showing it,
/// and the thin TypeScript proxy formats them for the grid exactly as it does today.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicListing {
    pub key: String,
    pub name: String,
    pub property_type: Option<String>,
    pub status: String,
    pub list_price: Option<f64>,
    pub featured: bool,
}
