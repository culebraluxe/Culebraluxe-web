//! /properties/:slug — the public property record, on Yew.
//!
//! This is the port of the LAST TypeScript PropertyMediaPanel before the Rust cutover,
//! not the simplified string renderer that replaced it. Carousel selection and lightbox
//! state stay in Model/Msg/update; this component is a pure projection plus callbacks.

use yew::prelude::*;

use crate::icons::icon_html;
use crate::model::{MediaItem, Model, Msg, PropertyRecord, PropertyTab};

#[derive(Clone, PartialEq)]
struct Photo {
    src: String,
    alt: String,
    caption: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct PropertyDetailProps {
    pub model: Model,
    pub on_msg: Callback<Msg>,
}

pub struct PropertyDetail;

impl Component for PropertyDetail {
    type Message = ();
    type Properties = PropertyDetailProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let model = &ctx.props().model;
        let on_msg = &ctx.props().on_msg;
        let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) else {
            return html! {
                <section class="px-6 py-20 md:px-12">
                    <div class="mx-auto max-w-[1600px]">
                        <p class="text-sm font-light text-muted-foreground">
                            { if model.loading { "Loading property…" } else { "This property could not be loaded." } }
                        </p>
                    </div>
                </section>
            };
        };

        let on_keydown = {
            let on_msg = on_msg.clone();
            let lightbox_open = model.property_media.lightbox_open;
            Callback::from(move |event: KeyboardEvent| {
                match event.key().as_str() {
                    "ArrowLeft" => {
                        event.prevent_default();
                        if lightbox_open {
                            on_msg.emit(Msg::PropertyLightboxMoved(-1));
                        } else {
                            on_msg.emit(Msg::PropertyMediaPrevious);
                        }
                    }
                    "ArrowRight" => {
                        event.prevent_default();
                        if lightbox_open {
                            on_msg.emit(Msg::PropertyLightboxMoved(1));
                        } else {
                            on_msg.emit(Msg::PropertyMediaNext);
                        }
                    }
                    "Escape" if lightbox_open => {
                        event.prevent_default();
                        on_msg.emit(Msg::PropertyLightboxClosed);
                    }
                    _ => {}
                }
            })
        };

        html! {
            <div tabindex="0" onkeydown={on_keydown} class="outline-none">
                <div class="mx-auto max-w-[1600px] px-6 py-8 md:px-12 md:py-10">
                    { breadcrumb(record) }
                    { cockpit(record, model, on_msg) }
                    { detail_sections(record, model, on_msg) }
                    { similar_properties(record) }
                    { recently_viewed(model) }
                </div>
                { lightbox(record, model, on_msg) }
            </div>
        }
    }
}

fn photos(record: &PropertyRecord) -> Vec<Photo> {
    let mut result = Vec::new();
    let hero = record.hero_url.as_deref();

    if let Some(hero_src) = hero {
        let hero_item = record
            .gallery
            .iter()
            .find(|item| item.src().as_deref() == Some(hero_src));
        result.push(Photo {
            src: hero_src.to_string(),
            alt: hero_item
                .and_then(MediaItem::text)
                .unwrap_or(record.title.as_str())
                .to_string(),
            caption: hero_item.and_then(|item| item.caption.clone()),
        });
    }

    let mut removed_hero = false;
    for item in &record.gallery {
        let Some(src) = item.src() else { continue };
        if !removed_hero && hero.is_some_and(|hero_src| hero_src == src) {
            removed_hero = true;
            continue;
        }
        result.push(Photo {
            alt: item.text().unwrap_or(record.title.as_str()).to_string(),
            caption: item.caption.clone(),
            src,
        });
    }

    result
}

fn breadcrumb(record: &PropertyRecord) -> Html {
    let chevron = icon_html("chevron-right", "h-3 w-3", "2").unwrap_or_default();
    html! {
        <nav aria-label="Breadcrumb"
            class="mb-6 flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.18em] text-muted-foreground">
            <a href="/" class="transition-colors hover:text-foreground">{"Home"}</a>
            { chevron.clone() }
            <a href="/buyers" class="transition-colors hover:text-foreground">{"Properties"}</a>
            { chevron }
            <span class="text-foreground">{ record.title.clone() }</span>
        </nav>
    }
}

