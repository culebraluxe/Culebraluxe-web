//! OPPS Data Workbench: one selector/editor shell over typed Rust domain adapters.
//! The renderer is generic; each entity supplies field and section definitions.

use yew::prelude::*;

use crate::model::{
    Msg, PortalOpsMediaAsset, PortalOpsPerson, PortalOpsProject, PortalOpsProperty,
    PortalOpsWorkbenchPage,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct OpsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct OpsWorkbench;

impl Component for OpsWorkbench {
    type Message = ();
    type Properties = OpsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("property-admin").expect("OPPS workbench screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { workbench(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

#[derive(Clone, Copy)]
enum FieldKind {
    Text,
    Number,
    Date,
    Textarea(u32),
    Toggle,
    Select(&'static [(&'static str, &'static str)]),
}

#[derive(Clone, Copy)]
struct FieldSpec {
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    wide: bool,
    hint: Option<&'static str>,
}

const PROPERTY_STATUS: &[(&str, &str)] = &[
    ("prospect", "Prospect"),
    ("coming_soon", "Coming soon"),
    ("active", "Active"),
    ("off_market", "Off market"),
    ("under_contract", "Under contract - workflow owned"),
    ("sold", "Sold - workflow owned"),
    ("archived", "Archived"),
];

const PERSON_STATUS: &[(&str, &str)] = &[
    ("new", "New"),
    ("warm", "Warm"),
    ("active", "Active"),
    ("referral", "Referral"),
];

const PROJECT_STATUS: &[(&str, &str)] = &[
    ("open", "Open"),
    ("doing", "In progress"),
    ("done", "Done"),
    ("archived", "Archived"),
];

const PROPERTY_CORE: &[FieldSpec] = &[
    FieldSpec {
        key: "name",
        label: "Property name",
        kind: FieldKind::Text,
        wide: true,
        hint: Some("Canonical property name."),
    },
    FieldSpec {
        key: "status",
        label: "Status",
        kind: FieldKind::Select(PROPERTY_STATUS),
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "propertyType",
        label: "Property type",
        kind: FieldKind::Text,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "listPrice",
        label: "List price",
        kind: FieldKind::Number,
        wide: false,
        hint: None,
    },
    FieldSpec {
        key: "originalListPrice",
        label: "Original list price",
        kind: FieldKind::Number,
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

const PROPERTY_SITE_AREA: &[FieldSpec] = &[
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

const PROPERTY_FEATURES: &[FieldSpec] = &[
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

const PROPERTY_SITE: &[FieldSpec] = &[
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

const PROPERTY_ADDRESS: &[FieldSpec] = &[
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

const PROPERTY_LEGAL: &[FieldSpec] = &[
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

const PROPERTY_PARCEL: &[FieldSpec] = &[
    FieldSpec {
        key: "catastroNumber",
        label: "Catastro number",
        kind: FieldKind::Text,
        wide: false,
        hint: Some("Puerto Rico parcel identifier, distinct from a listing ID."),
    },
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

const PROPERTY_AGENT: &[FieldSpec] = &[
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

const WEBSITE_FIELDS: &[FieldSpec] = &[
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
        key: "shortDescription",
        label: "Short description",
        kind: FieldKind::Textarea(3),
        wide: true,
        hint: None,
    },
    FieldSpec {
        key: "editorialDescription",
        label: "Editorial description",
        kind: FieldKind::Textarea(6),
        wide: true,
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

const MLS_FIELDS: &[FieldSpec] = &[
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

const PROPERTY_ADMIN_FIELDS: &[FieldSpec] = &[FieldSpec {
    key: "archived",
    label: "Archived record",
    kind: FieldKind::Toggle,
    wide: false,
    hint: Some("Archives the record without rewriting transaction-owned listing status."),
}];

const PERSON_FIELDS: &[FieldSpec] = &[
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
];

const PROJECT_FIELDS: &[FieldSpec] = &[
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

const PROJECT_LINKS: &[FieldSpec] = &[
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

fn payload(model: &crate::model::Model) -> Option<&PortalOpsWorkbenchPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.ops.as_ref())
}

fn value(model: &crate::model::Model, key: &str) -> String {
    model.ops.form.get(key).cloned().unwrap_or_default()
}

fn workbench(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let data = payload(model);
    let total = data.map(|data| data.total).unwrap_or(0);
    let current = data.map(|data| data.page).unwrap_or(1);
    let page_size = data.map(|data| data.page_size.max(1)).unwrap_or(50);
    let pages = ((total + page_size - 1) / page_size).max(1);
    let write_action = match model.ops.entity.as_str() {
        "person" => "person.write",
        "project" => "project.write",
        _ => "property.write",
    };
    let rail_class = if model.ops.rail_collapsed {
        "grid min-h-0 gap-3 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[56px_minmax(0,1fr)]"
    } else {
        "grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[240px_minmax(0,1fr)]"
    };

    html! {
        <div class="space-y-3">
            { entity_switcher(model, on_msg) }
            <div class={rail_class}>
                { selector_rail(model, on_msg, total, current, pages) }
                <main class="min-h-0 overflow-hidden">
                    <fieldset disabled={!model.can(write_action)}>
                        { editor(model, on_msg) }
                    </fieldset>
                </main>
            </div>
        </div>
    }
}

fn entity_switcher(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    html! {
        <div class="portal-glass-panel flex flex-wrap items-center justify-between gap-3 rounded-[var(--portal-panel-radius)] px-3 py-2">
            <div class="flex min-w-0 items-center gap-3">
                <div class="hidden sm:block">
                    <div class="text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"OPPS"}</div>
                    <div class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Data Workbench"}</div>
                </div>
                <div class="flex rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-1">
                    { entity_button(model, on_msg, "property", "⌂", "Property") }
                    { entity_button(model, on_msg, "person", "♙", "Person") }
                    { entity_button(model, on_msg, "project", "◇", "Project") }
                </div>
            </div>
            <div class="text-right">
                <div class="text-[10px] uppercase tracking-[0.14em] text-black/35">{"Canonical data"}</div>
                <div class="text-[12px] font-light text-black/55">{"Edit once · project downstream"}</div>
            </div>
        </div>
    }
}

fn entity_button(
    model: &crate::model::Model,
    on_msg: &Callback<Msg>,
    key: &'static str,
    icon: &'static str,
    label: &'static str,
) -> Html {
    let active = model.ops.entity == key;
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsEntitySelected(key.into())))
    };
    html! {
        <button
            type="button"
            {onclick}
            aria-pressed={active.to_string()}
            class={classes!(
                "inline-flex","h-9","items-center","gap-1.5","rounded-[calc(var(--portal-tab-radius)-2px)]","px-3","text-[12px]","font-medium","transition",
                if active { "bg-[var(--portal-navy)] text-white shadow-sm" } else { "text-[var(--portal-navy)] hover:bg-white/60" }
            )}
        >
            <span class="text-[15px] leading-none" aria-hidden="true">{icon}</span>
            <span>{label}</span>
        </button>
    }
}

fn selector_rail(
    model: &crate::model::Model,
    on_msg: &Callback<Msg>,
    total: i64,
    current: i64,
    pages: i64,
) -> Html {
    let data = payload(model);
    let rows = data.map(|data| data.rows.as_slice()).unwrap_or(&[]);
    let collapsed = model.ops.rail_collapsed;

    let search = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };
    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsRailToggled))
    };
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(-1)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(1)))
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsCreateToggled))
    };

    html! {
        <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2">
                <div class="flex items-center justify-between gap-2">
                    if !collapsed {
                        <div class="truncate text-[10px] font-semibold uppercase tracking-[0.14em] text-black/40">
                            { format!("{} · {total}", entity_plural(&model.ops.entity)) }
                        </div>
                    }
                    <button
                        type="button"
                        onclick={toggle}
                        title={if collapsed { "Expand selector" } else { "Collapse selector" }}
                        class="ml-auto grid h-8 w-8 shrink-0 place-items-center rounded-full border border-[var(--portal-panel-border)] bg-white/45 text-[var(--portal-navy)] transition hover:bg-white/75"
                    >
                        { if collapsed { "›" } else { "‹" } }
                    </button>
                </div>
                if !collapsed {
                    <input
                        type="search"
                        oninput={search}
                        value={model.controls.query.clone()}
                        disabled={model.ops.dirty}
                        placeholder={format!("Search {}…", entity_plural(&model.ops.entity).to_lowercase())}
                        class="mt-2 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 px-2.5 py-2 text-[13px] font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)] disabled:opacity-45"
                    />
                    if model.ops.entity == "property" {
                        <button
                            type="button"
                            onclick={create}
                            disabled={model.ops.dirty || !model.can("property.write")}
                            class="mt-2 flex h-8 w-full items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)] hover:bg-white/60 disabled:opacity-35"
                        >
                            { if model.ops.creating { "Cancel new property" } else { "+ New property" } }
                        </button>
                        if model.ops.creating {
                            { create_property(model, on_msg) }
                        }
                    }
                }
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto">
                if rows.is_empty() {
                    <p class={if collapsed { "px-2 py-6 text-center text-xs text-black/35" } else { "px-3 py-6 text-sm font-light text-black/40" }}>
                        { if model.loading { "…" } else if collapsed { "—" } else { "No matching records." } }
                    </p>
                } else {
                    { for rows.iter().map(|row| {
                        let selected = data.and_then(|data| data.selected_id.as_deref()) == Some(row.id.as_str());
                        let id = row.id.clone();
                        let onclick = {
                            let on_msg = on_msg.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RowSelected(id.clone())))
                        };
                        html! {
                            <button
                                type="button"
                                {onclick}
                                title={if collapsed { row.title.clone() } else { String::new() }}
                                class={classes!(
                                    "w-full","border-b","border-[var(--portal-panel-border)]","text-left","transition",
                                    if collapsed { "grid h-11 place-items-center px-1" } else { "flex items-start gap-2 px-2.5 py-2.5" },
                                    if selected { "border-l-2 border-l-[var(--portal-gold)] bg-white/45" } else { "border-l-2 border-l-transparent hover:bg-white/25" }
                                )}
                            >
                                if collapsed {
                                    <span class={format!("h-2 w-2 rounded-full {}", status_dot(&row.status))}></span>
                                } else {
                                    <span class={format!("mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full {}", status_dot(&row.status))}></span>
                                    <span class="min-w-0 flex-1">
                                        <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">{ row.title.clone() }</span>
                                        <span class="mt-0.5 block truncate text-[11px] font-light text-black/45">
                                            { compact_meta(row.subtitle.as_deref(), row.meta.as_deref(), &row.status) }
                                        </span>
                                    </span>
                                }
                            </button>
                        }
                    }) }
                }
            </div>

            if !collapsed {
                <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={previous} disabled={current <= 1 || model.ops.dirty}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"← Prev"}
                    </button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{current} / {pages}") }</span>
                    <button type="button" onclick={next} disabled={current >= pages || model.ops.dirty}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"Next →"}
                    </button>
                </div>
            }
        </aside>
    }
}

