//! `/buyers` — the showroom, on Yew.
//!
//! THE CONTROLS ARE MVI, NOT DOM LISTENERS. Every one is an `oninput`/`onchange`/`onclick` callback that emits a `Msg`;
//! the reducer is the only thing that decides what that means, and the rendered value IS the model's value
//! (`value={…}`, `selected`, `aria-pressed`). Nothing reads a field back out of the DOM, nothing scrapes an attribute,
//! and there is no repaint step: Yew diffs the view, so a keystroke cannot rebuild the chrome and the caret stays put.
//!
//! THE SEARCH CONTRACT IS THE SAME CODE. `view::buyers_visible` — the ported `lib/search-contract.ts` — decides which
//! listings match and in what order, so this view and the string renderer cannot disagree about what a filter means.

use yew::prelude::*;

use crate::model::{Listing, Model, Msg};
use crate::view::{buyers_visible, BUYER_SERVICES, BUYER_STEPS};

#[derive(Properties, PartialEq)]
pub struct BuyersProps {
    pub model: Model,
    pub on_msg: Callback<Msg>,
}

pub struct Buyers;

impl Component for Buyers {
    type Message = ();
    type Properties = BuyersProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let model = &props.model;
        let on_msg = &props.on_msg;
        let listings: &[Listing] = model
            .page
            .as_ref()
            .map(|page| page.listings.as_slice())
            .unwrap_or(&[]);
        html! {
            <>
                { page_hero(
                    "For Buyers",
                    "Find your place on Culebra.",
                    Some("Exceptional homes, villas, and land — presented with the perspective of people who know the island intimately."),
                    "/images/hero-villa.png",
                    "A modern luxury villa overlooking the Culebra coastline",
                ) }
                { self.showroom(model, listings, on_msg) }
                { self.steps() }
                { self.private_opportunities() }
                { self.beyond_the_search() }
            </>
        }
    }
}

/// `components/page-hero.tsx` — the header every interior page opens with.
///
/// One component, five pages, so it is written once here too: the height, the padding that clears the fixed header, the
/// scrim that makes ivory text legible over a photograph, and the rule that an absent intro renders nothing.
pub fn page_hero(
    eyebrow: &str,
    title: &str,
    intro: Option<&str>,
    image: &str,
    image_alt: &str,
) -> Html {
    let intro = intro.map(|intro| {
        html! {
            <p class="mt-8 max-w-2xl text-pretty text-base font-light leading-relaxed text-background/80 md:text-lg">
                { intro }
            </p>
        }
    });
    html! {
        <section class="relative flex min-h-[68svh] items-end overflow-hidden">
            <img src={image.to_string()} alt={image_alt.to_string()} sizes="100vw"
                class="absolute inset-0 h-full w-full object-cover" />
            <div class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/25 to-black/40"></div>
            <div class="relative w-full px-6 pb-16 pt-40 md:px-12 md:pb-24">
                <div class="mx-auto max-w-[1600px]">
                    // The live hero faded up on load (`Reveal` fires at once for what is already in view), heading
                    // first and the intro a beat after.
                    <div class="animate-[fadeUp_1.2s_cubic-bezier(0.22,1,0.36,1)_both]">
                        <p class="mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70">{ eyebrow }</p>
                        <h1 class="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                            { title }
                        </h1>
                    </div>
                    <div class="animate-[fadeUp_1.2s_cubic-bezier(0.22,1,0.36,1)_both] [animation-delay:120ms]">{ intro }</div>
                </div>
            </div>
        </section>
    }
}

/// The option lists, exactly as the live bar offers them.
const PRICES: [(&str, &str); 6] = [
    ("", "Any Price"),
    ("1000000", "Up to $1M"),
    ("2000000", "Up to $2M"),
    ("3000000", "Up to $3M"),
    ("5000000", "Up to $5M"),
    ("10000000", "Up to $10M"),
];
const BEDS: [(&str, &str); 5] = [
    ("", "Any Beds"),
    ("2", "2+ Beds"),
    ("3", "3+ Beds"),
    ("4", "4+ Beds"),
    ("5", "5+ Beds"),
];
const SORTS: [(&str, &str); 4] = [
    ("featured", "Featured"),
    ("price-high", "Price High"),
    ("price-low", "Price Low"),
    ("name", "Name"),
];