fn cockpit(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
    let land = record.kind.as_deref().is_some_and(|kind| kind.eq_ignore_ascii_case("land"));
    let facts = [
        if land { None } else { record.beds.map(|value| ("Beds", format_number(value, ""))) },
        if land { None } else { record.baths.map(|value| ("Baths", format_number(value, ""))) },
        if land { None } else { record.area.clone().map(|value| ("Interior", value)) },
        if land { record.lot_size.clone().map(|value| ("Lot", value)) } else { None },
        record.year_built.map(|value| ("Built", value.to_string())),
        record.parking_spaces.map(|value| ("Parking", format_number(value, ""))),
        record.stories.map(|value| ("Stories", format_number(value, ""))),
    ].into_iter().flatten().collect::<Vec<_>>();
    let viewing = format!("/contact?propertyId={}&requestType=private_viewing#contact", record.id);
    let saved = model.property_media.saved;
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyFavoriteToggled))
    };

    html! {
        <section class="overflow-hidden border border-brand-navy/20 bg-card shadow-[0_14px_36px_rgba(3,15,35,0.06)] lg:grid lg:min-h-[432px] lg:grid-cols-[minmax(0,1.5fr)_minmax(380px,1fr)]">
            <div class="relative h-[290px] sm:h-[370px] lg:h-[432px]">
                { media_panel(record, model, on_msg) }
            </div>
            <div class="flex flex-col border-l-0 border-brand-navy/15 bg-card lg:border-l">
                <div class="border-b border-brand-navy/15 px-5 py-3.5 lg:px-6">
                    if let Some(location) = record.location.as_ref().filter(|value| !value.is_empty()) {
                        <p class="mb-2 flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.18em] text-brand-navy/65">
                            { icon_html("map-pin", "h-3.5 w-3.5 text-brand-gold", "2").unwrap_or_default() }
                            { location.clone() }
                        </p>
                    }
                    <h1 class="text-balance font-serif text-2xl font-medium leading-tight text-brand-navy xl:text-3xl">
                        { record.title.clone() }
                    </h1>
                    <p class="mt-1.5 text-lg font-medium text-foreground/90">
                        { record.price.clone().unwrap_or_else(|| "Price Upon Request".into()) }
                    </p>
                    <div class="mt-2.5 flex flex-wrap gap-3 text-[10px] font-medium uppercase tracking-[0.16em] text-brand-navy/70">
                        if let Some(kind) = &record.kind { <span>{kind.clone()}</span> }
                        if let Some(status) = &record.status { <span class="text-brand-gold">{status.clone()}</span> }
                    </div>
                </div>

                <div class="flex flex-1 items-center px-5 py-3 lg:px-6">
                    <div class="w-full">
                        <p class="mb-2 text-[10px] font-semibold uppercase tracking-[0.2em] text-brand-navy/75">{"Key Facts"}</p>
                        <dl class="grid grid-cols-2 gap-2 xl:grid-cols-3">
                        { for facts.into_iter().map(|(label, value)| html! {
                            <div class="flex min-h-12 flex-col justify-center border border-brand-navy/35 bg-brand-navy/[0.04] px-2 py-1.5">
                                <dt class="text-[9px] font-semibold uppercase tracking-[0.12em] text-brand-navy/70">{label}</dt>
                                <dd class="truncate text-sm font-medium text-brand-navy">{value.trim().to_string()}</dd>
                            </div>
                        }) }
                        </dl>
                    </div>
                </div>
                <div class="border-t border-brand-navy/15 bg-brand-navy/[0.025] px-5 py-2.5 lg:px-6">
                    <div class="mb-2 flex flex-wrap gap-1.5">
                        { for record.view_type.iter().take(5).map(|tag| html! {
                            <span class="border border-brand-gold/35 bg-brand-gold/[0.08] px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.14em] text-brand-navy/80">{tag.clone()}</span>
                        }) }
                    </div>
                    <div class="flex items-stretch gap-2">
                        <a href={viewing} class="inline-flex min-h-11 flex-1 items-center justify-center bg-brand-navy px-4 py-2.5 text-center text-[10px] font-medium uppercase tracking-[0.17em] text-brand-ivory">
                            {"Book a Private Viewing"}
                        </a>
                        <button type="button" onclick={save} aria-pressed={saved.to_string()}
                            class="min-h-11 flex-none border border-brand-navy/25 px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-brand-navy">
                            { if saved { "Saved" } else { "Save Property" } }
                        </button>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn media_panel(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
    let media = photos(record);
    if media.is_empty() {
        return html! {
            <div class="absolute inset-0 bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]"></div>
        };
    }

    let total = media.len();
    let active_index = model.property_media.active_index % total;
    let active = media[active_index].clone();
    let supporting = media
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, photo)| (index, photo.clone()))
        .collect::<Vec<_>>();
    let visible = supporting.iter().take(4).cloned().collect::<Vec<_>>();
    let remaining = supporting.len().saturating_sub(visible.len());
    let first_hidden = supporting.get(visible.len()).map(|(index, _)| *index).unwrap_or(0);
    let has_navigation = total > 1;

    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_msg.emit(Msg::PropertyMediaPrevious);
        })
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_msg.emit(Msg::PropertyMediaNext);
        })
    };
    let open_current = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyLightboxOpened(active_index)))
    };
    let hero = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: MouseEvent| {
            event.stop_propagation();
            on_msg.emit(Msg::PropertyMediaSelected(0));
        })
    };

    html! {
        <div class="absolute inset-0 flex flex-col gap-1 bg-brand-navy/[0.08] p-1"
             role="group" aria-label="Property photo gallery">
            <div class={classes!(
                "relative","flex-none","overflow-hidden","bg-muted",
                if visible.is_empty() { "h-[282px] sm:h-[362px] lg:h-[424px]" }
                else { "h-[210px] sm:h-[280px] lg:h-[342px]" }
            )}>
                <img src={active.src.clone()} alt={active.alt.clone()} sizes="100vw"
                    class="absolute inset-0 h-full w-full object-cover" />

                if has_navigation {
                    <button type="button" onclick={open_current.clone()} aria-label="View all photos"
                        class="absolute inset-0 z-[5] h-full w-full"></button>

                    <button type="button" onclick={previous} aria-label="Previous photo"
                        class="absolute left-2 top-1/2 z-10 flex h-12 w-12 -translate-y-1/2 items-center justify-center rounded-full border border-brand-gold/30 bg-brand-navy/55 text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                        { icon_html("chevron-right", "h-5 w-5 rotate-180", "2").unwrap_or_default() }
                    </button>
                    <button type="button" onclick={next} aria-label="Next photo"
                        class="absolute right-2 top-1/2 z-10 flex h-12 w-12 -translate-y-1/2 items-center justify-center rounded-full border border-brand-gold/30 bg-brand-navy/55 text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                        { icon_html("chevron-right", "h-5 w-5", "2").unwrap_or_default() }
                    </button>

                    <button type="button" onclick={open_current}
                        class="absolute bottom-3 left-3 z-10 inline-flex min-h-11 items-center gap-2 border border-brand-gold/45 bg-brand-navy/80 px-3.5 py-1.5 text-[10px] font-medium uppercase tracking-[0.16em] text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/60">
                        {"View all photos"}
                        { icon_html("camera", "h-3.5 w-3.5", "2").unwrap_or_default() }
                    </button>
                }

                if active_index != 0 && record.hero_url.is_some() {
                    <button type="button" onclick={hero}
                        class="absolute right-3 top-3 z-10 border border-brand-gold/45 bg-brand-navy/90 px-3 py-1.5 text-[9px] font-medium uppercase tracking-[0.16em] text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/60">
                        {"Hero Image"}
                    </button>
                }

                <p class="absolute bottom-3 right-3 z-10 bg-brand-navy/70 px-2.5 py-1 text-[10px] font-medium tabular-nums tracking-[0.12em] text-brand-ivory backdrop-blur-sm"
                    aria-live="polite">
                    { format!("{} / {}", active_index + 1, total) }
                </p>
            </div>

            if !visible.is_empty() {
                <div class="grid h-[68px] flex-none grid-cols-4 gap-1 sm:h-[78px]"
                    role="group" aria-label="Supporting property images">
                    { for visible.into_iter().enumerate().map(|(visible_index, (media_index, photo))| {
                        let show_remaining = visible_index == 3.min(supporting.len().saturating_sub(1)) && remaining > 0;
                        let selected = active_index == media_index;
                        let on_msg = on_msg.clone();
                        let target = if show_remaining { first_hidden } else { media_index };
                        let onclick = Callback::from(move |_: MouseEvent| {
                            if show_remaining {
                                on_msg.emit(Msg::PropertyLightboxOpened(target));
                            } else {
                                on_msg.emit(Msg::PropertyMediaSelected(target));
                            }
                        });
                        html! {
                            <button type="button" {onclick}
                                aria-label={if show_remaining { "View all photos".to_string() } else { format!("View {}", photo.alt) }}
                                class={classes!(
                                    "relative","h-[68px]","overflow-hidden","bg-muted","transition-opacity","sm:h-[78px]",
                                    "focus-visible:outline-none","focus-visible:ring-1","focus-visible:ring-inset","focus-visible:ring-brand-gold/70",
                                    if selected { "opacity-100 ring-2 ring-inset ring-brand-gold/70" } else { "opacity-85 hover:opacity-100" }
                                )}>
                                <img src={photo.src} alt="" class="absolute inset-0 h-full w-full object-cover" />
                                if show_remaining {
                                    <div class="pointer-events-none absolute inset-0 flex items-center justify-center bg-brand-navy/80 text-sm font-medium uppercase tracking-[0.16em] text-brand-ivory backdrop-blur-[1px]">
                                        { format!("+{remaining}") }
                                    </div>
                                }
                            </button>
                        }
                    }) }
                </div>
            }
        </div>
    }
}