fn create_property(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::OpsCreateNameChanged(value));
        })
    };
    let change_type = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::OpsCreateTypeChanged(value));
        })
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsCreateRequested))
    };
    html! {
        <div class="mt-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-2">
            <input
                value={model.ops.new_name.clone()}
                oninput={change}
                placeholder="Property name"
                class="h-8 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[12px] outline-none"
            />
            <input type="text" oninput={change_type} value={model.ops.new_property_type.clone()}
                placeholder="Property type (for example, Land)"
                class="mt-2 h-9 w-full rounded border border-[var(--portal-panel-border)] bg-white/65 px-2 text-[12px]" />
            <button
                type="button"
                onclick={create}
                disabled={model.loading || !model.can("property.write")}
                class="mt-2 h-8 w-full rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
            >
                { if model.loading { "Creating…" } else { "Create & open" } }
            </button>
        </div>
    }
}

fn editor(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return empty_editor(model.loading);
    };
    if data.selected_id.is_none() {
        return empty_editor(model.loading);
    }

    html! {
        <div class="flex h-full min-h-0 flex-col gap-3">
            { editor_header(model, data, on_msg) }
            { section_tabs(model, on_msg) }
            <section class="portal-glass-panel min-h-0 flex-1 overflow-y-auto rounded-[var(--portal-panel-radius)]">
                <div class="p-4 md:p-5">
                    {
                        match data.entity.as_str() {
                            "person" => person_editor(model, data.person.as_ref(), on_msg),
                            "project" => project_editor(model, data.project.as_ref(), on_msg),
                            _ => property_editor(model, data.property.as_ref(), &data.media, on_msg),
                        }
                    }
                </div>
            </section>
        </div>
    }
}

