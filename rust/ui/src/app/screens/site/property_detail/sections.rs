//! The property's sections: overview, details, location, videos and documents.

#[allow(unused_imports)]
use super::*;

pub(super) fn detail_sections(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
    let mut tabs = vec![
        (PropertyTab::Overview, "Overview"),
        (PropertyTab::Details, "Details"),
    ];
    if !record.videos.is_empty() {
        tabs.push((PropertyTab::Video, "Video"));
    }
    if !record.documents.is_empty() {
        tabs.push((PropertyTab::Documents, "Documents"));
    }
    tabs.push((PropertyTab::Map, "Map"));
    let active = model.property_media.tab;
    let content = match active {
        PropertyTab::Overview => overview(record),
        PropertyTab::Details => details(record),
        PropertyTab::Video => videos(record),
        PropertyTab::Documents => documents(record),
        PropertyTab::Map => location(record),
    };
    html! {
        <section class="w-full border-x border-b border-brand-navy/35 bg-card">
            <nav class="flex h-[60px] flex-nowrap overflow-x-auto border-b border-brand-navy/35 bg-card sm:h-16" aria-label="Property information">
                { for tabs.into_iter().map(|(tab, label)| {
                    let selected = active == tab;
                    let on_msg = on_msg.clone();
                    let onclick = Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyTabSelected(tab)));
                    html! {
                        <button type="button" {onclick} aria-current={selected.then_some("page")}
                            class={format!(
                                "relative flex min-h-12 min-w-[116px] flex-none self-stretch items-center justify-center border-r border-brand-gold px-6 font-serif text-sm tracking-[0.04em] transition-colors duration-300 sm:min-w-[132px] sm:px-8 sm:text-[15px] {}",
                                if selected { "bg-brand-navy font-semibold text-brand-gold" } else { "bg-brand-navy font-medium text-brand-ivory/85 hover:text-brand-ivory" }
                            )}>
                            {label}
                            <span class={if selected { "absolute inset-x-0 bottom-0 h-0.5 bg-brand-gold transition-colors duration-300" } else { "absolute inset-x-0 bottom-0 h-0.5 bg-transparent transition-colors duration-300" }}></span>
                        </button>
                    }
                }) }
            </nav>
            <div class="bg-brand-navy/[0.035] p-5 shadow-[inset_0_0_0_1px_rgba(3,15,35,0.16)] sm:p-6 lg:p-7">
                {content}
            </div>
        </section>
    }
}

pub(super) fn definition(label: &str, value: Option<String>) -> Html {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Html::default();
    };
    html! {
        <div class="min-w-0 border-t border-brand-navy/40 bg-card/60 px-3 py-4 ring-1 ring-inset ring-brand-navy/25">
            <dt class="text-[10px] font-semibold uppercase tracking-[0.17em] text-brand-navy/70">{label.to_string()}</dt>
            <dd class="mt-1.5 break-words text-[15px] font-medium leading-snug text-brand-navy">{value}</dd>
        </div>
    }
}

pub(super) fn editorial_fact(label: &str, value: Option<String>) -> Html {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Html::default();
    };
    html! {
        <div class="flex items-baseline justify-between gap-5 border-b border-brand-navy/20 py-2 last:border-0">
            <dt class="text-[10px] font-semibold uppercase tracking-[0.16em] text-brand-navy/70">{label.to_string()}</dt>
            <dd class="min-w-0 break-words text-right text-sm font-medium text-brand-navy">{value}</dd>
        </div>
    }
}

