//! Public-site shapes: page blocks, listings, guide items, media and the property record.

#[allow(unused_imports)]
use super::*;

/// One editorial block of a public page: exactly the shape `lib/marketing-content` gives the TypeScript pages.
///
/// WHY A PAYLOAD AND NOT ROWS. A list screen's data is a list, and `Row` is the right shape for it. A page is not a
/// list: its hero has an image and an alt text, its sections have eyebrows and calls to action, and rendering those as
/// `cells[0]`/`cells[1]` is how a converted page ends up as a list of strings with no design. This is the shape the
/// page needs, and it is the same shape the TypeScript page read, so the port is a reproduction rather than a guess.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Block {
    pub eyebrow: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub cta_label: Option<String>,
    pub cta_href: Option<String>,
    /// The image this block is built around, and what it shows. Both parts are required for an accessible page, so they
    /// travel together rather than the alt text being optional in practice and omitted in fact.
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
    /// The block's list items, for the blocks that have them (a buyer's services, a set of stats).
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BlockItem {
    /// `list`, `stat`, `office`, `email`, `faq` — what the item is FOR, so the view can render it as what it is.
    pub key: String,
    pub label: Option<String>,
    pub value: Option<String>,
}

/// A card in the featured/property grids: the public shape of a listing.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Listing {
    /// The property's id, which the enquiry link carries so the contact page knows which estate was asked about.
    pub id: String,
    pub slug: String,
    pub name: String,
    pub location: Option<String>,
    pub price: Option<String>,
    pub kind: Option<String>,
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
    /// Bedrooms and bathrooms are NOT integers. Half-baths are ordinary, so `7.5` is a real value in this data — and
    /// declaring these as `i64` made the whole page payload fail to deserialize the first time a listing had one, which
    /// presents as a page with no hero and no sections rather than as a bad number.
    pub beds: Option<f64>,
    pub baths: Option<f64>,
    /// The LOT, formatted ("1 Acre", "12,000 SF").
    pub area: Option<String>,
    /// The interior living area, formatted ("6,399 SF"). Separate from `area` because a residence is read by its
    /// interior first and its lot second; land has only a lot.
    pub interior_area: Option<String>,
    /// The views the property has, in the read model's order ("Ocean", "Beach", ...).
    pub views: Vec<String>,
    pub beach_access: bool,
    /// The listing's one-line pitch ("Two residences. Two pools. ..."). Absent until the public read serves it: the
    /// card draws it when it is there and leaves no gap when it is not.
    pub tagline: Option<String>,
    /// Whether the estate is in the featured set, which is what draws the badge on its card.
    pub featured: bool,
}

/// One entry in the Island Guide: a place, with its photograph.
///
/// THE FIRST PAYLOAD TYPE THAT IS NOT EDITORIAL COPY. The guide is a catalogue read from `guide_item` — beaches, dining,
/// essentials — and each entry is a card with a picture, an area, contact details and a website. Forcing that into a
/// `Block` would have meant cramming a place into `cells`, which is the abstraction the page route exists to avoid.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuideItem {
    /// Which section of the guide it belongs to (`beaches`, `dining`, …), so the page can group it.
    pub section: String,
    pub name: String,
    /// The line above the name: the neighbourhood if there is one, else the area, else the entry's own eyebrow.
    pub subtitle: Option<String>,
    pub area: Option<String>,
    pub eyebrow: Option<String>,
    pub description: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub website_url: Option<String>,
    /// The card photograph. Absent is a real state — the page says "Image coming soon" rather than showing a broken
    /// frame, and it keeps the alt text travelling with the image for the case where there is one.
    pub image_path: Option<String>,
    pub image_alt: Option<String>,
}

/// One piece of a property's media: a gallery frame, a video, or a document.
///
/// PERMISSIVE ON PURPOSE. The read model owns the shape of its media rows — a gallery image and a video do not carry the
/// same fields, and a document carries neither an alt text nor a duration. The route passes them through untouched, so
/// this accepts whatever arrives and the view reads whichever fields are present rather than assuming a shape the read
/// model never promised.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaItem {
    pub id: Option<String>,
    pub url: Option<String>,
    pub href: Option<String>,
    pub media_url: Option<String>,
    pub media_id: Option<String>,
    pub alt: Option<String>,
    pub alt_text: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub label: Option<String>,
    pub caption: Option<String>,
    pub playback_id: Option<String>,
    pub role: Option<String>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

impl MediaItem {
    /// Where the item lives, whichever of the read model's fields carried it. The guide's photographs arrive as a media
    /// id that has to be turned into a route; these arrive as a URL, and both must work.
    pub fn src(&self) -> Option<String> {
        self.url
            .clone()
            .or_else(|| self.href.clone())
            .or_else(|| self.media_url.clone())
            .or_else(|| self.media_id.clone().map(|id| format!("/api/media/{id}")))
    }

    /// What it is called, for the alt text and the label.
    pub fn text(&self) -> Option<&str> {
        self.alt
            .as_deref()
            .or(self.alt_text.as_deref())
            .or(self.title.as_deref())
            .or(self.name.as_deref())
            .or(self.label.as_deref())
            .or(self.caption.as_deref())
    }
}

/// A property record: the facts the card shows, and the media the page is made of.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyRecord {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: Option<String>,
    pub price: Option<String>,
    pub beds: Option<f64>,
    pub baths: Option<f64>,
    pub area: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub year_built: Option<i64>,
    pub architecture: Option<String>,
    pub status: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub lot_size: Option<String>,
    pub lot_size_sqft: Option<f64>,
    pub road_frontage_feet: Option<f64>,
    pub road_surface_type: Option<String>,
    pub lot_description: Option<String>,
    pub utilities_notes: Option<String>,
    pub living_area: Option<f64>,
    pub bathrooms_full: Option<f64>,
    pub bathrooms_half: Option<f64>,
    pub stories: Option<f64>,
    pub parking_spaces: Option<f64>,
    pub water_access: bool,
    pub beach_access: bool,
    pub amenities: Vec<String>,
    pub view_type: Vec<String>,
    pub lifestyle_tags: Vec<String>,
    pub short_description: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_office: Option<String>,
    pub listing_id: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub hero_url: Option<String>,
    pub gallery: Vec<MediaItem>,
    pub videos: Vec<MediaItem>,
    pub documents: Vec<MediaItem>,
    pub similar: Vec<Listing>,
    pub public_slugs: Vec<String>,
}