impl Buyers {
    /// The showroom: the featured strip, the category tabs, the filter bar and the inventory.
    fn showroom(&self, model: &Model, listings: &[Listing], on_msg: &Callback<Msg>) -> Html {
        if listings.is_empty() {
            return html! {
                <section class="px-6 py-24 md:px-12 md:py-32">
                    <div class="mx-auto max-w-[1600px]">
                        <p class="font-serif text-3xl font-light text-foreground">{"New opportunities are being prepared."}</p>
                        <p class="mt-4 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                            {"Contact CulebraLuxe for private and upcoming properties on the island."}
                        </p>
                    </div>
                </section>
            };
        }
        // WHAT THE CONTROLS SAY, not what the payload holds: the same call the string renderer makes, so the count and
        // the grid are one answer.
        let visible = buyers_visible(listings, model);
        let count = match visible.len() {
            1 => "1 property".to_string(),
            total => format!("{total} properties"),
        };
        let strip = listings.iter().map(slide).collect::<Vec<_>>();
        let cards = visible
            .iter()
            .map(|listing| {
                let compared = model.compare.iter().any(|entry| entry.id == listing.id);
                card(listing, model.saved_listings.contains(&listing.id), Some(compared), on_msg)
            })
            .collect::<Vec<_>>();
        html! {
            <>
                <section class="px-6 pb-12 pt-12 md:px-12 md:pb-16 md:pt-16">
                    <div class="mx-auto max-w-[1600px]">
                        <div class="mb-6 flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
                            <div>
                                <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Selected Properties"}</p>
                                <h2 class="max-w-3xl text-balance font-serif text-4xl font-light leading-[1.03] text-foreground md:text-5xl">
                                    {"Exceptional places."}<br />{"Singular settings."}
                                </h2>
                            </div>
                            <p class="max-w-md text-sm font-light leading-relaxed text-muted-foreground md:pb-1">
                                {"A considered selection of residences and land across Culebra."}
                            </p>
                        </div>
                        <div class="flex h-[320px] gap-2 overflow-x-auto sm:h-[360px] md:h-[400px] lg:h-[420px]">
                            { for strip }
                        </div>
                    </div>
                </section>
                <section id="inventory" class="border-t border-border bg-background px-6 py-16 md:px-12 md:py-20">
                    <div class="mx-auto max-w-[1600px]">
                        <div class="mb-10">
                            <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Explore Culebra"}</p>
                            <div class="flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
                                <h2 class="font-serif text-4xl font-light leading-none text-foreground md:text-5xl">
                                    {"Available properties."}
                                </h2>
                                <div class="flex items-center gap-5">
                                    <p class="text-xs font-light uppercase tracking-[0.18em] text-muted-foreground">{ count }</p>
                                    <a href="/favorites" class="text-xs font-light uppercase tracking-[0.18em] text-accent transition-colors hover:text-foreground">
                                        {"Saved"}
                                    </a>
                                </div>
                            </div>
                        </div>
                        <div class="mb-7 flex flex-wrap gap-x-8 gap-y-3 border-b border-border">
                            { self.tabs(model, on_msg) }
                        </div>
                        { self.filters(model, listings, on_msg) }
                        { saved_searches_panel(model, listings, on_msg) }
                        if cards.is_empty() {
                            <div class="border-t border-border py-16">
                                <p class="font-serif text-2xl font-light text-foreground">{"No properties match these filters."}</p>
                                <p class="mt-3 text-sm font-light text-muted-foreground">{"Widen the search, or save it to hear when one arrives."}</p>
                            </div>
                        } else {
                            <div class="grid gap-x-7 gap-y-14 md:grid-cols-2 xl:grid-cols-3">
                                { for cards }
                            </div>
                        }
                        { compare_table(model, listings, on_msg) }
                        if !model.saved_searches.is_empty() {
                            // Saved searches, as a lead: until alerts are sent automatically, the team is the alert.
                            { crate::yew_views::contact::quick_enquiry(model, on_msg, crate::yew_views::contact::QuickEnquiry {
                                eyebrow: "Alerts",
                                title: "Tell me when new properties match.",
                                body: "Leave your details and the CulebraLuxe team will let you know as soon as a new property matches one of your saved searches, often before it is widely marketed.",
                                button: "Alert me",
                                sent: "We have your saved searches. A member of the CulebraLuxe team will be in touch when a new property matches.",
                                message: searches_message(model),
                            }) }
                        }
                    </div>
                </section>
            </>
        }
    }
}

