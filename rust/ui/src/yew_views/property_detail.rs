//! /properties/:slug — the public property record, on Yew.
//!
//! This is the port of the LAST TypeScript PropertyMediaPanel before the Rust cutover,
//! not the simplified string renderer that replaced it. Carousel selection and lightbox
//! state stay in Model/Msg/update; this component is a pure projection plus callbacks.

use yew::prelude::*;

use crate::icons::icon_html;
use crate::model::{MediaItem, Model, Msg, PropertyRecord};

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
                    { detail_sections(record) }
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
    let facts = [
        record.kind.clone(),
        record.beds.map(|value| format_number(value, "Beds")),
        record.baths.map(|value| format_number(value, "Baths")),
        record.area.clone(),
        record.year_built.map(|year| format!("Built {year}")),
        record.architecture.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

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
                </div>

                <div class="flex flex-1 items-center px-5 py-4 lg:px-6">
                    <div class="grid w-full grid-cols-2 gap-x-6 sm:grid-cols-3 lg:grid-cols-2">
                        { for facts.into_iter().map(|fact| html! {
                            <p class="border-b border-brand-navy/10 py-3 text-[11px] font-light uppercase tracking-[0.16em] text-brand-navy/65">
                                { fact }
                            </p>
                        }) }
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

fn detail_sections(record: &PropertyRecord) -> Html {
    let documents = record.documents.iter().filter_map(|item| {
        let src = item.src()?;
        let label = item.text().unwrap_or("Document").to_string();
        Some(html! {
            <li class="border-b border-border py-4">
                <a href={src} target="_blank" rel="noreferrer"
                    class="text-sm font-light text-foreground underline underline-offset-4">{ label }</a>
            </li>
        })
    }).collect::<Vec<_>>();
    let videos = record.videos.iter().filter_map(|item| {
        let src = item.src()?;
        let label = item.text().unwrap_or("Video").to_string();
        Some(html! {
            <li class="border-b border-border py-4">
                <a href={src} target="_blank" rel="noreferrer"
                    class="text-sm font-light text-foreground underline underline-offset-4">{ label }</a>
            </li>
        })
    }).collect::<Vec<_>>();

    html! {
        <>
            if let Some(description) = record.description.as_ref().filter(|value| !value.is_empty()) {
                <section class="mt-16 max-w-3xl">
                    <h2 class="font-serif text-2xl font-light text-foreground">{"About this property"}</h2>
                    <p class="mt-6 whitespace-pre-line text-sm font-light leading-relaxed text-muted-foreground">
                        { description.clone() }
                    </p>
                </section>
            }
            if !videos.is_empty() {
                <section class="mt-16">
                    <h2 class="font-serif text-2xl font-light text-foreground">{"Video"}</h2>
                    <ul class="mt-6 border-t border-border">{ for videos }</ul>
                </section>
            }
            if !documents.is_empty() {
                <section class="mt-16">
                    <h2 class="font-serif text-2xl font-light text-foreground">{"Documents"}</h2>
                    <ul class="mt-6 border-t border-border">{ for documents }</ul>
                </section>
            }
        </>
    }
}

fn format_number(value: f64, label: &str) -> String {
    if value.fract() == 0.0 {
        format!("{} {label}", value as i64)
    } else {
        format!("{value} {label}")
    }
}
