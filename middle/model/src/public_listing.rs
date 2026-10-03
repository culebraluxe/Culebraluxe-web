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
    pub water_access: bool,
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

/// One attached asset, as the site needs it — a photograph, a Mux video, or a document. All three are `media` rows,
/// which is why they travel together: the surface decides which are gallery, which are videos, which are documents.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPropertyMedia {
    pub id: String,
    pub role: String,
    pub media_type: String,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<f64>,
    pub sort_order: i32,
    /// Present on Mux videos: the site links out to Mux with this, which is why no video file is stored here.
    pub mux_playback_id: Option<String>,
    pub aspect_ratio: Option<String>,
    pub duration_seconds: Option<f64>,
}

/// One Property as the public site shows it: the key that opens it, its facts, and its photographs.
///
/// Every field is optional except the identity and the status, deliberately — a listing with no price, no year and no
/// architecture notes is still a listing the site must show. The surface decides what to print; nothing here decides
/// whether the Property exists.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicProperty {
    pub id: String,
    pub key: String,
    pub name: String,
    pub status: String,
    pub property_type: Option<String>,
    pub list_price: Option<f64>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub bedrooms: Option<i32>,
    pub bathrooms: Option<f64>,
    pub bathrooms_full: Option<f64>,
    pub bathrooms_half: Option<f64>,
    pub square_feet: Option<i32>,
    pub lot_size: Option<f64>,
    pub lot_size_units: Option<String>,
    pub lot_size_sqft: Option<f64>,
    pub road_frontage_feet: Option<f64>,
    pub road_surface_type: Option<String>,
    pub lot_description: Option<String>,
    pub utilities_notes: Option<String>,
    pub year_built: Option<i32>,
    pub stories: Option<f64>,
    pub parking_spaces: Option<f64>,
    pub view_type: Vec<String>,
    pub water_access: bool,
    pub beach_access: bool,
    pub amenities: Vec<String>,
    pub architecture_notes: Option<String>,
    pub lifestyle_tags: Vec<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_office: Option<String>,
    pub listing_identifier: Option<String>,
    /// The hero: the marked one, or the first photograph. A page always has a picture.
    pub hero_media_id: Option<String>,
    /// Everything attached to the Property — photographs, Mux videos, documents — in gallery order. The surface sorts
    /// them into the gallery, the video strip and the document list; the service does not decide what a surface shows.
    pub media: Vec<PublicPropertyMedia>,
    /// Videos attached to the listing — Mux assets the page links out to.
    pub video_count: i64,
}