impl Buyers {
    /// The three category tabs: real buttons, each emitting the intent the reducer already understands.
    fn tabs(&self, model: &Model, on_msg: &Callback<Msg>) -> Html {
        let current = model
            .controls
            .tab
            .clone()
            .unwrap_or_else(|| "all".to_string());
        [("all", "All"), ("homes", "Homes & Villas"), ("land", "Land")]
            .into_iter()
            .map(|(key, label)| {
                let is_current = key == current;
                let onclick = {
                    let on_msg = on_msg.clone();
                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TabSelected(key.to_string())))
                };
                html! {
                    <button type="button" {onclick} aria-pressed={is_current.to_string()}
                        class={classes!(
                            "relative", "-mb-px", "cursor-pointer", "pb-4", "text-xs", "font-light",
                            "uppercase", "tracking-[0.2em]", "transition-colors",
                            if is_current { "text-foreground" } else { "text-muted-foreground hover:text-foreground" }
                        )}>
                        { label }
                        <span class={classes!(
                            "absolute", "inset-x-0", "bottom-0", "h-px",
                            if is_current { "bg-foreground" } else { "bg-transparent" }
                        )}></span>
                    </button>
                }
            })
            .collect()
    }
}

impl Buyers {
    /// The filter bar. Every control renders the MODEL's value and emits exactly one message; none is read back out of
    /// the DOM, which is what makes filtering a pure function of the model.
    fn filters(&self, model: &Model, listings: &[Listing], on_msg: &Callback<Msg>) -> Html {
        let controls = &model.controls;
        let named = |key: &str| controls.named.get(key).cloned().unwrap_or_default();
        // The contract's own rule: land has no bedrooms, so that control is disabled on the Land tab — there it would
        // exclude every listing rather than narrow them.
        let beds_disabled = controls.tab.as_deref() == Some("land");
        let oninput = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: InputEvent| {
                let value = event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value();
                on_msg.emit(Msg::QueryChanged(value));
            })
        };
        let chosen_sort = if named("sort").is_empty() {
            "featured".to_string()
        } else {
            named("sort")
        };
        html! {
            <div class="sticky top-0 z-30 mb-12 border-y border-border bg-background/95 py-4 backdrop-blur-md">
                <div class="grid gap-3 md:grid-cols-12">
                    <label class="relative md:col-span-4">
                        <span class="sr-only">{"Search properties"}</span>
                        <svg class="absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
                            viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                            <circle cx="11" cy="11" r="7" />
                            <path d="m21 21-4.3-4.3" />
                        </svg>
                        <input {oninput} value={controls.query.clone()} placeholder="Property, neighborhood, view..."
                            class="h-12 w-full border border-border bg-transparent pl-11 pr-4 text-sm font-light text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-foreground" />
                    </label>
                    { select("price", "Any Price", &PRICES, &named("price"), false, on_msg) }
                    { select("beds", "Any Beds", &BEDS, &named("beds"), beds_disabled, on_msg) }
                    { view_select(&crate::search::view_options(listings), &named("view"), on_msg) }
                    { select("sort", "Sort", &SORTS, &chosen_sort, false, on_msg) }
                </div>
            </div>
        }
    }
}