pub(super) fn overview(record: &PropertyRecord) -> Html {
    let editorial = record
        .description
        .as_deref()
        .filter(|text| !text.trim().is_empty());
    let compact_amenities = record
        .amenities
        .iter()
        .filter(|item| {
            matches!(
                item.as_str(),
                "Pool"
                    | "Whole-Home Generator"
                    | "Solar Power"
                    | "Furnished"
                    | "Gated"
                    | "Water Access"
                    | "Beach Access"
            )
        })
        .collect::<Vec<_>>();
    let amenity_notes = record
        .amenities
        .iter()
        .filter(|item| {
            !matches!(
                item.as_str(),
                "Pool"
                    | "Whole-Home Generator"
                    | "Solar Power"
                    | "Furnished"
                    | "Gated"
                    | "Water Access"
                    | "Beach Access"
            )
        })
        .collect::<Vec<_>>();
    let views = &record.view_type;
    let lifestyle = record
        .lifestyle_tags
        .iter()
        .filter(|tag| {
            !record
                .amenities
                .iter()
                .any(|item| item.eq_ignore_ascii_case(tag))
                && !views.iter().any(|view| {
                    view.eq_ignore_ascii_case(tag)
                        || format!("{view} View").eq_ignore_ascii_case(tag)
                })
        })
        .collect::<Vec<_>>();
    let has_sidebar = record.lot_size.is_some()
        || record.neighborhood.is_some()
        || !views.is_empty()
        || !lifestyle.is_empty()
        || record.listing_agent_name.is_some()
        || record.listing_id.is_some();
    html! {
        <div class={if has_sidebar { "grid items-start gap-8 lg:grid-cols-[minmax(0,1.65fr)_minmax(280px,1fr)] lg:gap-12" } else { "max-w-4xl" }}>
            <div class="min-w-0">
                <p class="mb-4 text-xs font-medium uppercase tracking-[0.34em] text-brand-gold">{"The Property"}</p>
                <div class="flex max-w-[68ch] flex-col gap-5">
                    if let Some(description) = editorial {
                        { for description.split("\n\n").filter(|text| !text.trim().is_empty()).map(|paragraph| html! {
                            <p class="text-pretty text-[15px] font-normal leading-relaxed text-brand-navy/90">{paragraph.trim().to_string()}</p>
                        }) }
                    } else if let Some(short) = &record.short_description {
                        <p class="text-pretty text-base leading-7 text-brand-navy/90">{short.clone()}</p>
                    } else {
                        <p class="text-sm leading-relaxed text-brand-navy/80">{"A detailed description of this residence is being prepared."}</p>
                    }
                </div>
                if !record.amenities.is_empty() || record.architecture.is_some() {
                    <div class="mt-7 border-t border-brand-navy/20 pt-6">
                        <p class="mb-5 text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{"Property Highlights"}</p>
                        <div class="grid gap-x-10 gap-y-6 sm:grid-cols-2">
                            if !record.amenities.is_empty() {
                                <section>
                                    <h3 class="mb-3 font-serif text-base font-semibold text-brand-navy">{"Amenities"}</h3>
                                    <ul class="space-y-2 pl-5">{ for compact_amenities.iter().map(|item| html! {
                                        <li class="list-disc text-sm leading-snug text-brand-navy/90 marker:text-brand-gold">{(**item).clone()}</li>
                                    }) }</ul>
                                    { for amenity_notes.iter().map(|note| html! {
                                        <p class="mt-4 border-l border-brand-gold/50 pl-4 text-sm leading-relaxed text-brand-navy/82">{(**note).clone()}</p>
                                    }) }
                                </section>
                            }
                            if let Some(architecture) = &record.architecture {
                                <section>
                                    <h3 class="mb-3 font-serif text-base font-semibold text-brand-navy">{"Architecture"}</h3>
                                    <p class="border-l border-brand-gold/50 pl-4 text-sm leading-relaxed text-brand-navy/82">{architecture.clone()}</p>
                                </section>
                            }
                        </div>
                    </div>
                }
            </div>
            if has_sidebar {
                <aside class="min-w-0 space-y-6">
                    if record.lot_size.is_some() || record.neighborhood.is_some() {
                        <section class="border-t border-brand-navy/40 bg-brand-navy/[0.05] px-5 py-5 ring-1 ring-inset ring-brand-navy/20">
                            <p class="mb-4 text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{"Key Facts"}</p>
                            <dl>
                                { editorial_fact("Lot Size", record.lot_size.clone()) }
                                { editorial_fact("Neighborhood", record.neighborhood.clone()) }
                            </dl>
                        </section>
                    }
                    if record.listing_agent_name.is_some() || record.listing_id.is_some() {
                        <section class="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20">
                            <p class="mb-3 text-xs font-medium uppercase tracking-[0.2em] text-brand-gold">{"Listing Information"}</p>
                            <dl>
                                { editorial_fact("Listing Agent", record.listing_agent_name.clone()) }
                                { editorial_fact("Office", record.listing_office.clone()) }
                                { editorial_fact("Phone", record.listing_agent_phone.clone()) }
                                { editorial_fact("Email", record.listing_agent_email.clone()) }
                                { editorial_fact("MLS / Listing ID", record.listing_id.clone()) }
                            </dl>
                        </section>
                    }
                    if !views.is_empty() {
                        <section class="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20">
                            <h3 class="mb-3 font-serif text-base font-semibold text-brand-navy">{"Views"}</h3>
                            <ul class="grid grid-cols-2 gap-2 pl-5">{ for views.iter().map(|view| html! { <li class="list-disc text-sm text-brand-navy/90 marker:text-brand-gold">{view.clone()}</li> }) }</ul>
                        </section>
                    }
                    if !lifestyle.is_empty() {
                        <section class="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20">
                            <h3 class="mb-3 font-serif text-base font-semibold text-brand-navy">{"Lifestyle"}</h3>
                            <ul class="grid grid-cols-2 gap-2 pl-5">{ for lifestyle.iter().map(|tag| html! { <li class="list-disc text-sm text-brand-navy/90 marker:text-brand-gold">{(**tag).clone()}</li> }) }</ul>
                        </section>
                    }
                </aside>
            }
        </div>
    }
}

