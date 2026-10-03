//! The property's photographs: the media panel and the lightbox.

#[allow(unused_imports)]
use super::*;

pub(super) fn media_panel(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
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
    let first_hidden = supporting
        .get(visible.len())
        .map(|(index, _)| *index)
        .unwrap_or(0);
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

pub(super) fn lightbox(record: &PropertyRecord, model: &Model, on_msg: &Callback<Msg>) -> Html {
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