/// One dropdown: the model's value is the selected option, and a change emits one `FilterSelected`.
fn select(
    name: &'static str,
    label: &'static str,
    options: &[(&str, &str)],
    chosen: &str,
    disabled: bool,
    on_msg: &Callback<Msg>,
) -> Html {
    let onchange = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::FilterSelected {
                key: name.to_string(),
                value,
            });
        })
    };
    html! {
        <select aria-label={label} disabled={disabled} {onchange}
            class={classes!(
                "h-12", "border", "border-border", "bg-background", "px-4", "text-xs", "font-light",
                "uppercase", "tracking-[0.12em]", "text-foreground", "outline-none", "cursor-pointer",
                "md:col-span-2", if disabled { "opacity-40" } else { "" }
            )}>
            { for options.iter().map(|(value, text)| html! {
                <option value={value.to_string()} selected={*value == chosen}>{ text }</option>
            }) }
        </select>
    }
}

/// The view filter, fed by the views the inventory actually has: an option for each, and nothing it could not match.
fn view_select(options: &[String], chosen: &str, on_msg: &Callback<Msg>) -> Html {
    let onchange = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            on_msg.emit(Msg::FilterSelected { key: "view".to_string(), value });
        })
    };
    html! {
        <select aria-label="Any View" disabled={options.is_empty()} {onchange}
            class={classes!(
                "h-12", "border", "border-border", "bg-background", "px-4", "text-xs", "font-light", "uppercase",
                "tracking-[0.12em]", "text-foreground", "outline-none", "cursor-pointer", "md:col-span-2",
                options.is_empty().then_some("opacity-40")
            )}>
            <option value="" selected={chosen.is_empty()}>{"Any View"}</option>
            { for options.iter().map(|view| html! {
                <option value={view.clone()} selected={view.eq_ignore_ascii_case(chosen)}>{ format!("{view} View") }</option>
            }) }
        </select>
    }
}

/// The scales on a card (`CompareProperty`): adds the listing to the compare set, or takes it out.
fn compare_button(listing: &Listing, compared: bool, on_msg: &Callback<Msg>) -> Html {
    let onclick = {
        let on_msg = on_msg.clone();
        let id = listing.id.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();
            on_msg.emit(Msg::CompareToggled(id.clone()));
        })
    };
    let label = if compared {
        format!("Remove {} from compare", listing.name)
    } else {
        format!("Add {} to compare", listing.name)
    };
    html! {
        <button type="button" {onclick} aria-pressed={compared.to_string()} aria-label={label.clone()} title={label}
            class="absolute right-16 top-4 z-30 flex h-10 w-10 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
            <svg class={classes!("h-4", "w-4", "transition-colors", "duration-300", compared.then_some("text-accent"))}
                viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m16 16 3-8 3 8c-.87.65-1.92 1-3 1s-2.13-.35-3-1Z" />
                <path d="m2 16 3-8 3 8c-.87.65-1.92 1-3 1s-2.13-.35-3-1Z" />
                <path d="M7 21h10" /><path d="M12 3v18" /><path d="M3 7h2c2 0 5-1 7-2 2 1 5 2 7 2h2" />
            </svg>
        </button>
    }
}

/// The lead's message: each saved search by its name, so the team knows what to watch for.
fn searches_message(model: &Model) -> String {
    let lines = model
        .saved_searches
        .iter()
        .map(|search| format!("- {}", search.name))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Please alert me when new properties match my saved searches:\n{lines}")
}

/// A fresh id for a saved search, in the TypeScript store's style, and the time now, for the reducer.
fn search_stamp() -> (String, String) {
    let now = js_sys::Date::new_0();
    let id = format!("ss-{}-{}", now.get_time() as u64, (js_sys::Math::random() * 1e9) as u64);
    (id, now.to_iso_string().into())
}

