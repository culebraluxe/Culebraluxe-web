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
    /// Direct CDN copies from the server-resolved page payload. Absent (a static image, an older payload, or a
    /// photograph the resolver could not serve) falls back to the guarded `/api/media` route.
    card_url: Option<String>,
    web_url: Option<String>,
    thumb_url: Option<String>,
}

/// A derivative size the media route serves. The route maps `card` to the 1200px copy, `thumb` to the 400px copy
/// and `web` to the 3600px copy; a request with no `size` gets the `web` copy — and, for a legacy photograph that
/// has no derivatives at all, the ORIGINAL (up to 6.6MB), which is exactly what a gallery tile must not ask for.
/// Bump with the server `MEDIA_CACHE_VERSION` when an image target changes.
const MEDIA_CACHE_VERSION: &str = "20261022a";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MediaSize {
    Card,
    Thumb,
    Web,
}

impl MediaSize {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Thumb => "thumb",
            Self::Web => "web",
        }
    }
}

/// The URL of `src` at `size`, for a source served by this application's media route.
///
/// `/api/media/{id}` takes `?size=card|thumb|web`; without a `size` the backend serves the web copy (or the
/// original for a legacy photograph), so the gallery names the derivative it actually displays. The parameter is
/// appended, or an existing `size` is replaced, while every other query parameter and any fragment is preserved.
/// A non-media URL (`/images/...`, an absolute asset URL, a data URI) is returned unchanged: only the media route
/// understands the parameter, and rewriting somebody else's URL would be a bug, not an optimisation.
pub(super) fn media_url(src: &str, size: MediaSize) -> String {
    if !is_media_route(src) {
        return src.to_string();
    }
    let (before_fragment, fragment) = match src.split_once('#') {
        Some((before_fragment, fragment)) => (before_fragment, Some(fragment)),
        None => (src, None),
    };
    let (base, query) = match before_fragment.split_once('?') {
        Some((base, query)) => (base, query),
        None => (before_fragment, ""),
    };
    let mut params: Vec<String> = query
        .split('&')
        .filter(|pair| {
            !pair.is_empty() && pair.split_once('=').map_or(true, |(key, _)| key != "size")
        })
        .map(str::to_string)
        .collect();
    params.retain(|pair| pair.split_once('=').map_or(true, |(key, _)| key != "v"));
    params.push(format!("size={}", size.as_str()));
    params.push(format!("v={MEDIA_CACHE_VERSION}"));
    let mut out = format!("{base}?{}", params.join("&"));
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// Whether `src` is served by the `/api/media/{id}` route — relative (`/api/media/…`) or absolute
/// (`https://host/api/media/…`). The check is on the PATH, so a `?size=` or `#` suffix cannot fake it.
fn is_media_route(src: &str) -> bool {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    let path = match path.split_once("://") {
        Some((_, after_scheme)) => after_scheme
            .split_once('/')
            .map(|(_, path)| path)
            .unwrap_or(""),
        None => path,
    };
    path.starts_with("/api/media/") || path.starts_with("api/media/")
}

/// `sizes` for the full-screen lightbox image: it is laid out edge to edge.
pub(super) const LIGHTBOX_SIZES: &str = "100vw";

/// `sizes` for the carousel panel. On desktop the panel is a little over half the content width; on mobile it is
/// edge to edge. The browser uses this plus the viewport DPR to choose between the 1200px card and 3600px web copy,
/// so a retina desktop gets its sharp copy while a phone still downloads only the card.
pub(super) const PANEL_SIZES: &str = "(min-width: 1024px) 52vw, 100vw";

/// The URL a photograph slot should fetch, at the size it displays.
///
/// DIRECT FIRST. The public page resolves the final CDN URLs server-side and carries them on the media item, so a
/// photograph is fetched straight from the CDN with no `/api/media/{id}` redirect and no database lookup per image.
/// When a direct URL is absent — a static image, a document, an older payload, or a copy the resolver could not
/// serve — the slot falls back to the guarded route exactly as before.
fn photo_url(photo: &Photo, size: MediaSize) -> String {
    let direct = match size {
        MediaSize::Card => photo.card_url.as_deref(),
        MediaSize::Web => photo.web_url.as_deref(),
        MediaSize::Thumb => photo.thumb_url.as_deref(),
    };
    direct
        .map(str::to_owned)
        .unwrap_or_else(|| media_url(&photo.src, size))
}

/// The lightbox `srcset`: the 1200px card copy for narrow screens and the 3600px web copy for large ones. A phone
/// downloads the card; a wide or retina desktop still gets the full web derivative. Both entries use the direct CDN
/// copy when the page resolved one, so the browser never asks this server for either.
fn lightbox_srcset(photo: &Photo) -> String {
    format!(
        "{} 1200w, {} 3600w",
        photo_url(photo, MediaSize::Card),
        photo_url(photo, MediaSize::Web)
    )
}

/// The carousel panel's `srcset`: the same two sizes as the lightbox, but the panel's own `sizes` tells the browser
/// that it only occupies about half the viewport. This is what restores sharpness on a large/retina screen without
/// putting the 3600px copy on phones.
fn panel_srcset(photo: &Photo) -> String {
    format!(
        "{} 1200w, {} 3600w",
        photo_url(photo, MediaSize::Card),
        photo_url(photo, MediaSize::Web)
    )
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
        let hero_item = record.gallery.iter().find(|item| {
            item.src().as_deref() == Some(hero_src) || item.card_url.as_deref() == Some(hero_src)
        });
        result.push(Photo {
            src: hero_src.to_string(),
            alt: hero_item
                .and_then(MediaItem::text)
                .unwrap_or(record.title.as_str())
                .to_string(),
            caption: hero_item.and_then(|item| item.caption.clone()),
            card_url: record
                .hero_card_url
                .clone()
                .or_else(|| hero_item.and_then(|item| item.card_url.clone())),
            web_url: record
                .hero_web_url
                .clone()
                .or_else(|| hero_item.and_then(|item| item.web_url.clone())),
            thumb_url: record
                .hero_thumb_url
                .clone()
                .or_else(|| hero_item.and_then(|item| item.thumb_url.clone())),
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
            card_url: item.card_url.clone(),
            web_url: item.web_url.clone(),
            thumb_url: item.thumb_url.clone(),
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
                    <div class="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] font-medium uppercase tracking-[0.16em]">
                        if let Some(kind) = &record.kind { <span class="text-brand-navy/70">{kind.clone()}</span> }
                        if let Some(status) = &record.status { <span class="border-l border-brand-gold/40 pl-3 text-brand-gold">{status.clone()}</span> }
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
                    <p class="mb-4 text-xs font-light uppercase tracking-[0.34em] text-brand-gold">{"Continue Exploring"}</p>
                    <h2 class="text-balance font-serif text-3xl font-light leading-[1.08] text-foreground md:text-5xl">{"Similar Residences"}</h2>
                </div>
                <a href="/buyers" class="inline-flex shrink-0 items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors hover:text-brand-gold">{"View all properties"}</a>
            </div>
            <div class="grid gap-8 md:grid-cols-3">{ for record.similar.iter().map(|listing| {
                let href = format!("/properties/{}", listing.slug);
                html! {
                    <article>
                        <a href={href.clone()} class="relative block aspect-[16/10] overflow-hidden bg-muted">
                            if let Some(src) = &listing.image_path { <img src={media_url(src, MediaSize::Card)} alt={listing.image_alt.clone().unwrap_or_else(|| listing.name.clone())} class="absolute inset-0 h-full w-full object-cover" /> }
                        </a>
                        <div class="mt-5 border-t border-border pt-4">
                            <h3 class="font-serif text-xl font-light leading-tight text-foreground md:text-2xl"><a href={href}>{listing.name.clone()}</a></h3>
                            if let Some(location) = &listing.location { <p class="mt-2 text-[10px] font-light uppercase tracking-[0.24em] text-muted-foreground">{location.clone()}</p> }
                            if let Some(price) = &listing.price { <p class="mt-4 text-sm font-light text-foreground">{price.clone()}</p> }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A photograph with no direct copies: every slot must fall back to the guarded route.
    fn api_photo(src: &str) -> Photo {
        Photo {
            src: src.to_owned(),
            alt: String::new(),
            caption: None,
            card_url: None,
            web_url: None,
            thumb_url: None,
        }
    }

    #[test]
    fn media_url_appends_the_size_to_a_bare_media_route() {
        assert_eq!(
            media_url("/api/media/abc", MediaSize::Card),
            "/api/media/abc?size=card&v=20261022a"
        );
        assert_eq!(
            media_url("/api/media/abc", MediaSize::Thumb),
            "/api/media/abc?size=thumb&v=20261022a"
        );
        assert_eq!(
            media_url("/api/media/abc", MediaSize::Web),
            "/api/media/abc?size=web&v=20261022a"
        );
    }

    #[test]
    fn media_url_replaces_only_an_existing_size() {
        assert_eq!(
            media_url("/api/media/abc?foo=1", MediaSize::Card),
            "/api/media/abc?foo=1&size=card&v=20261022a"
        );
        assert_eq!(
            media_url("/api/media/abc?size=web&foo=1", MediaSize::Thumb),
            "/api/media/abc?foo=1&size=thumb&v=20261022a"
        );
        assert_eq!(
            media_url("/api/media/abc?size=web&size=card", MediaSize::Web),
            "/api/media/abc?size=web&v=20261022a"
        );
    }

    #[test]
    fn media_url_leaves_non_media_sources_alone() {
        for src in [
            "/images/hero-villa.png",
            "https://cdn.example.test/photos/a.jpg?size=web",
            "data:image/png;base64,AAAA",
            "/v1/media/abc",
            "",
        ] {
            assert_eq!(media_url(src, MediaSize::Card), src);
        }
    }

    #[test]
    fn media_url_handles_absolute_media_routes_and_fragments() {
        assert_eq!(
            media_url("https://example.test/api/media/abc", MediaSize::Card),
            "https://example.test/api/media/abc?size=card&v=20261022a"
        );
        assert_eq!(
            media_url("/api/media/abc#photo", MediaSize::Web),
            "/api/media/abc?size=web&v=20261022a#photo"
        );
        assert_eq!(
            media_url("/api/media/abc?size=web&v=20261022a#photo", MediaSize::Card),
            "/api/media/abc?size=card&v=20261022a#photo"
        );
        assert_eq!(
            media_url("/api/media/abc?", MediaSize::Card),
            "/api/media/abc?size=card&v=20261022a"
        );
    }

    #[test]
    fn panel_offers_card_and_web_at_the_panel_width() {
        let photo = api_photo("/api/media/abc");
        assert_eq!(
            panel_srcset(&photo),
            "/api/media/abc?size=card&v=20261022a 1200w, /api/media/abc?size=web&v=20261022a 3600w"
        );
        assert_eq!(PANEL_SIZES, "(min-width: 1024px) 52vw, 100vw");
    }

    #[test]
    fn lightbox_srcset_offers_card_and_web_with_their_widths() {
        assert_eq!(
            lightbox_srcset(&api_photo("/api/media/abc")),
            "/api/media/abc?size=card&v=20261022a 1200w, /api/media/abc?size=web&v=20261022a 3600w"
        );
        assert_eq!(LIGHTBOX_SIZES, "100vw");
    }

    /// THE POINT OF THE PAGE PAYLOAD: when the server resolved the CDN copies, every slot fetches them directly —
    /// no `/api/media` redirect and no per-image database lookup from the browser.
    #[test]
    fn a_direct_copy_is_used_without_the_api_route() {
        let photo = Photo {
            src: "/api/media/abc?size=card&v=20261022a".into(),
            alt: "Casa".into(),
            caption: None,
            card_url: Some("https://cdn.example.test/media-cdn/media/abc/card.jpg".into()),
            web_url: Some("https://cdn.example.test/media-cdn/media/abc/web.jpg".into()),
            thumb_url: Some("https://cdn.example.test/media-cdn/media/abc/thumb.jpg".into()),
        };
        assert_eq!(
            photo_url(&photo, MediaSize::Card),
            "https://cdn.example.test/media-cdn/media/abc/card.jpg"
        );
        assert_eq!(
            photo_url(&photo, MediaSize::Web),
            "https://cdn.example.test/media-cdn/media/abc/web.jpg"
        );
        assert_eq!(
            photo_url(&photo, MediaSize::Thumb),
            "https://cdn.example.test/media-cdn/media/abc/thumb.jpg"
        );
        assert_eq!(
            lightbox_srcset(&photo),
            "https://cdn.example.test/media-cdn/media/abc/card.jpg 1200w, \
             https://cdn.example.test/media-cdn/media/abc/web.jpg 3600w"
        );
    }

    /// A missing direct copy (old payload, static image, unresolved photograph) is the guarded route, exactly as
    /// before this change — the fast path is an optimization, never a requirement.
    #[test]
    fn a_missing_direct_copy_falls_back_to_the_guarded_route() {
        let photo = api_photo("/api/media/abc");
        assert_eq!(
            photo_url(&photo, MediaSize::Card),
            "/api/media/abc?size=card&v=20261022a"
        );
        assert_eq!(
            photo_url(&photo, MediaSize::Thumb),
            "/api/media/abc?size=thumb&v=20261022a"
        );
        // A static image (not served by the media route) is untouched at every size.
        let static_photo = api_photo("/images/hero-villa.png");
        assert_eq!(
            photo_url(&static_photo, MediaSize::Web),
            "/images/hero-villa.png"
        );
        // One resolved copy does not force the others: card direct, web still the guarded route.
        let partial = Photo {
            card_url: Some("https://cdn.example.test/media-cdn/media/abc/card.jpg".into()),
            ..api_photo("/api/media/abc")
        };
        assert_eq!(
            photo_url(&partial, MediaSize::Card),
            "https://cdn.example.test/media-cdn/media/abc/card.jpg"
        );
        assert_eq!(
            photo_url(&partial, MediaSize::Web),
            "/api/media/abc?size=web&v=20261022a"
        );
    }

    #[test]
    fn every_gallery_slot_asks_for_the_derivative_it_displays() {
        // The UI crate has no SSR renderer (`yew` is compiled for CSR), so the rendered markup cannot be turned into
        // a string here. The markup is asserted at its source instead: the panel defaults to the sharp web copy and
        // offers a card/web srcset with the panel's sizes hint, both thumbnail strips ask for the 400px thumb and are
        // lazy, and the lightbox keeps web quality through its own srcset.
        let media = include_str!("property_detail/media.rs");
        assert_eq!(
            media.matches("srcset={panel_srcset(&active)}").count(),
            1,
            "the media panel must offer a card/web srcset"
        );
        assert!(
            media.contains("sizes={PANEL_SIZES}"),
            "the media panel must tell the browser its layout width"
        );
        assert_eq!(
            media.matches("photo_url(&active, MediaSize::Web)").count(),
            2,
            "the panel and lightbox active photographs must default to the sharp web derivative"
        );
        assert_eq!(
            media.matches("photo_url(&photo, MediaSize::Thumb)").count(),
            2,
            "both thumbnail strips (panel and lightbox) must request the thumb derivative"
        );
        assert_eq!(
            media.matches("loading=\"lazy\"").count(),
            2,
            "both thumbnail strips must be lazy"
        );
        assert_eq!(
            media.matches("loading=\"eager\"").count(),
            1,
            "the lightbox's active photograph is what the viewer opened the dialog for"
        );
        assert!(
            media.contains("srcset={lightbox_srcset(&active)}")
                && media.contains("sizes={LIGHTBOX_SIZES}"),
            "the lightbox must offer the srcset and tell the browser the layout width"
        );
        assert!(
            !media.contains("sizes=\"100vw\""),
            "the dead `sizes` on the panel (no srcset beside it) is back"
        );
    }
}