pub(super) fn details(record: &PropertyRecord) -> Html {
    let land = record
        .kind
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("land"));
    html! {
        <dl class="grid grid-cols-1 gap-x-8 sm:grid-cols-2 lg:grid-cols-3 lg:gap-x-10">
            { definition("Property Type", record.kind.clone()) }
            { definition("Status", record.status.clone()) }
            if !land {
                { definition("Bedrooms", record.beds.map(|value| format_number(value, ""))) }
                { definition("Bathrooms", record.baths.map(|value| {
                    let mut parts = Vec::new();
                    if let Some(full) = record.bathrooms_full { parts.push(format!("{} Full", format_number(full, ""))); }
                    if let Some(half) = record.bathrooms_half { parts.push(format!("{} Half", format_number(half, ""))); }
                    if parts.is_empty() { format_number(value, "") } else { format!("{} ({})", format_number(value, ""), parts.join(", ")) }
                })) }
                { definition("Living Area", record.area.clone()) }
                { definition("Stories", record.stories.map(|value| format_number(value, ""))) }
            }
            { definition("Lot Size", record.lot_size.clone()) }
            { definition("Lot Square Feet", record.lot_size_sqft.map(|value| format_number(value, "sq ft"))) }
            { definition("Road Frontage", record.road_frontage_feet.map(|value| format_number(value, "ft"))) }
            { definition("Road Surface", record.road_surface_type.clone()) }
            { definition("Lot Description", record.lot_description.clone()) }
            { definition("Utilities", record.utilities_notes.clone()) }
            { definition("Year Built", record.year_built.map(|value| value.to_string())) }
            { definition("Parking Spaces", record.parking_spaces.map(|value| format_number(value, ""))) }
            { definition("Neighborhood", record.neighborhood.clone()) }
            { definition("Water Access", record.water_access.then(|| "Yes".into())) }
            { definition("Beach Access", record.beach_access.then(|| "Yes".into())) }
        </dl>
    }
}

pub(super) fn location(record: &PropertyRecord) -> Html {
    let context = [
        &record.neighborhood,
        &record.city,
        &record.state_or_province,
    ]
    .into_iter()
    .filter_map(|item| item.as_deref())
    .collect::<Vec<_>>()
    .join(", ");
    let map = record.latitude.zip(record.longitude);
    html! {
        <div class="grid items-start gap-7 lg:grid-cols-[minmax(0,2fr)_minmax(280px,1fr)]">
            if let Some((lat, lng)) = map {
                <div class="h-[320px] w-full overflow-hidden border border-brand-navy/45 sm:h-[360px] lg:h-[450px]">
                    <iframe title={format!("Map of {}", record.title)} loading="lazy" referrerpolicy="no-referrer-when-downgrade"
                        src={format!("https://www.google.com/maps?q={lat},{lng}&z=14&output=embed")}
                        class="h-full w-full border-0"></iframe>
                </div>
            } else {
                <div class="flex h-[300px] items-center justify-center border border-brand-navy/45 bg-brand-navy/[0.05] px-8 text-center lg:h-[450px]">
                    <p class="font-serif text-xl font-semibold text-brand-navy">{"Private Location — precise location information is available through CulebraLuxe."}</p>
                </div>
            }
            if !context.is_empty() {
                <aside class="border border-brand-navy/40 bg-brand-navy/[0.05] px-5 py-6">
                    <p class="text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{"Location"}</p>
                    <h2 class="mt-4 font-serif text-2xl font-semibold text-brand-navy">{record.neighborhood.clone().or_else(|| record.city.clone()).unwrap_or_default()}</h2>
                    <p class="mt-5 border-t border-brand-navy/30 pt-5 text-sm text-brand-navy/90">{format!("This property is located in {context}.")}</p>
                    <dl class="mt-6">
                        { editorial_fact("Neighborhood", record.neighborhood.clone()) }
                        { editorial_fact("Municipality", record.city.clone()) }
                        { editorial_fact("Region", record.state_or_province.clone()) }
                    </dl>
                </aside>
            }
        </div>
    }
}