fn editor_header(
    model: &crate::model::Model,
    data: &PortalOpsWorkbenchPage,
    on_msg: &Callback<Msg>,
) -> Html {
    let (title, subtitle, status) = match data.entity.as_str() {
        "person" => data.person.as_ref().map(|record| {
            (
                record.display_name.clone(),
                record
                    .company
                    .clone()
                    .or_else(|| record.email.clone())
                    .unwrap_or_else(|| "Person".into()),
                record.status.clone(),
            )
        }),
        "project" => data.project.as_ref().map(|record| {
            (
                record.name.clone(),
                record
                    .project_type
                    .clone()
                    .unwrap_or_else(|| "Project".into()),
                record.status.clone(),
            )
        }),
        _ => data.property.as_ref().map(|record| {
            (
                record.name.clone(),
                record.location.clone().unwrap_or_else(|| "Property".into()),
                if record.archived {
                    "archived".into()
                } else {
                    record.status.clone()
                },
            )
        }),
    }
    .unwrap_or_else(|| ("Record".into(), String::new(), String::new()));

    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsSaveRequested))
    };
    let revert = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsRevertRequested))
    };

    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-4 py-3">
            <div class="flex flex-wrap items-center gap-3">
                <div class="min-w-0 flex-1">
                    <div class="flex flex-wrap items-center gap-2">
                        <h1 class="truncate font-serif text-2xl font-light text-[var(--portal-navy)]">{title}</h1>
                        <span class="rounded-full border border-[var(--portal-panel-border)] bg-white/45 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-navy)]">
                            {status.replace('_', " ")}
                        </span>
                        if model.ops.dirty {
                            <span class="rounded-full bg-[var(--portal-gold)]/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-gold-muted)]">
                                {"Unsaved"}
                            </span>
                        }
                    </div>
                    <p class="mt-1 truncate text-[12px] font-light text-black/50">{subtitle}</p>
                </div>
                { header_metrics(data) }
                <div class="flex shrink-0 gap-2">
                    <button
                        type="button"
                        onclick={revert}
                        disabled={!model.ops.dirty || model.ops.saving}
                        class="h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-30"
                    >
                        {"Revert"}
                    </button>
                    <button
                        type="button"
                        onclick={save}
                        disabled={!model.ops.dirty || model.ops.saving}
                        class="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white shadow-sm disabled:opacity-35"
                    >
                        {if model.ops.saving { "Saving…" } else { "Save" }}
                    </button>
                </div>
            </div>
        </section>
    }
}

fn header_metrics(data: &PortalOpsWorkbenchPage) -> Html {
    if data.entity != "property" {
        return html! {};
    }
    let Some(property) = data.property.as_ref() else {
        return html! {};
    };
    let stellar_fields = [
        property.stellar.listing_contract_date.as_deref(),
        property.stellar.expiration_date.as_deref(),
        property.stellar.listing_type.as_deref(),
        property.stellar.agent_mls_id.as_deref(),
        property.stellar.tax_id.as_deref(),
        property.stellar.tax_year.as_deref(),
        property.stellar.annual_tax.as_deref(),
        property.stellar.legal_description.as_deref(),
        property.stellar.zoning.as_deref(),
        property.stellar.total_area_sqft.as_deref(),
        property.stellar.heated_area_source.as_deref(),
        property.stellar.ownership_type.as_deref(),
        property.stellar.hoa_details.as_deref(),
        property.stellar.showing_instructions.as_deref(),
        property.stellar.occupant_type.as_deref(),
    ];
    let filled = stellar_fields
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .count();

    html! {
        // ONE PHOTOGRAPH COUNT, AND IT LIVES HERE.
        //
        // The Photos tab used to print "6 photos · 0 hero · 0 documents" one line above the uploader, while this
        // header already carried "MEDIA — 6 images": the same number twice, and the copy in the reading path was the
        // wrong one. These are the counts, once each, where the other Property facts already sit.
        <div class="hidden shrink-0 gap-5 xl:flex">
            { metric("Media", &format!("{} images", property.image_count)) }
            // HERO IS NOT HERE YET, DELIBERATELY. `PortalOpsProperty` carries `image_count` and `document_count`
            // but no hero count, and no field on it can be filtered into one — so printing "0 hero" from a guess
            // would be a number that looks authoritative and is wrong. It needs one field on the read model
            // (`hero_count`, or the media list with roles), which is a service-layer change rather than a header
            // change. Until then the header says what it knows.
            { metric("Documents", &property.document_count.to_string()) }
            { metric("MLS details", &format!("{filled}/15 filled")) }
        </div>
    }
}

fn metric(label: &str, value: &str) -> Html {
    html! {
        <div class="text-right">
            <div class="text-[9px] uppercase tracking-[0.14em] text-black/35">{label}</div>
            <div class="text-[12px] font-medium text-[var(--portal-navy)]">{value}</div>
        </div>
    }
}

fn section_tabs(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let tabs: &[(&str, &str)] = match model.ops.entity.as_str() {
        "person" => &[
            ("identity", "Identity"),
            ("contact", "Contact"),
            ("relations", "Relations"),
        ],
        "project" => &[("project", "Project"), ("links", "Links")],
        _ => &[
            ("property", "Property"),
            ("site", "Site"),
            ("legal", "Legal"),
            ("website", "Website"),
            ("mls", "MLS"),
            ("photos", "Photos"),
            ("video", "Video"),
            ("person", "Person"),
            ("sources", "Sources"),
        ],
    };

    html! {
        <div class="portal-glass-panel flex gap-1 overflow-x-auto rounded-[var(--portal-panel-radius)] p-1.5">
            {for tabs.iter().map(|(key, label)| {
                let active = model.ops.section == *key;
                let selected_key = (*key).to_string();
                let onclick = {
                    let on_msg = on_msg.clone();
                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsSectionSelected(selected_key.clone())))
                };
                html! {
                    <button
                        type="button"
                        {onclick}
                        class={classes!(
                            "h-8","shrink-0","rounded-[var(--portal-tab-radius)]","px-3","text-[11px]","font-medium","transition",
                            if active { "bg-[var(--portal-navy)] text-white" } else { "text-[var(--portal-navy)] hover:bg-white/50" }
                        )}
                    >
                        {*label}
                    </button>
                }
            })}
        </div>
    }
}