fn lightbox(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
    if !model.property_media.lightbox_open {
        return Html::default();
    }
    let media = photos(record);
    if media.is_empty() {
        return Html::default();
    }

    let total = media.len();
    let index = model.property_media.lightbox_index % total;
    let active = media[index].clone();
    let close = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyLightboxClosed))
    };
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyLightboxMoved(-1)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PropertyLightboxMoved(1)))
    };

    html! {
        <div role="dialog" aria-modal="true" aria-label="All photos"
            class="fixed inset-0 z-[70] flex flex-col bg-brand-navy/95 text-brand-ivory">
            <header class="flex items-center justify-between gap-4 px-4 py-3 sm:px-6">
                <p class="text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">
                    { format!("{} / {}", index + 1, total) }
                </p>
                <button type="button" onclick={close}
                    class="inline-flex min-h-12 min-w-12 items-center justify-center rounded-full border border-brand-gold/40 bg-brand-navy/60 text-2xl leading-none text-brand-ivory shadow-sm transition hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold"
                    aria-label="Close photo viewer">
                    {"×"}
                </button>
            </header>

            <div class="relative min-h-0 flex-1">
                <img src={active.src} alt={active.alt}
                    class="absolute inset-0 h-full w-full object-contain" />
                if total > 1 {
                    <button type="button" onclick={previous} aria-label="Previous photo"
                        class="absolute left-2 top-1/2 flex h-14 w-14 -translate-y-1/2 items-center justify-center rounded-full border border-brand-gold/40 bg-brand-navy/70 text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                        { icon_html("chevron-right", "h-7 w-7 rotate-180", "2").unwrap_or_default() }
                    </button>
                    <button type="button" onclick={next} aria-label="Next photo"
                        class="absolute right-2 top-1/2 flex h-14 w-14 -translate-y-1/2 items-center justify-center rounded-full border border-brand-gold/40 bg-brand-navy/70 text-brand-ivory shadow-sm backdrop-blur-sm transition-colors hover:bg-brand-navy focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                        { icon_html("chevron-right", "h-7 w-7", "2").unwrap_or_default() }
                    </button>
                }
            </div>

            if total > 1 {
                <div class="flex gap-2 overflow-x-auto px-4 py-4 sm:px-6">
                    { for media.into_iter().enumerate().map(|(photo_index, photo)| {
                        let selected = photo_index == index;
                        let on_msg = on_msg.clone();
                        let onclick = Callback::from(move |_: MouseEvent| {
                            on_msg.emit(Msg::PropertyLightboxOpened(photo_index));
                        });
                        html! {
                            <button type="button" {onclick}
                                aria-label={format!("View {}", photo.alt)}
                                class={classes!(
                                    "relative","h-16","w-24","flex-none","overflow-hidden","rounded-sm","bg-brand-navy","transition",
                                    "focus-visible:outline-none","focus-visible:ring-2","focus-visible:ring-inset","focus-visible:ring-brand-gold",
                                    if selected { "ring-2 ring-inset ring-brand-gold" } else { "opacity-70 hover:opacity-100" }
                                )}>
                                <img src={photo.src} alt="" class="absolute inset-0 h-full w-full object-cover" />
                            </button>
                        }
                    }) }
                </div>
            }
        </div>
    }
}

