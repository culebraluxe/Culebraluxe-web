//! The record editor shell: header, metrics, section tabs, the property editor and its panels.

use super::*;

pub(super) fn editor(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn editor_header(model: &Vm<'_>, data: &PortalOpsWorkbenchPage, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn header_metrics(data: &PortalOpsWorkbenchPage) -> Html {
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

pub(super) fn metric(label: &str, value: &str) -> Html {
    html! {
        <div class="text-right">
            <div class="text-[9px] uppercase tracking-[0.14em] text-black/35">{label}</div>
            <div class="text-[12px] font-medium text-[var(--portal-navy)]">{value}</div>
        </div>
    }
}

pub(super) fn section_tabs(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn property_editor(
    model: &Vm<'_>,
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
                {field_panel(model, on_msg, "GPS coordinates", PROPERTY_GPS)}
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
                // The Administration panel is gone with its toggle: Status owns archiving now.
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
        "video" => video_editor(model, property, media, on_msg),
        "person" => property_person_editor(model, property, on_msg),
        _ => html! {
            <div class="space-y-4">
                {section_intro("Property facts", "Start with what is known. Save and return to complete other fields in later passes.")}
                {property_pane(model, on_msg)}
                {field_panel(model, on_msg, "Description", PROPERTY_DESCRIPTIONS)}
                {field_panel(model, on_msg, "Listing and building facts", PROPERTY_CORE)}
            </div>
        },
    }
}

pub(super) fn feature_panel(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
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
                                    let checked = crate::app::exec::checked(&event);
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

pub(super) fn source_columns_panel(
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