fn property_editor(
    model: &crate::model::Model,
    property: Option<&PortalOpsProperty>,
    media: &[PortalOpsMediaAsset],
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(property) = property else {
        return empty_record("Property");
    };

    match model.ops.section.as_str() {
        "site" => html! {
            <div class="space-y-4">
                {section_intro("Site and land", "Lot, location and parcel facts apply to every property, including houses. Enter what is known now and return later.")}
                {field_panel(model, on_msg, "Land area", PROPERTY_SITE_AREA)}
                {field_panel(model, on_msg, "Terrain, roads and utilities", PROPERTY_SITE)}
                {feature_panel(model, on_msg)}
                {field_panel(model, on_msg, "Location and address", PROPERTY_ADDRESS)}
                {field_panel(model, on_msg, "Parcel identifiers", PROPERTY_PARCEL)}
            </div>
        },
        "legal" => html! {
            <div class="space-y-4">
                {section_intro("Legal and listing details", "Add identifiers and representation details as they become available.")}
                {field_panel(model, on_msg, "Legal owner and listing ID", PROPERTY_LEGAL)}
                {field_panel(model, on_msg, "Listing representation", PROPERTY_AGENT)}
                {field_panel(model, on_msg, "Administration", PROPERTY_ADMIN_FIELDS)}
            </div>
        },
        "sources" => html! {
            <div class="space-y-4">
                {section_intro("Source data", "System and imported values are visible here for reconciliation.")}
                {source_columns_panel("System and provenance columns", &property.source_metadata)}
                {source_columns_panel("Regrid enrichment columns", &property.regrid_fields)}
            </div>
        },
        "website" => html! {
            <div class="space-y-4">
                {section_intro("Website", "Presentation and publication controls over the canonical Property record.")}
                {field_panel(model, on_msg, "Publication", WEBSITE_FIELDS)}
            </div>
        },
        "mls" => html! {
            <div class="space-y-4">
                {section_intro("Stellar MLS preparation", "Canonical Property values stay inherited; this section collects only MLS-specific extension fields.")}
                <div class="grid gap-3 md:grid-cols-2">
                    {readonly_card("property_id", property.stellar_property_id.as_deref().unwrap_or("—"))}
                    {readonly_card("updated_at", property.stellar_updated_at.as_deref().unwrap_or("—"))}
                </div>
                <div class="grid gap-3 md:grid-cols-4">
                    {inherited("Property", value(model, "name"))}
                    {inherited("List price", value(model, "listPrice"))}
                    {inherited("Address", value(model, "addressLine1"))}
                    {inherited("Beds / baths", format!("{} / {}", value(model, "bedrooms"), value(model, "bathrooms")))}
                </div>
                {field_panel(model, on_msg, "MLS-specific fields", MLS_FIELDS)}
            </div>
        },
        "photos" => media_editor(model, property, media, on_msg),
        "video" => video_editor(property, media, on_msg),
        "person" => property_person_editor(model, property, on_msg),
        _ => html! {
            <div class="space-y-4">
                {section_intro("Property facts", "Start with what is known. Save and return to complete other fields in later passes.")}
                {field_panel(model, on_msg, "Listing and building facts", PROPERTY_CORE)}
            </div>
        },
    }
}

fn feature_panel(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    html! {
        <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-4">
            <h2 class="mb-3 text-[12px] font-semibold text-[var(--portal-navy)]">{"Views, access and improvements"}</h2>
            <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
                {for PROPERTY_FEATURES.iter().map(|field| {
                    let key = field.key.to_string();
                    let on_msg = on_msg.clone();
                    html! {
                        <label class="flex min-h-10 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-3 py-2 text-[13px] text-[var(--portal-navy)]">
                            <input
                                type="checkbox"
                                checked={value(model, field.key) == "true"}
                                disabled={model.ops.saving}
                                onchange={Callback::from(move |event: Event| {
                                    let checked = event.target_unchecked_into::<web_sys::HtmlInputElement>().checked();
                                    on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value: checked.to_string() });
                                })}
                                class="h-4 w-4 shrink-0 rounded border-[var(--portal-panel-border)]"
                            />
                            <span>{field.label}</span>
                        </label>
                    }
                })}
            </div>
        </section>
    }
}