/// Saved searches (`SavedSearchesPanel`): save what the bar says now, and each saved search as a chip with its match
/// count and an honest "+N new" since it was last viewed. Choosing one applies its filters.
fn saved_searches_panel(model: &Model, listings: &[Listing], on_msg: &Callback<Msg>) -> Html {
    let current = crate::search::SearchFilters::from_controls(&model.controls);
    let current_saved = model.saved_searches.iter().any(|search| search.filters.key() == current.key());
    let views = model
        .saved_searches
        .iter()
        .map(|search| {
            let matches = crate::search::match_ids(listings, &search.filters);
            let new = crate::search::new_match_ids(search, &matches).len();
            (search, matches.len(), new)
        })
        .collect::<Vec<_>>();
    let total_new: usize = views.iter().map(|(_, _, new)| new).sum();
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            let (new_id, now) = search_stamp();
            on_msg.emit(Msg::SearchSaved { new_id, now });
        })
    };
    html! {
        <div class="mb-12 flex flex-col gap-4 border border-border bg-muted/30 px-5 py-4 md:flex-row md:items-center md:justify-between">
            <button type="button" onclick={save} aria-pressed={current_saved.to_string()}
                class="flex h-12 min-w-0 items-center gap-3 border border-border bg-background px-5 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors duration-300 hover:border-foreground">
                <svg class={classes!("h-4", "w-4", "flex-none", if current_saved { "text-accent" } else { "text-muted-foreground" })}
                    viewBox="0 0 24 24" fill={if current_saved { "currentColor" } else { "none" }} stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                    <path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" />
                </svg>
                <span class="truncate">{ if current_saved { "Search saved" } else { "Save this search" } }</span>
            </button>
            if views.is_empty() {
                <p class="text-xs font-light leading-relaxed text-muted-foreground">
                    {"Save a search to get an alert when new matching properties appear."}
                </p>
            } else {
                <div class="flex flex-wrap items-center gap-2" aria-label="Saved searches">
                    if total_new > 0 {
                        <span class="inline-flex h-6 items-center gap-1.5 bg-accent/10 px-2.5 text-[10px] font-light uppercase tracking-[0.16em] text-accent">
                            { format!("{total_new} new across {}", views.len()) }
                        </span>
                    }
                    { for views.into_iter().map(|(search, count, new)| {
                        let apply = {
                            let on_msg = on_msg.clone();
                            let id = search.id.clone();
                            Callback::from(move |_: MouseEvent| {
                                let (_, now) = search_stamp();
                                on_msg.emit(Msg::SavedSearchApplied { id: id.clone(), now });
                            })
                        };
                        let remove = {
                            let on_msg = on_msg.clone();
                            let id = search.id.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SavedSearchRemoved(id.clone())))
                        };
                        html! {
                            <span class="inline-flex items-stretch border border-border bg-background">
                                <button type="button" onclick={apply} title="Apply this saved search"
                                    class="group flex h-12 items-center gap-2 pl-4 pr-2 text-left">
                                    <span class="max-w-56 truncate text-xs font-light tracking-wide text-foreground/90 group-hover:text-foreground">
                                        { search.name.clone() }
                                    </span>
                                    <span class="text-[10px] font-light uppercase tracking-[0.14em] text-muted-foreground" aria-hidden="true">{ count }</span>
                                    if new > 0 {
                                        <span class="text-[10px] font-light uppercase tracking-[0.14em] text-accent">{ format!("+{new} new") }</span>
                                    }
                                </button>
                                <button type="button" onclick={remove} aria-label={format!("Remove saved search {}", search.name)}
                                    class="flex h-12 w-10 flex-none items-center justify-center text-muted-foreground transition-colors hover:text-foreground">
                                    { close_icon() }
                                </button>
                            </span>
                        }
                    }) }
                </div>
            }
        </div>
    }
}

fn close_icon() -> Html {
    html! {
        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
            <path d="M18 6 6 18" /><path d="m6 6 12 12" />
        </svg>
    }
}