fn detail_sections(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
    let mut tabs = vec![(PropertyTab::Overview, "Overview"), (PropertyTab::Details, "Details")];
    if !record.videos.is_empty() { tabs.push((PropertyTab::Video, "Video")); }
    if !record.documents.is_empty() { tabs.push((PropertyTab::Documents, "Documents")); }
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
                            class={classes!(
                                "relative flex min-h-12 min-w-[116px] flex-none self-stretch items-center justify-center border-r border-brand-gold px-6 font-serif text-sm tracking-[0.04em] sm:min-w-[132px] sm:px-8 sm:text-[15px]",
                                if selected { "bg-brand-navy font-semibold text-brand-gold" } else { "bg-brand-navy font-medium text-brand-ivory/85 hover:text-brand-ivory" }
                            )}>
                            {label}
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

fn definition(label: &str, value: Option<String>) -> Html {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else { return Html::default() };
    html! {
        <div class="min-w-0 border-t border-brand-navy/40 bg-card/60 px-3 py-4 ring-1 ring-inset ring-brand-navy/25">
            <dt class="text-[10px] font-semibold uppercase tracking-[0.17em] text-brand-navy/70">{label.to_string()}</dt>
            <dd class="mt-1.5 break-words text-[15px] font-medium leading-snug text-brand-navy">{value}</dd>
        </div>
    }
}

fn overview(record: &PropertyRecord) -> Html {
    let editorial = record.description.as_deref().filter(|text| !text.trim().is_empty());
    let compact_amenities = record.amenities.iter().filter(|item| matches!(item.as_str(),
        "Pool" | "Whole-Home Generator" | "Solar Power" | "Furnished" | "Gated" | "Water Access" | "Beach Access"
    )).collect::<Vec<_>>();
    let amenity_notes = record.amenities.iter().filter(|item| !matches!(item.as_str(),
        "Pool" | "Whole-Home Generator" | "Solar Power" | "Furnished" | "Gated" | "Water Access" | "Beach Access"
    )).collect::<Vec<_>>();
    let views = &record.view_type;
    let lifestyle = record.lifestyle_tags.iter().filter(|tag| {
        !record.amenities.iter().any(|item| item.eq_ignore_ascii_case(tag))
            && !views.iter().any(|view| view.eq_ignore_ascii_case(tag)
                || format!("{view} View").eq_ignore_ascii_case(tag))
    }).collect::<Vec<_>>();
    let has_sidebar = record.lot_size.is_some() || record.neighborhood.is_some()
        || !views.is_empty() || !lifestyle.is_empty() || record.listing_agent_name.is_some()
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
                            <dl class="grid grid-cols-2 gap-4">
                                { definition("Lot Size", record.lot_size.clone()) }
                                { definition("Neighborhood", record.neighborhood.clone()) }
                            </dl>
                        </section>
                    }
                    if record.listing_agent_name.is_some() || record.listing_id.is_some() {
                        <section class="border-t border-brand-navy/40 bg-card/50 px-5 py-5 ring-1 ring-inset ring-brand-navy/20">
                            <p class="mb-3 text-xs font-medium uppercase tracking-[0.2em] text-brand-gold">{"Listing Information"}</p>
                            <dl class="grid grid-cols-2 gap-2">
                                { definition("Listing Agent", record.listing_agent_name.clone()) }
                                { definition("Office", record.listing_office.clone()) }
                                { definition("Phone", record.listing_agent_phone.clone()) }
                                { definition("Email", record.listing_agent_email.clone()) }
                                { definition("MLS / Listing ID", record.listing_id.clone()) }
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

fn details(record: &PropertyRecord) -> Html {
    let land = record.kind.as_deref().is_some_and(|kind| kind.eq_ignore_ascii_case("land"));
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
            { definition("Year Built", record.year_built.map(|value| value.to_string())) }
            { definition("Parking Spaces", record.parking_spaces.map(|value| format_number(value, ""))) }
            { definition("Neighborhood", record.neighborhood.clone()) }
            { definition("Water Access", record.water_access.then(|| "Yes".into())) }
            { definition("Beach Access", record.beach_access.then(|| "Yes".into())) }
        </dl>
    }
}

fn location(record: &PropertyRecord) -> Html {
    let context = [&record.neighborhood, &record.city, &record.state_or_province]
        .into_iter().filter_map(|item| item.as_deref()).collect::<Vec<_>>().join(", ");
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
                    <dl class="mt-6 space-y-3">
                        { definition("Neighborhood", record.neighborhood.clone()) }
                        { definition("Municipality", record.city.clone()) }
                        { definition("Region", record.state_or_province.clone()) }
                    </dl>
                </aside>
            }
        </div>
    }
}