fn source_columns_panel(
    title: &str,
    columns: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Html {
    html! {
        <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-4">
            <h2 class="mb-3 text-sm font-semibold text-[var(--portal-navy)]">{title}</h2>
            <p class="mb-4 text-[12px] text-black/55">{"Read-only values maintained by the system or imported data source."}</p>
            <dl class="grid gap-3 lg:grid-cols-2">
                {for columns.iter().map(|(key, value)| {
                    let display = if value.is_null() { "—".to_string() }
                        else if let Some(text) = value.as_str() { text.to_string() }
                        else { value.to_string() };
                    html! {
                        <div class="min-w-0 border-b border-[var(--portal-panel-border)] pb-2">
                            <dt class="text-[12px] font-medium text-[var(--portal-navy)]">{key.as_str()}</dt>
                            <dd class="break-all text-[13px] text-black/70">{display}</dd>
                        </div>
                    }
                })}
            </dl>
        </section>
    }
}

fn property_person_editor(
    model: &crate::model::Model,
    property: &PortalOpsProperty,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = model.ops.selected_person.as_ref();
    let linked_id = value(model, "sellerPersonId");
    let name = selected
        .map(|person| person.display_name.as_str())
        .or(property.seller_name.as_deref())
        .unwrap_or("No Person linked");
    let phone = selected
        .and_then(|person| person.phone.as_deref())
        .or(property.seller_phone.as_deref())
        .unwrap_or("—");
    let email = selected
        .and_then(|person| person.email.as_deref())
        .or(property.seller_email.as_deref())
        .unwrap_or("—");
    let location = selected
        .and_then(|person| person.location.as_deref())
        .or(property.seller_location.as_deref())
        .unwrap_or("—");

    let on_query = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::OpsPersonQueryChanged(value));
        })
    };
    let clear = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsPersonSelected(String::new())))
    };

    html! {
        <div class="space-y-4">
            {section_intro(
                "Person",
                "Link the Property to a canonical Person using the identity humans actually know: name, phone or email. The UUID stays visible for diagnostics, not as the picker.",
            )}
            <div class="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
                <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
                    <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                        {"Linked seller / owner"}
                    </div>
                    <div class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{name}</div>
                    <div class="mt-4 grid gap-3 sm:grid-cols-2">
                        {readonly_card("Phone", phone)}
                        {readonly_card("Email", email)}
                        {readonly_card("Location", location)}
                        {readonly_card("Person ID", if linked_id.is_empty() { "—" } else { linked_id.as_str() })}
                    </div>
                </section>

                <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                                {"Find Person"}
                            </div>
                            <p class="mt-1 text-[11px] font-light text-black/45">{"Search name, phone or email."}</p>
                        </div>
                        if !linked_id.is_empty() {
                            <button
                                type="button"
                                onclick={clear}
                                class="text-[10px] font-semibold uppercase tracking-[0.11em] text-black/45 hover:text-[var(--portal-navy)]"
                            >
                                {"Clear link"}
                            </button>
                        }
                    </div>
                    <input
                        type="search"
                        value={model.ops.person_query.clone()}
                        oninput={on_query}
                        placeholder="Lisa · 787… · lisa@…"
                        class="mt-3 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light outline-none focus:border-[var(--portal-navy)]"
                    />
                    if model.ops.person_searching {
                        <div class="px-1 py-3 text-[11px] font-light text-black/40">{"Searching…"}</div>
                    } else if !model.ops.person_people.is_empty() {
                        <div class="mt-2 max-h-64 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55">
                            {for model.ops.person_people.iter().map(|person| {
                                let id = person.id.clone();
                                let onclick = {
                                    let on_msg = on_msg.clone();
                                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsPersonSelected(id.clone())))
                                };
                                let detail = person
                                    .phone
                                    .as_deref()
                                    .or(person.email.as_deref())
                                    .or(person.location.as_deref())
                                    .unwrap_or("No contact identity");
                                html! {
                                    <button
                                        type="button"
                                        {onclick}
                                        class="flex w-full items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-3 py-2.5 text-left last:border-b-0 hover:bg-white/75"
                                    >
                                        <span class="min-w-0">
                                            <span class="block truncate text-[13px] font-medium text-[var(--portal-navy)]">
                                                {person.display_name.clone()}
                                            </span>
                                            <span class="mt-0.5 block truncate text-[11px] font-light text-black/45">{detail}</span>
                                        </span>
                                        <span class="shrink-0 text-[9px] uppercase tracking-[0.1em] text-black/35">
                                            {person.role.clone()}
                                        </span>
                                    </button>
                                }
                            })}
                        </div>
                    } else if model.ops.person_query.trim().len() >= 2 {
                        <div class="px-1 py-3 text-[11px] font-light text-black/40">{"No matching people."}</div>
                    }
                </section>
            </div>

            <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/25 p-4">
                <div class="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">{"Property record"}</div>
                <div class="mt-1 grid gap-x-6 sm:grid-cols-3">
                    {read_line("Property ID", &property.id)}
                    {read_line("Created", property.created_at.as_deref().unwrap_or("—"))}
                    {read_line("Updated", property.updated_at.as_deref().unwrap_or("—"))}
                </div>
            </section>
        </div>
    }
}

fn video_editor(
    property: &PortalOpsProperty,
    media: &[PortalOpsMediaAsset],
    on_msg: &Callback<Msg>,
) -> Html {
    let videos = media
        .iter()
        .filter(|item| item.media_type == "video" && item.mux_playback_id.is_some())
        .collect::<Vec<_>>();
    let films = videos.iter().filter(|item| item.role == "video").count() as i64;
    let shorts = videos.iter().filter(|item| item.role == "short").count() as i64;
    let payload = serde_json::json!({
        "propertyId": property.id,
        "propertyName": property.name,
        "videos": videos.iter().map(|item| {
            serde_json::json!({
                "id": item.id,
                "role": item.role,
                "caption": item.caption,
                "muxAssetId": item.mux_asset_id,
                "playbackId": item.mux_playback_id,
                "durationSeconds": item.duration_seconds,
                "aspectRatio": item.aspect_ratio,
            })
        }).collect::<Vec<_>>(),
    })
    .to_string();
    let refresh = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsVideoRefreshRequested))
    };

    html! {
        <div class="space-y-4">
            {section_intro(
                "Video",
                "Property films stream from Mux. The playback identity stays canonical in Media while the large source file lives at Mux.",
            )}
            <div class="grid gap-3 sm:grid-cols-3">
                {count_card("Videos", property.video_count)}
                {count_card("Property films", films)}
                {count_card("Short films", shorts)}
            </div>
            <div
                id="opps-video-island"
                data-video-widget={payload}
                class="min-h-[360px] overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30"
            />
            <button id="opps-video-refresh" type="button" onclick={refresh} class="hidden" aria-hidden="true">
                {"Refresh video"}
            </button>
        </div>
    }
}

