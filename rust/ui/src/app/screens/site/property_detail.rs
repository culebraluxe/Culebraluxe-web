//! /properties/:slug — the public property record, on Yew.
//!
//! Port of the TypeScript property detail page. Its carousel, tabs and saved state
//! stay in Model/Msg/update; this component is a projection plus callbacks.

use yew::prelude::*;

use super::visitor::{Model, Msg};

use crate::icons::icon_html;
use crate::model::{MediaItem, PropertyRecord, PropertyTab};
mod media;
mod sections;
#[allow(unused_imports)]
pub(super) use media::*;
#[allow(unused_imports)]
pub(super) use sections::*;

#[derive(Clone, PartialEq)]
struct Photo {
    src: String,
    alt: String,
    caption: Option<String>,
}

pub struct PropertyDetail;

impl PropertyDetail {
    pub(super) fn render(&self, model: &Model, on_msg: &Callback<Msg>) -> Html {
        let Some(record) = model.page.as_ref().and_then(|page| page.property.as_ref()) else {
            return html! {
                <section class="px-6 py-20 md:px-12">
                    <div class="mx-auto max-w-[1600px]">
                        if model.loading {
                            { crate::app::template::loading_toned(crate::app::template::Tone::Site, "the property") }
                        } else {
                            <p class="text-sm font-light text-muted-foreground">{"This property could not be loaded."}</p>
                        }
                    </div>
                </section>
            };
        };

        let on_keydown = {
            let on_msg = on_msg.clone();
            let lightbox_open = model.property_media.lightbox_open;
            Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
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
    let land = record
        .kind
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("land"));
    let facts = [
        if land {
            None
        } else {
            record
                .beds
                .map(|value| ("bed-double", "Beds", format_number(value, "")))
        },
        if land {
            None
        } else {
            record
                .baths
                .map(|value| ("bath", "Baths", bathrooms_display(record, value)))
        },
        if land {
            None
        } else {
            record
                .area
                .clone()
                .map(|value| ("maximize", "Interior", value))
        },
        if land {
            record.lot_size.clone().map(|value| ("trees", "Lot", value))
        } else {
            None
        },
        record
            .year_built
            .map(|value| ("calendar-days", "Built", value.to_string())),
        record
            .parking_spaces
            .map(|value| ("car", "Parking", format_number(value, ""))),
        record
            .stories
            .map(|value| ("layers-3", "Stories", format_number(value, "")))
            .or_else(|| {
                record
                    .water_access
                    .then(|| ("waves", "Water Access", "Yes".into()))
            }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let viewing = format!(
        "/contact?propertyId={}&requestType=private_viewing#contact",
        record.id
    );
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

                <div class="flex flex-1 items-center">
                    <div class="w-full px-5 py-3 lg:px-6">
                        <p class="mb-2 text-[10px] font-semibold uppercase tracking-[0.2em] text-brand-navy/75">{"Key Facts"}</p>
                        <dl class="grid grid-cols-2 gap-2 xl:grid-cols-3">
                        { for facts.into_iter().map(|(icon, label, value)| html! {
                            <div class="flex min-h-12 items-center gap-2 border border-brand-navy/35 bg-brand-navy/[0.04] px-2 py-1.5">
                                <span class="flex h-6 w-6 flex-none items-center justify-center border border-brand-gold/35 bg-brand-gold/[0.09] text-brand-gold">
                                    { icon_html(icon, "h-3.5 w-3.5", "2").unwrap_or_default() }
                                </span>
                                <div class="min-w-0">
                                    <dt class="text-[9px] font-semibold uppercase tracking-[0.12em] text-brand-navy/70">{label}</dt>
                                    <dd class="truncate text-sm font-medium leading-tight text-brand-navy">{value}</dd>
                                </div>
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
                        <a href={viewing} class="inline-flex min-h-11 flex-1 items-center justify-center bg-brand-navy px-4 py-2.5 text-center text-[10px] font-medium uppercase tracking-[0.17em] text-brand-ivory shadow-sm transition-colors duration-500 hover:bg-brand-navy/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                            {"Book a Private Viewing"}
                        </a>
                        <button type="button" onclick={save} aria-pressed={saved.to_string()}
                            class="inline-flex min-h-11 flex-none items-center justify-center gap-2 border border-brand-navy/25 px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-brand-navy transition-colors hover:border-brand-navy/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold">
                            { icon_html("heart", if saved { "h-4 w-4 fill-brand-gold text-brand-gold" } else { "h-4 w-4" }, "2").unwrap_or_default() }
                            { if saved { "Saved" } else { "Save Property" } }
                        </button>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn similar_properties(record: &PropertyRecord) -> Html {
    if record.similar.is_empty() {
        return Html::default();
    }
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
    if model.property_media.recent.is_empty() {
        return Html::default();
    }
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
    if label.is_empty() {
        number
    } else {
        format!("{number} {label}")
    }
}

fn bathrooms_display(record: &PropertyRecord, total: f64) -> String {
    let mut parts = Vec::new();
    if let Some(full) = record.bathrooms_full {
        parts.push(format!("{} Full", format_number(full, "")));
    }
    if let Some(half) = record.bathrooms_half {
        parts.push(format!("{} Half", format_number(half, "")));
    }
    if parts.is_empty() {
        format_number(total, "")
    } else {
        format!("{} ({})", format_number(total, ""), parts.join(", "))
    }
}