pub(super) fn videos(record: &PropertyRecord) -> Html {
    html! {
        <div class="flex flex-col gap-12">
            { for record.videos.iter().filter_map(|video| {
                let playback = video.playback_id.as_deref()?;
                let title = video.title.as_deref().unwrap_or("Property Film").to_string();
                Some(html! {
                    <section class="mx-auto w-full max-w-[860px]">
                        <p class="mb-5 text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{title.clone()}</p>
                        <div class="relative aspect-video overflow-hidden border border-brand-navy/45 bg-brand-navy">
                            <iframe src={format!("https://player.mux.com/{playback}")} title={title} allow="autoplay; fullscreen; picture-in-picture" allowfullscreen=true
                                class="absolute inset-0 h-full w-full border-0"></iframe>
                        </div>
                        if let Some(caption) = &video.caption { <p class="mt-4 text-sm leading-relaxed text-brand-navy/85">{caption.clone()}</p> }
                    </section>
                })
            }) }
        </div>
    }
}

pub(super) fn documents(record: &PropertyRecord) -> Html {
    html! {
        <div class="mx-auto max-w-5xl">
            <p class="mb-5 text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{"Property Documents"}</p>
            <ul class="space-y-3">{ for record.documents.iter().filter_map(|document| {
                let id = document.id.as_deref()?;
                let route = crate::app::api::links::property_document(id);
                let label = document.title.as_deref().unwrap_or("Property Document").to_string();
                Some(html! {
                    <li class="flex flex-col gap-4 border border-brand-navy/30 bg-card/70 p-4 sm:flex-row sm:items-center sm:justify-between sm:px-5">
                        <div class="flex min-w-0 items-center gap-3.5">
                            <span class="flex h-10 w-10 flex-none items-center justify-center border border-brand-gold/55 bg-brand-gold/10 text-brand-gold">
                                { icon_html("file-text", "h-5 w-5", "2").unwrap_or_default() }
                            </span>
                            <div class="min-w-0">
                                <h3 class="font-serif text-base font-semibold leading-snug text-brand-navy">{label.clone()}</h3>
                                <p class="mt-1 text-[11px] font-medium uppercase tracking-[0.14em] text-brand-navy/65">{document.mime_type.clone().unwrap_or_default()}</p>
                            </div>
                        </div>
                        <div class="flex flex-none items-center gap-2 sm:justify-end">
                            <a href={route.clone()} target="_blank" rel="noopener noreferrer" aria-label={format!("View {label}")} class="inline-flex min-h-11 items-center justify-center gap-2 border border-brand-navy/35 px-4 text-xs font-semibold uppercase tracking-[0.1em] text-brand-navy transition-colors hover:bg-brand-navy/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                                {"View"}{ icon_html("external-link", "h-3.5 w-3.5", "2").unwrap_or_default() }
                            </a>
                            <a href={format!("{route}?download=1")} aria-label={format!("Download {label}")} class="inline-flex min-h-11 items-center justify-center gap-2 bg-brand-navy px-4 text-xs font-semibold uppercase tracking-[0.1em] text-brand-ivory transition-colors hover:bg-brand-navy/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                                {"Download"}{ icon_html("download", "h-3.5 w-3.5", "2").unwrap_or_default() }
                            </a>
                        </div>
                    </li>
                })
            }) }</ul>
        </div>
    }
}