fn media_editor(
    model: &crate::model::Model,
    property: &PortalOpsProperty,
    media: &[PortalOpsMediaAsset],
    on_msg: &Callback<Msg>,
) -> Html {
    let mut images = media
        .iter()
        .filter(|item| item.media_type == "image")
        .collect::<Vec<_>>();
    images.sort_by_key(|item| if item.role == "hero" { 0 } else { 1 });

    let active_index = if images.is_empty() {
        0
    } else {
        model.ops.media_index.min(images.len() - 1)
    };
    let active = images.get(active_index).copied();
    let previous_index = if images.is_empty() {
        0
    } else {
        (active_index + images.len() - 1) % images.len()
    };
    let next_index = if images.is_empty() {
        0
    } else {
        (active_index + 1) % images.len()
    };

    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(previous_index)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(next_index)))
    };
    let file_change = media_file_change(on_msg);

    html! {
        // `min-w-0` IS THE FIX FOR "IT GOES PAST THE LEFT WIDGET". This column is a flex/grid child, and such a child
        // defaults to `min-width: auto` — it refuses to shrink below its content's intrinsic width. A 6000px
        // photograph therefore widened the column past the sidebar instead of being scaled into it. With `min-w-0`
        // the column can be no wider than the space it was given, and `max-w-full` keeps the media inside it.
        <div class="min-w-0 max-w-full space-y-4">
            // NO SECTION INTRO HERE. It said "Review the Property photography in-place, then add the next photo
            // without leaving the canonical record" — a sentence that describes the screen to someone who has not
            // seen it, placed on a screen only reached by someone who already knows why they are there. The tab is
            // called Photos.

            // THE FAILURE HAS TO BE VISIBLE. Every op in this screen writes its problem into `model.error`, and this
            // view never rendered it — so an upload that failed said nothing at all. "Nothing happened" is the most
            // expensive bug report there is: it describes a screen, not a cause.
            if let Some(error) = model.error.clone() {
                <div class="rounded-[var(--portal-tab-radius)] border border-red-400/60 bg-red-50 px-3 py-2 text-[12px] font-light text-red-700">
                    {error}
                </div>
            }

            // NO COUNT STRIP HERE. It printed "6 photos · 0 hero · 0 documents" one line above the uploader while the
            // Property header already carried "MEDIA — 6 images": the same number, twice, and the second copy was the
            // one in the reading path. The counts live in the header (Media, Hero, Documents) — once each. This tab
            // is for the photographs.

            // THE UPLOADER IS ALWAYS HERE, not behind a toggle. It used to need a "+ Add new photo" click to appear,
            // which made a second control for a job the panel's own button already does. One screen, one upload
            // button: the one inside this panel.
            {ops_media_uploader(model, on_msg)}

            <section class="overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-navy)]">
                if let Some(image) = active {
                    // A MEDIA MANAGER, NOT A CAROUSEL.
                    //
                    // This was one photograph stretched edge to edge and cropped to fill a full-width band — which
                    // misrepresents the picture (a portrait shot became a slice of its middle) and made the panel's
                    // height jump between photographs. Now the selected photograph is large on the LEFT, contained
                    // (`object-contain`) so its proportions are its own, in a box of stable height; and every uploaded
                    // photograph is a thumbnail on the RIGHT. The bottom filmstrip is gone — it duplicated that grid.
                    <div class="grid min-w-0 gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,190px)]">
                        <div class="min-w-0">
                    <div class="relative h-[300px] w-full max-w-full overflow-hidden rounded-[var(--portal-tab-radius)] bg-black/85 sm:h-[380px] xl:h-[440px]">
                        <img
                            src={image.url.clone()}
                            alt={image.alt_text.clone().unwrap_or_else(|| property.name.clone())}
                            class="h-full w-full object-contain"
                        />
                        <div class="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/50 via-transparent to-black/10"></div>
                        <div class="absolute left-4 top-4 flex gap-2">
                            if image.role == "hero" {
                                <span class="rounded-full border border-white/35 bg-black/30 px-2.5 py-1 text-[9px] font-semibold uppercase tracking-[0.13em] text-white backdrop-blur-sm">
                                    {"Hero"}
                                </span>
                            }
                            <span class="rounded-full border border-white/30 bg-black/25 px-2.5 py-1 text-[9px] font-medium text-white/90 backdrop-blur-sm">
                                {format!("{} / {}", active_index + 1, images.len())}
                            </span>
                        </div>
                        if images.len() > 1 {
                            <button
                                type="button"
                                onclick={previous}
                                aria-label="Previous photo"
                                class="absolute left-3 top-1/2 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                            >
                                {"‹"}
                            </button>
                            <button
                                type="button"
                                onclick={next}
                                aria-label="Next photo"
                                class="absolute right-3 top-1/2 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                            >
                                {"›"}
                            </button>
                        }
                        <div class="absolute inset-x-0 bottom-0 p-4 text-white">
                            <div class="text-[11px] font-medium">{image.filename.clone().unwrap_or_else(|| property.name.clone())}</div>
                            if let Some(caption) = image.caption.as_deref() {
                                <div class="mt-1 text-[10px] font-light text-white/70">{caption}</div>
                            }
                        </div>
                    </div>

                        </div>

                        // THE UPLOADED PHOTOGRAPHS, BESIDE THE SELECTED ONE. Two columns where there is room, one
                        // where there is not, scrolling inside the panel — a media manager instead of a strip that
                        // ran off the bottom and capped itself at eight. `object-contain` on a fixed-ratio frame,
                        // because a thumbnail that crops or stretches is a thumbnail you cannot judge.
                        <div class="min-w-0">
                            <div class="grid max-h-[440px] grid-cols-2 gap-1.5 overflow-y-auto pr-0.5 max-lg:grid-cols-3 max-sm:grid-cols-2">
                                {for images.iter().enumerate().map(|(index, image)| {
                                    let onclick = {
                                        let on_msg = on_msg.clone();
                                        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(index)))
                                    };
                                    html! {
                                        <button
                                            type="button"
                                            {onclick}
                                            aria-current={(index == active_index).to_string()}
                                            class={classes!(
                                                "relative","aspect-[4/3]","w-full","overflow-hidden","rounded-[var(--portal-tab-radius)]","border","bg-black/70","transition",
                                                if index == active_index { "border-[var(--portal-gold)] opacity-100" } else { "border-transparent opacity-65 hover:opacity-100" }
                                            )}
                                        >
                                            <img src={image.url.clone()} alt="" class="h-full w-full object-contain" />
                                            if image.role == "hero" {
                                                <span class="absolute left-1 top-1 rounded-full bg-black/55 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-[0.1em] text-white">
                                                    {"Hero"}
                                                </span>
                                            }
                                        </button>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                } else {
                    <div class="grid h-[300px] place-items-center p-8 text-center sm:h-[390px]">
                        <div>
                            <div class="font-serif text-2xl font-light text-white">{"No photos yet"}</div>
                            <p class="mt-2 text-[12px] font-light text-white/55">{"Add the first Property image below."}</p>
                        </div>
                    </div>
                }
            </section>

            // THE FILE INPUT LIVES HERE, ALWAYS, HIDDEN — not inside the uploader panel. A file input can only be
            // opened from a user gesture, so an input that only exists once the panel is open cannot be opened BY
            // the click that opens that panel: the input would be created after the gesture that needed it. Keeping
            // it in the tree lets "+ Add new photo" open the picker directly.
            <input
                id="ops-media-file"
                type="file"
                accept="image/*"
                onchange={file_change}
                class="hidden"
            />

            // NO SECOND BUTTON HERE. This row carried a caption and a "+ Add new photo" / "Close uploader" button —
            // a duplicate of the control inside the uploader panel, which is the one upload button on this screen. A
            // row whose only job is to duplicate a control above it is how a screen ends up with three buttons for
            // one action, and the extra two are what made this look broken.
        </div>
    }
}

/// Opens the listing-media file input — the hidden one that lives in the Photos tab at all times.
///
/// A file picker may only be opened from a USER GESTURE, so this is called from inside the click handler rather than
/// from an effect afterwards: by the time an effect runs, the gesture is over and the browser refuses to open it. That
/// is also why the input is not created by the button — an element created by a click cannot be clicked by the same
/// click.
///
/// If the element is missing this does nothing at all, deliberately: the uploader's own "Choose file" button is the
/// fallback, so a lookup that fails leaves a usable screen rather than a panic.
fn open_file_picker() {
    // `web_sys` re-exports the cast trait, and this file's other casts come from Yew's prelude which does not include
    // it — so it is brought in here, locally, rather than widening the module's imports for one call.
    use web_sys::wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id("ops-media-file") else {
        return;
    };
    if let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() {
        input.click();
    }
}

/// The chosen-file callback, shared by the hidden input and the uploader panel so both write the same message field.
fn media_file_change(on_msg: &Callback<Msg>) -> Callback<Event> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: Event| {
        let input = event.target_unchecked_into::<web_sys::HtmlInputElement>();
        let name = input
            .files()
            .and_then(|files| files.get(0))
            .map(|file| file.name())
            .unwrap_or_default();
        on_msg.emit(Msg::OpsMediaFileChosen(name));
    })
}

