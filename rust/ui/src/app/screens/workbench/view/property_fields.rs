//! The property's field definitions: kinds, status vocabularies and every property section.

#[allow(unused_imports)]
use super::*;

#[derive(Clone, Copy)]
pub(super) enum FieldKind {
    Text,
    /// US dollars: shown as $1,250,000, stored as the digits.
    Money,
    Number,
    Date,
    Textarea(u32),
    Toggle,
    Select(&'static [(&'static str, &'static str)]),
}

#[derive(Clone, Copy)]
pub(super) struct FieldSpec {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) kind: FieldKind,
    pub(super) wide: bool,
    pub(super) hint: Option<&'static str>,
}

pub(super) const PROPERTY_STATUS: &[(&str, &str)] = &[
    ("prospect", "Prospect"),
    ("coming_soon", "Coming soon"),
    ("active", "Active"),
    ("off_market", "Off market"),
    // No "workflow owned" notes any more: the service no longer refuses these transitions, so the label would be
    // describing a rule that does not exist.
    ("under_contract", "Under contract"),
    ("sold", "Sold"),
    ("archived", "Archived"),
];

pub(super) const PERSON_STATUS: &[(&str, &str)] = &[
    ("new", "New"),
    ("warm", "Warm"),
    ("active", "Active"),
    ("referral", "Referral"),
];

pub(super) const PROJECT_STATUS: &[(&str, &str)] = &[
    ("open", "Open"),
    ("doing", "In progress"),
    ("done", "Done"),
    ("archived", "Archived"),
];

/// The record's first pane: what it is called, its parcel, where it stands, and what it asks.
pub(super) const PROPERTY_IDENTITY: &[FieldSpec] = &[
    FieldSpec {
        key: "name",
        label: "Property name",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "catastroNumber",
        label: "Catastro number",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "status",
        label: "Status",
        kind: FieldKind::Select(PROPERTY_STATUS),
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listPrice",
        label: "Asking price",
        kind: FieldKind::Money,
        wide: false,
        hint: None,
    },
];

/// How the property is described on its page (the property card and page show this one description).
pub(super) const PROPERTY_DESCRIPTIONS: &[FieldSpec] = &[
    FieldSpec {
        key: "editorialDescription",
        label: "Description",
        kind: FieldKind::Textarea(6),
        wide: true,
        hint: Some("The property page's main description."),
    },
];

pub(super) const PROPERTY_CORE: &[FieldSpec] = &[
    FieldSpec {
        key: "propertyType",
        label: "Property type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "originalListPrice",
        label: "Original list price",
        kind: FieldKind::Money,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "bedrooms",
        label: "Bedrooms",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "bathrooms",
        label: "Bathrooms",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "bathroomsFull",
        label: "Full baths",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "bathroomsHalf",
        label: "Half baths",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "squareFeet",
        label: "Interior sqft",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "yearBuilt",
        label: "Year built",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "stories",
        label: "Stories",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "parkingSpaces",
        label: "Parking spaces",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_SITE_AREA: &[FieldSpec] = &[
    FieldSpec {
        key: "lotSizeAcres",
        label: "Lot acres",
        kind: FieldKind::Number,
        wide: false,
        hint: Some("Reported acreage; enter independently of square feet."),
    },
    FieldSpec {
        key: "lotSizeSqft",
        label: "Lot square feet",
        kind: FieldKind::Number,
        wide: false,
        hint: Some(
            "Reported lot area in square feet; enter independently of acres and interior area.",
        ),
    },
];

pub(super) const PROPERTY_FEATURES: &[FieldSpec] = &[
    FieldSpec {
        key: "hasOceanView",
        label: "Ocean view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasBayView",
        label: "Bay view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasBeachView",
        label: "Beach view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasHarborView",
        label: "Harbor view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasIslandView",
        label: "Island view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasMountainView",
        label: "Mountain view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasSunriseView",
        label: "Sunrise view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasSunsetView",
        label: "Sunset view",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasWaterAccess",
        label: "Water access",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasBeachAccess",
        label: "Beach access",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasPool",
        label: "Pool",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasGenerator",
        label: "Generator",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "hasSolar",
        label: "Solar",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "isFurnished",
        label: "Furnished",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "isGated",
        label: "Gated",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_SITE: &[FieldSpec] = &[
    FieldSpec {
        key: "buildability",
        label: "Buildability",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "slopeDescription",
        label: "Slope / terrain",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "poolPotential",
        label: "Room for pool",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "roadAdjacency",
        label: "Road adjacency",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "viewDescription",
        label: "View description",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "lotDescription",
        label: "Other lot details",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "roadFrontageFeet",
        label: "Road frontage (feet)",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "roadSurfaceType",
        label: "Road surface",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "utilitiesAvailability",
        label: "Utilities availability / location",
        kind: FieldKind::Text,
        wide: false,
        hint: Some("For example: available at the property edge."),
    },
    FieldSpec {
        key: "hoaStatus",
        label: "HOA status",
        kind: FieldKind::Select(&[("", "Unknown"), ("No", "No"), ("Yes", "Yes")]),
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "utilitiesNotes",
        label: "Utilities details",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
];

pub(super) const PROPERTY_GPS: &[FieldSpec] = &[
    FieldSpec {
        key: "latitude",
        label: "Latitude",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "longitude",
        label: "Longitude",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_ADDRESS: &[FieldSpec] = &[
    FieldSpec {
        key: "location",
        label: "Location label",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "addressLine1",
        label: "Address line",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "streetNumber",
        label: "Street number",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "streetName",
        label: "Street name",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "unitNumber",
        label: "Unit",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "neighborhood",
        label: "Neighborhood",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "city",
        label: "Municipality / city",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "stateOrProvince",
        label: "State / province",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "postalCode",
        label: "Postal code",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "country",
        label: "Country",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "isoCountryCode",
        label: "ISO country",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_LEGAL: &[FieldSpec] = &[
    FieldSpec {
        key: "legalOwnerName",
        label: "Legal owner",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "listingIdentifier",
        label: "MLS / listing ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_PARCEL: &[FieldSpec] = &[
    FieldSpec {
        key: "registryEntry",
        label: "Registry entry",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "fincaNumber",
        label: "Finca number",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "registrySection",
        label: "Registry section",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
];

pub(super) const PROPERTY_AGENT: &[FieldSpec] = &[
    FieldSpec {
        key: "listingAgentName",
        label: "Listing agent",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listingAgentEmail",
        label: "Agent email",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listingAgentPhone",
        label: "Agent phone",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listingOffice",
        label: "Listing office",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
];