/// One row of the compare table: its label and how a listing answers it.
type CompareRow = (&'static str, fn(&Listing) -> String);

fn dash() -> String {
    "\u{2014}".to_string()
}

fn count_or_dash(value: Option<f64>) -> String {
    value.map(|value| format!("{value}")).unwrap_or_else(dash)
}

const COMPARE_ROWS: [CompareRow; 9] = [
    ("Price", |listing| crate::format::listing_price_label(listing)),
    ("Location", |listing| listing.location.clone().unwrap_or_else(dash)),
    ("Type", |listing| listing.kind.clone().unwrap_or_else(dash)),
    ("Beds", |listing| if crate::format::listing_is_land(listing) { dash() } else { count_or_dash(listing.beds) }),
    ("Baths", |listing| if crate::format::listing_is_land(listing) { dash() } else { count_or_dash(listing.baths) }),
    ("Interior", |listing| {
        if crate::format::listing_is_land(listing) { dash() } else { listing.interior_area.clone().unwrap_or_else(dash) }
    }),
    ("Lot", |listing| listing.area.clone().unwrap_or_else(dash)),
    ("Views", |listing| if listing.views.is_empty() { dash() } else { listing.views.join(", ") }),
    ("Beach access", |listing| (if listing.beach_access { "Yes" } else { "No" }).to_string()),
];

/// "Side by side" (`CompareBar`): once two are chosen, a table of the facts that decide between them. Only published
/// listings appear, in the order they were chosen.
fn compare_table(model: &Model, listings: &[Listing], on_msg: &Callback<Msg>) -> Html {
    let chosen = model
        .compare
        .iter()
        .filter_map(|entry| listings.iter().find(|listing| listing.slug == entry.slug))
        .collect::<Vec<&Listing>>();
    if chosen.len() < 2 {
        return Html::default();
    }
    html! {
        <section class="mt-20 border-t border-border pt-12" aria-label="Compare properties">
            <div class="mb-8 flex flex-wrap items-center justify-between gap-4">
                <div>
                    <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Compare"}</p>
                    <h2 class="font-serif text-3xl font-light leading-tight text-foreground">{"Side by side"}</h2>
                </div>
                <p class="text-xs font-light text-muted-foreground">
                    { format!("{} of {} selected", chosen.len(), crate::search::COMPARE_MAX) }
                </p>
            </div>
            <div class="overflow-x-auto">
                <table class="w-full min-w-[640px] border-collapse">
                    <thead>
                        <tr>
                            <th class="w-40 border-b border-border pb-4 pr-6 text-left align-bottom">
                                <span class="text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">{"Property"}</span>
                            </th>
                            { for chosen.iter().map(|listing| {
                                let remove = {
                                    let on_msg = on_msg.clone();
                                    let id = listing.id.clone();
                                    Callback::from(move |_: MouseEvent| on_msg.emit(Msg::CompareToggled(id.clone())))
                                };
                                html! {
                                    <th class="border-b border-border pb-4 pr-6 text-left align-bottom">
                                        <div class="relative mb-4 aspect-[4/3] w-44 overflow-hidden bg-muted">
                                            { listing_image(listing, "absolute inset-0 h-full w-full object-cover", "176px") }
                                            <button type="button" onclick={remove} aria-label={format!("Remove {} from compare", listing.name)}
                                                class="absolute right-2 top-2 flex h-10 w-10 items-center justify-center rounded-full bg-background/80 text-foreground backdrop-blur-sm transition-colors duration-300 hover:bg-background">
                                                { close_icon() }
                                            </button>
                                        </div>
                                        <a href={format!("/properties/{}", listing.slug)}
                                            class="font-serif text-xl font-light leading-tight text-foreground transition-colors hover:text-accent">
                                            { listing.name.clone() }
                                        </a>
                                    </th>
                                }
                            }) }
                        </tr>
                    </thead>
                    <tbody>
                        { for COMPARE_ROWS.iter().map(|(label, value)| html! {
                            <tr>
                                <td class="border-b border-border py-4 pr-6 text-[10px] font-light uppercase tracking-[0.16em] text-muted-foreground">{ *label }</td>
                                { for chosen.iter().map(|listing| html! {
                                    <td class="border-b border-border py-4 pr-6 text-sm font-light text-foreground">{ value(listing) }</td>
                                }) }
                            </tr>
                        }) }
                    </tbody>
                </table>
            </div>
        </section>
    }
}

/// A listing's photograph, or the soft gradient the design uses when a property has no hero image. Shared with the
/// homepage cards, so a listing without a photograph looks the same everywhere rather than a gradient here and a
/// placeholder graphic there. Lazy, because the cards sit below the fold and the strip scrolls sideways; the browser
/// still fetches whatever is in view straight away.
pub(crate) fn listing_image(listing: &Listing, class: &str, sizes: &str) -> Html {
    match listing.image_path.as_deref().filter(|src| !src.trim().is_empty()) {
        Some(src) => html! {
            <img src={src.to_string()} alt={listing.image_alt.clone().unwrap_or_else(|| listing.name.clone())}
                sizes={sizes.to_string()} loading="lazy" decoding="async" class={class.to_string()} />
        },
        None => html! {
            <div class="h-full w-full bg-gradient-to-br from-[#d9dde0] via-[#eef0f1] to-[#c4cbd0]"></div>
        },
    }
}

/// The round heart on a listing card, `SaveProperty variant="icon"` from the TypeScript card. It sits above the card's
/// stretched link, so pressing it saves rather than navigates. Nothing without an id can be saved, so nothing is drawn.
pub(crate) fn save_heart(listing: &Listing, saved: bool, on_msg: &Callback<Msg>, class: &'static str) -> Html {
    if listing.id.trim().is_empty() {
        return Html::default();
    }
    let onclick = {
        let on_msg = on_msg.clone();
        let id = listing.id.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();
            on_msg.emit(Msg::ListingFavoriteToggled(id.clone()));
        })
    };
    let label = if saved { format!("Saved: {}", listing.name) } else { format!("Save {}", listing.name) };
    html! {
        <button type="button" {onclick} aria-pressed={saved.to_string()} aria-label={label.clone()} title={label}
            class={classes!(
                "z-30", "flex", "h-10", "w-10", "items-center", "justify-center", "rounded-full", "bg-background/80",
                "text-foreground", "backdrop-blur-sm", "transition-colors", "duration-300", "hover:bg-background",
                "focus-visible:outline-none", "focus-visible:ring-2", "focus-visible:ring-accent", class
            )}>
            { crate::icons::icon_html(
                "heart",
                if saved { "h-4 w-4 fill-accent text-accent transition-all duration-300" } else { "h-4 w-4 transition-all duration-300" },
                "2",
            ).unwrap_or_default() }
        </button>
    }
}