fn videos(record: &PropertyRecord) -> Html {
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

fn documents(record: &PropertyRecord) -> Html {
    html! {
        <div class="mx-auto max-w-5xl">
            <p class="mb-5 text-xs font-medium uppercase tracking-[0.24em] text-brand-gold">{"Property Documents"}</p>
            <ul class="space-y-3">{ for record.documents.iter().filter_map(|document| {
                let id = document.id.as_deref()?;
                let route = format!("/api/media/documents/{id}");
                let label = document.title.as_deref().unwrap_or("Property Document").to_string();
                Some(html! {
                    <li class="flex flex-col gap-4 border border-brand-navy/30 bg-card/70 p-4 sm:flex-row sm:items-center sm:justify-between">
                        <div>
                            <h3 class="font-serif text-base font-semibold text-brand-navy">{label.clone()}</h3>
                            <p class="mt-1 text-[11px] uppercase tracking-[0.14em] text-brand-navy/65">{document.mime_type.clone().unwrap_or_default()}</p>
                        </div>
                        <div class="flex gap-2">
                            <a href={route.clone()} target="_blank" rel="noopener noreferrer" class="inline-flex min-h-11 items-center border border-brand-navy/35 px-4 text-xs font-semibold uppercase text-brand-navy">{"View"}</a>
                            <a href={format!("{route}?download=1")} class="inline-flex min-h-11 items-center bg-brand-navy px-4 text-xs font-semibold uppercase text-brand-ivory">{"Download"}</a>
                        </div>
                    </li>
                })
            }) }</ul>
        </div>
    }
}

fn similar_properties(record: &PropertyRecord) -> Html {
    if record.similar.is_empty() { return Html::default(); }
    html! {
        <section class="mt-24 md:mt-32">
            <div class="mb-12 flex items-end justify-between gap-6 border-b border-border pb-10">
                <div>
                    <p class="mb-4 text-xs uppercase tracking-[0.34em] text-brand-gold">{"Continue Exploring"}</p>
                    <h2 class="font-serif text-3xl font-light text-foreground md:text-5xl">{"Similar Residences"}</h2>
                </div>
                <a href="/buyers" class="text-xs uppercase tracking-[0.2em] text-foreground">{"View all properties"}</a>
            </div>
            <div class="grid gap-8 md:grid-cols-3">{ for record.similar.iter().map(|listing| {
                let href = format!("/properties/{}", listing.slug);
                html! {
                    <article>
                        <a href={href.clone()} class="relative block aspect-[16/10] overflow-hidden bg-muted">
                            if let Some(src) = &listing.image_path { <img src={src.clone()} alt={listing.image_alt.clone().unwrap_or_else(|| listing.name.clone())} class="absolute inset-0 h-full w-full object-cover" /> }
                        </a>
                        <div class="mt-5 border-t border-border pt-4">
                            <h3 class="font-serif text-xl text-foreground"><a href={href}>{listing.name.clone()}</a></h3>
                            if let Some(location) = &listing.location { <p class="mt-2 text-[10px] uppercase tracking-[0.24em] text-muted-foreground">{location.clone()}</p> }
                            if let Some(price) = &listing.price { <p class="mt-4 text-sm text-foreground">{price.clone()}</p> }
                        </div>
                    </article>
                }
            }) }</div>
        </section>
    }
}

fn recently_viewed(model: &Model) -> Html {
    if model.property_media.recent.is_empty() { return Html::default(); }
    html! {
        <nav aria-label="Recently viewed properties" class="mt-16 border-t border-border pt-8 md:mt-20">
            <p class="text-xs font-light uppercase tracking-[0.34em] text-brand-gold">{"Recently Viewed"}</p>
            <ul class="mt-6 flex flex-wrap gap-x-10 gap-y-4">
                { for model.property_media.recent.iter().map(|item| html! {
                    <li><a href={format!("/properties/{}", item.slug)} class="inline-flex min-h-11 items-center font-serif text-lg font-light text-foreground hover:text-brand-gold">{item.name.clone()}</a></li>
                }) }
            </ul>
        </nav>
    }
}

fn format_number(value: f64, label: &str) -> String {
    let number = if value.fract() == 0.0 {
        (value as i64).to_string()
    } else {
        value.to_string()
    };
    if label.is_empty() { number } else { format!("{number} {label}") }
}
