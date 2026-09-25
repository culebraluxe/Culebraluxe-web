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
    pub id: String,
    pub name: String,
    pub property_type: Option<String>,
    pub status: String,
    pub list_price: Option<f64>,
    pub featured: bool,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub bedrooms: Option<i32>,
    pub bathrooms: Option<f64>,
    pub square_feet: Option<i32>,
    pub lot_size: Option<f64>,
    pub lot_size_units: Option<String>,
    /// The view labels a card shows, spelled out — "Ocean", "Sunset" — because the eight booleans they come from are
    /// a storage detail and every surface wants the same words.
    pub views: Vec<String>,
    pub beach_access: bool,
    /// The hero: the marked photograph, or the first one. A card always has a picture.
    pub hero_media_id: Option<String>,
    pub hero_alt: Option<String>,
}


/// One photograph on a public listing: the id the site serves it by, and the role that explains why it is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPropertyImage {
    pub id: String,
    pub role: String,
}

/// One Property as the public site shows it: the key that opens it, its facts, and its photographs.
///
/// Every field is optional except the identity and the status, deliberately — a listing with no price, no year and no
/// architecture notes is still a listing the site must show. The surface decides what to print; nothing here decides
/// whether the Property exists.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicProperty {
    pub key: String,
    pub name: String,
    pub status: String,
    pub property_type: Option<String>,
    pub list_price: Option<f64>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub bedrooms: Option<i32>,
    pub bathrooms: Option<f64>,
    pub square_feet: Option<i32>,
    pub lot_size: Option<f64>,
    pub lot_size_units: Option<String>,
    pub year_built: Option<i32>,
    pub architecture_notes: Option<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    /// The hero: the marked one, or the first photograph. A page always has a picture.
    pub hero_media_id: Option<String>,
    pub gallery_media_ids: Vec<String>,
    /// Videos attached to the listing — Mux assets the page links out to. A count, not a URL: the playable link is
    /// Mux's, and the page asks Mux directly.
    pub video_count: i64,
}