/// One slide of the featured strip.
fn slide(listing: &Listing) -> Html {
    html! {
        <article class="group relative h-full w-[86vw] flex-none overflow-hidden bg-muted sm:w-[420px]">
            { listing_image(listing, "absolute inset-0 h-full w-full object-cover", "(min-width: 1024px) 80vw, 100vw") }
            <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 via-black/25 to-transparent p-6">
                <p class="text-[10px] font-light uppercase tracking-[0.2em] text-background/75">
                    { listing.location.clone().unwrap_or_default() }
                </p>
                <div class="mt-2 flex items-end justify-between gap-4">
                    <h3 class="font-serif text-2xl font-light text-background">{ listing.name.clone() }</h3>
                    <p class="whitespace-nowrap text-sm font-light text-background/85">
                        { crate::format::listing_price_label(listing) }
                    </p>
                </div>
            </div>
        </article>
    }
}

/// One inventory card: the photograph with its badges, then the place, the price and its facts.
/// One inventory card. `compared` is `Some` where the page offers Compare (the Buyers grid) and `None` where it does
/// not (the Saved page).
pub(crate) fn card(listing: &Listing, saved: bool, compared: Option<bool>, on_msg: &Callback<Msg>) -> Html {
    let facts = crate::format::listing_facts(listing, crate::format::FactsStyle::Full);
    html! {
        <article class="group relative">
            <div class="relative aspect-[4/3] overflow-hidden bg-muted">
                { listing_image(listing, "absolute inset-0 h-full w-full object-cover", "(min-width: 1024px) 80vw, 100vw") }
                if listing.featured {
                    <span class="absolute left-4 top-4 z-20 bg-background/90 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.18em] text-foreground backdrop-blur-sm">
                        {"Featured"}
                    </span>
                }
                if crate::view::listing_is_land(listing) {
                    <span class="absolute bottom-4 left-4 z-20 bg-foreground/80 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.18em] text-background backdrop-blur-sm">
                        {"Land"}
                    </span>
                }
                { save_heart(listing, saved, on_msg, "absolute right-4 top-4") }
                if let Some(compared) = compared {
                    { compare_button(listing, compared, on_msg) }
                }
            </div>
            <a href={format!("/properties/{}", listing.slug)} aria-label={format!("View {}", listing.name)}
                class="absolute inset-0 z-10"></a>
            <div class="pointer-events-none relative z-20 pt-5">
                if let Some(location) = listing.location.clone() {
                    <p class="mb-2 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">{ location }</p>
                }
                <div class="flex items-start justify-between gap-6">
                    <h3 class="font-serif text-2xl font-light leading-tight text-foreground">{ listing.name.clone() }</h3>
                    <p class="whitespace-nowrap pt-1 text-sm font-light text-foreground">
                        { crate::format::listing_price_label(listing) }
                    </p>
                </div>
                <div class="mt-3 flex items-center justify-between gap-4">
                    <p class="text-[11px] font-light uppercase tracking-[0.14em] text-muted-foreground">{ facts }</p>
                    <svg class="h-4 w-4 flex-none text-muted-foreground transition-transform duration-500 group-hover:-translate-y-0.5 group-hover:translate-x-0.5"
                        viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                        <path d="M7 17 17 7M9 7h8v8" />
                    </svg>
                </div>
            </div>
        </article>
    }
}