fn ops_media_uploader(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::OpsMediaRoleChanged(value));
        })
    };
    let alt_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::OpsMediaAltChanged(value));
        })
    };
    let file_change = media_file_change(on_msg);
    let choose_again = Callback::from(move |_: MouseEvent| open_file_picker());

    html! {
        <section class="min-w-0 max-w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
            // THE BUTTON IS FIRST, AND THE FIELDS FOLLOW — the ordering is the point.
            //
            // This panel is why the screen felt broken: it appeared only after a click, then rendered under the
            // gallery, so the one control that does the job sat below the fold beneath two fields that describe a file
            // which does not exist yet. Now the button is the first thing in the panel, so it is on screen the moment
            // the tab opens, and Role and Alt text — which only describe a photograph that has already been chosen —
            // come after it.
            <div class="flex flex-wrap items-center justify-between gap-3">
                <span class="min-w-0 flex-1 truncate text-[11px] font-light text-black/45">
                    {model.ops.media_file_name.clone().unwrap_or_else(|| "Choose a photo — the upload starts by itself".into())}
                </span>
                // RIGHT-ALIGNED, because every other action on this screen is. A single button sitting on the left while
                // Save, Revert and the rest sit on the right reads as a different kind of control than it is.
                if model.ops.media_uploading {
                    <span class="shrink-0 text-[11px] font-light text-[var(--portal-gold-muted)]">{"Uploading…"}</span>
                }
                <button
                    type="button"
                    onclick={choose_again}
                    class="inline-flex h-9 shrink-0 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white"
                >
                    {"Add photo"}
                </button>
            </div>
            <div class="mt-2 grid gap-2 sm:grid-cols-2">
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Image role"}
                    <select
                        value={model.ops.media_role.clone()}
                        onchange={role_change}
                        class="mt-1 h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
                    >
                        <option value="gallery">{"Gallery"}</option>
                        <option value="hero">{"Hero"}</option>
                    </select>
                </label>
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Alt text"}
                    <input
                        value={model.ops.media_alt.clone()}
                        oninput={alt_change}
                        placeholder="Oceanfront villa overlooking Culebra"
                        class="mt-1 h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
                    />
                </label>
            </div>
        </section>
    }
}

fn person_editor(
    model: &crate::model::Model,
    person: Option<&PortalOpsPerson>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(person) = person else {
        return empty_record("Person");
    };

    match model.ops.section.as_str() {
        "contact" => html! {
            <div class="space-y-4">
                {section_intro("Contact", "Person identities stay canonical. This slice shows existing email and phone values without inventing a second identity writer.")}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Email", person.email.as_deref().unwrap_or("—"))}
                    {readonly_card("Phone", person.phone.as_deref().unwrap_or("—"))}
                    {readonly_card("Location", person.location.as_deref().unwrap_or("—"))}
                    {readonly_card("Role", &person.role)}
                </div>
            </div>
        },
        "relations" => html! {
            <div class="space-y-4">
                {section_intro("Relations", "Open the relationship workspace for communication history, properties, deals and operational context.")}
                <a href={format!("/portal/clients/{}", person.id)}
                    class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[11px] font-semibold uppercase tracking-[0.12em] text-white">
                    {"Open Client Relationship →"}
                </a>
            </div>
        },
        _ => html! {
            <div class="space-y-4">
                {section_intro("Person identity", "The same workbench shell, backed by the canonical Person service.")}
                {field_panel(model, on_msg, "Canonical person", PERSON_FIELDS)}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Role", &person.role)}
                    {readonly_card("Canonical ID", &person.id)}
                </div>
            </div>
        },
    }
}

fn project_editor(
    model: &crate::model::Model,
    project: Option<&PortalOpsProject>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(project) = project else {
        return empty_record("Project");
    };

    match model.ops.section.as_str() {
        "links" => html! {
            <div class="space-y-4">
                {section_intro("Project links", "Typed links connect a Project to its playbook and canonical Person, Property or Contract context.")}
                {field_panel(model, on_msg, "Bindings", PROJECT_LINKS)}
                <div class="grid gap-3 lg:grid-cols-2">
                    {readonly_card("Starts", project.starts_at.as_deref().unwrap_or("—"))}
                    {readonly_card("Ends", project.ends_at.as_deref().unwrap_or("—"))}
                </div>
            </div>
        },
        _ => html! {
            <div class="space-y-4">
                {section_intro("Project", "Edit the Project record without leaving OPPS. The full Project workspace remains the execution surface.")}
                {field_panel(model, on_msg, "Project facts", PROJECT_FIELDS)}
                <a href="/portal/projects"
                    class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-4 text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)]">
                    {"Open Project Workspace →"}
                </a>
            </div>
        },
    }
}

