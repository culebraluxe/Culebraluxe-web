//! Website and MLS listing fields, and the person and project field sets.

use super::*;

pub(super) const WEBSITE_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "slug",
        label: "Public slug",
        kind: FieldKind::Text,
        wide: true,
        hint: Some("Lowercase letters, numbers and single hyphens."),
    },
    FieldSpec {
        key: "featured",
        label: "Featured",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "isActiveListing",
        label: "Active listing",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "isPublished",
        label: "Published",
        kind: FieldKind::Toggle,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "publicRemarks",
        label: "Public remarks",
        kind: FieldKind::Textarea(6),
        wide: true,
        hint: Some("Canonical remarks reused by downstream publication preparation."),
    },
    FieldSpec {
        key: "seoTitle",
        label: "Search title",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "seoDescription",
        label: "Search description",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "heroTitle",
        label: "Hero title",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "tagline",
        label: "Tagline",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "architectureNotes",
        label: "Architecture notes",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "amenitiesNotes",
        label: "Amenities notes",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "lifestyleNotes",
        label: "Lifestyle notes",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
];

pub(super) const MLS_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "listingContractDate",
        label: "Listing contract date",
        kind: FieldKind::Date,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "expirationDate",
        label: "Expiration date",
        kind: FieldKind::Date,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listingType",
        label: "Listing type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "agentMlsId",
        label: "Agent MLS ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "taxId",
        label: "Tax ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "taxYear",
        label: "Tax year",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "annualTax",
        label: "Annual tax",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "zoning",
        label: "Zoning",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "totalAreaSqft",
        label: "Total area sqft",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "heatedAreaSource",
        label: "Heated area source",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "ownershipType",
        label: "Ownership type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "occupantType",
        label: "Occupant type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "legalDescription",
        label: "Legal description",
        kind: FieldKind::Textarea(4),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "hoaDetails",
        label: "HOA details",
        kind: FieldKind::Textarea(4),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "showingInstructions",
        label: "Showing instructions",
        kind: FieldKind::Textarea(4),
        wide: true,
        hint: None,
    },
];

// NO SEPARATE ARCHIVE CONTROL. This held a lone toggle, "Archived record", with the hint "Archives the record without
// rewriting transaction-owned listing status" — a second way to archive a Property, sitting alongside the Status
// dropdown that also offers Archived. Two controls for one state is how a listing ends up archived in one field and
// published in the other, which is exactly what happened: `status = 'archived'` was set while the column every public
// read filters on (`archived_at`) stayed null.
//
// The Status dropdown is the one control now, and the server derives `archived_at` from it, so the two cannot
// disagree. There is nothing left for this panel to do.

pub(super) const PERSON_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "displayName",
        label: "Display name",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "status",
        label: "Status",
        kind: FieldKind::Select(PERSON_STATUS),
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "company",
        label: "Company",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "email",
        label: "Email",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "phone",
        label: "Phone",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "location",
        label: "Location",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
];

pub(super) const PROJECT_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "name",
        label: "Project name",
        kind: FieldKind::Text,
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "status",
        label: "Status",
        kind: FieldKind::Select(PROJECT_STATUS),
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "owner",
        label: "Owner",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "projectType",
        label: "Project type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "areas",
        label: "Areas",
        kind: FieldKind::Text,
        wide: true,
        hint: Some("Comma-separated project domains."),
    },
    FieldSpec {
        key: "description",
        label: "Description",
        kind: FieldKind::Textarea(6),
        wide: true,
        hint: None,
    },
];

pub(super) const PROJECT_LINKS: &[FieldSpec] = &[
    FieldSpec {
        key: "playbookId",
        label: "Playbook ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "playbookVersion",
        label: "Playbook version",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "personId",
        label: "Person ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "propertyId",
        label: "Property ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "contractId",
        label: "Contract ID",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
];