impl Buyers {
    /// "Buying on Culebra" — the four-step path, from `BUYER_STEPS`.
    fn steps(&self) -> Html {
        html! {
            <section class="px-6 py-24 md:px-12 md:py-32">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mb-16 max-w-3xl md:mb-20">
                        <p class="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Buying on Culebra"}</p>
                        <h2 class="text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                            {"A considered path from first look to ownership."}
                        </h2>
                        <p class="mt-6 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                            {"Finding the right property is only the beginning. We guide the details that follow — privately, carefully, and with an understanding of how transactions work on the island."}
                        </p>
                    </div>
                    <div class="grid gap-12 md:grid-cols-2 lg:grid-cols-4 lg:gap-8">
                        { for BUYER_STEPS.iter().map(|(number, title, body)| html! {
                            <div class="border-t border-border pt-7">
                                <span class="font-serif text-2xl font-light text-accent">{ *number }</span>
                                <h3 class="mt-7 font-serif text-2xl font-light leading-snug text-foreground">{ *title }</h3>
                                <p class="mt-4 text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        }
    }

    /// The dark band: not every exceptional property is publicly listed.
    fn private_opportunities(&self) -> Html {
        html! {
            <section class="bg-foreground px-6 py-24 text-background md:px-12 md:py-32">
                <div class="mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:items-center">
                    <div class="lg:col-span-7">
                        <p class="mb-5 text-xs font-light uppercase tracking-[0.34em] text-background/50">{"Private Opportunities"}</p>
                        <h2 class="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl lg:text-6xl">
                            {"Not every exceptional property is publicly listed."}
                        </h2>
                    </div>
                    <div class="lg:col-span-4 lg:col-start-9">
                        <p class="text-sm font-light leading-relaxed text-background/70">
                            {"Culebra remains a small island with a highly relationship-driven property market. Some owners prefer discretion. Tell us what you are looking for, and we can widen the search beyond the public inventory."}
                        </p>
                        <a href="/contact" class="group mt-8 inline-flex items-center gap-3 border border-background/30 px-8 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 hover:border-background">
                            {"Begin a private search"}
                            <span class="inline-block h-px w-8 bg-background transition-all duration-500 group-hover:w-12"></span>
                        </a>
                    </div>
                </div>
            </section>
        }
    }

    /// The closing list, from `BUYER_SERVICES`.
    fn beyond_the_search(&self) -> Html {
        html! {
            <section class="px-6 py-24 md:px-12 md:py-32">
                <div class="mx-auto grid max-w-[1600px] gap-14 lg:grid-cols-12 lg:gap-20">
                    <div class="lg:col-span-5">
                        <p class="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Beyond the Search"}</p>
                        <h2 class="max-w-lg text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                            {"Every detail, quietly handled."}
                        </h2>
                    </div>
                    <div class="lg:col-span-6 lg:col-start-7">
                        <ul class="flex flex-col divide-y divide-border border-y border-border">
                            { for BUYER_SERVICES.iter().map(|item| html! {
                                <li class="py-5 text-sm font-light tracking-wide text-foreground/80">{ *item }</li>
                            }) }
                        </ul>
                    </div>
                </div>
            </section>
        }
    }
}