fn field_panel(
    model: &crate::model::Model,
    on_msg: &Callback<Msg>,
    title: &str,
    fields: &[FieldSpec],
) -> Html {
    html! {
        <section class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 p-4">
            <div class="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--portal-gold-muted)]">{title}</div>
            {field_grid(model, on_msg, fields)}
        </section>
    }
}

fn field_grid(model: &crate::model::Model, on_msg: &Callback<Msg>, fields: &[FieldSpec]) -> Html {
    html! {
        <div class="grid gap-4 lg:grid-cols-2">
            {for fields.iter().map(|field| editor_field(model, on_msg, field))}
        </div>
    }
}

fn editor_field(model: &crate::model::Model, on_msg: &Callback<Msg>, field: &FieldSpec) -> Html {
    let field_value = value(model, field.key);
    let wrapper = if field.wide { "lg:col-span-2" } else { "" };
    let disabled = model.ops.saving;

    let control = match field.kind {
        FieldKind::Textarea(rows) => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <textarea
                    value={field_value}
                    rows={rows.to_string()}
                    disabled={disabled}
                    oninput={Callback::from(move |event: InputEvent| {
                        let value = event.target_unchecked_into::<web_sys::HtmlTextAreaElement>().value();
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 py-2 text-[13px] font-light leading-relaxed text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                />
            }
        }
        FieldKind::Toggle => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <div class="mt-2 flex h-10 items-center">
                    <input
                        type="checkbox"
                        checked={field_value == "true"}
                        disabled={disabled}
                        onchange={Callback::from(move |event: Event| {
                            let checked = event.target_unchecked_into::<web_sys::HtmlInputElement>().checked();
                            on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value: checked.to_string() });
                        })}
                        class="h-4 w-4 rounded border-[var(--portal-panel-border)]"
                    />
                </div>
            }
        }
        FieldKind::Select(options) => {
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <select
                    value={field_value}
                    disabled={disabled}
                    onchange={Callback::from(move |event: Event| {
                        let value = event.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                >
                    {for options.iter().map(|(value, label)| html! { <option value={*value}>{*label}</option> })}
                </select>
            }
        }
        FieldKind::Date | FieldKind::Number | FieldKind::Text => {
            let input_type = match field.kind {
                FieldKind::Date => "date",
                FieldKind::Number => "number",
                _ => "text",
            };
            let step = if matches!(field.kind, FieldKind::Number) {
                "any"
            } else {
                ""
            };
            let key = field.key.to_string();
            let on_msg = on_msg.clone();
            html! {
                <input
                    type={input_type}
                    step={step}
                    value={field_value}
                    disabled={disabled}
                    oninput={Callback::from(move |event: InputEvent| {
                        let value = event.target_unchecked_into::<web_sys::HtmlInputElement>().value();
                        on_msg.emit(Msg::OpsFieldChanged { key: key.clone(), value });
                    })}
                    class="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light text-black/75 outline-none focus:border-[var(--portal-navy)] disabled:opacity-50"
                />
            }
        }
    };

    html! {
        <div class={wrapper}>
            <label class="block text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                {field.label}
                {control}
            </label>
            if let Some(hint) = field.hint {
                <p class="mt-1 text-[10px] font-light leading-snug text-black/40">{hint}</p>
            }
        </div>
    }
}

fn section_intro(title: &str, body: &str) -> Html {
    html! {
        <div>
            <div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{title}</div>
            <p class="mt-1 max-w-4xl text-[13px] font-light leading-relaxed text-black/55">{body}</p>
        </div>
    }
}

fn inherited(label: &str, value: String) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] p-3">
            <div class="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">{label}</div>
            <div class="mt-1 truncate text-[12px] font-medium text-[var(--portal-navy)]">
                {if value.trim().is_empty() { "—".into() } else { value }}
            </div>
            <div class="mt-1 text-[9px] uppercase tracking-[0.1em] text-black/30">{"Inherited from Property"}</div>
        </div>
    }
}

fn count_card(label: &str, count: i64) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
            <div class="font-serif text-3xl font-light text-[var(--portal-navy)]">{count}</div>
            <div class="mt-1 text-[10px] font-semibold uppercase tracking-[0.13em] text-black/40">{label}</div>
        </div>
    }
}

fn readonly_card(label: &str, value: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
            <div class="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">{label}</div>
            <div class="mt-1 break-words text-[13px] font-light text-[var(--portal-navy)]">{value}</div>
        </div>
    }
}

fn read_line(label: &str, value: &str) -> Html {
    html! {
        <div class="mt-2 flex gap-3 text-[12px]">
            <span class="w-20 shrink-0 text-black/35">{label}</span>
            <span class="min-w-0 break-all font-light text-[var(--portal-navy)]">{value}</span>
        </div>
    }
}

fn empty_editor(loading: bool) -> Html {
    html! {
        <section class="portal-glass-panel grid h-full min-h-64 place-items-center rounded-[var(--portal-panel-radius)] p-8 text-center">
            <div>
                <div class="font-serif text-xl font-light text-[var(--portal-navy)]">
                    {if loading { "Loading workbench…" } else { "Select a record" }}
                </div>
                <p class="mt-2 text-[12px] font-light text-black/45">
                    {"The same workspace edits every major entity."}
                </p>
            </div>
        </section>
    }
}

fn empty_record(label: &str) -> Html {
    html! {
        <div class="py-12 text-center text-sm font-light text-black/45">
            {format!("No {label} record is loaded.")}
        </div>
    }
}

fn entity_plural(entity: &str) -> &'static str {
    match entity {
        "person" => "People",
        "project" => "Projects",
        _ => "Properties",
    }
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "active" | "doing" | "open" => "bg-[var(--portal-success)]",
        "archived" | "done" | "sold" => "bg-black/30",
        "coming_soon" | "warm" => "bg-[var(--portal-gold)]",
        _ => "bg-[var(--portal-blue-gray)]",
    }
}

fn compact_meta(subtitle: Option<&str>, meta: Option<&str>, status: &str) -> String {
    [Some(status), subtitle, meta]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.replace('_', " "))
        .collect::<Vec<_>>()
        .join(" · ")
}
