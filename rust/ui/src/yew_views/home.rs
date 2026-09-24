//! `/` — the landing page, on Yew.
//!
//! THE SECTIONS ARE THE COMPONENTS THE PAGE IS MADE OF: the hero, the Collection, the dark portfolio band, the services
//! band, culture, and the about band. Their words come from the payload (`PageContent`) exactly as the string renderer
//! read them, and the bands that the live page only shows when there is something to show are conditional here the same
//! way — a portfolio section with nothing in it is worse than no portfolio section.
//!
//! NO PAYLOAD YET IS AN EMPTY BODY, not invented copy: the chrome and the loading line are the shell's, which is what
//! `site_home` does when `model.page` is `None`.

use yew::prelude::*;

use crate::format::{listing_enquire_href, listing_facts, listing_price_label, FactsStyle};
use crate::model::{Block, Listing};
use crate::yew_views::chrome::PageProps;

pub struct Home;

impl Component for Home {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Some(page) = ctx.props().model.page.as_ref() else {
            // The chrome and the loading line are already on screen. An empty body here is honest; filling it with
            // invented copy would be worse than a page that is still arriving.
            return Html::default();
        };
        html! {
            <>
                { hero(&page.hero) }
                if !page.listings.is_empty() {
                    { self.collection(&page.featured) }
                    { self.portfolio(&page.buyers, &page.listings) }
                }
                { self.services_band(&page.buyers, &page.sellers) }
                { self.culture(&page.culture) }
                { self.about_band(&page.about) }
            </>
        }
    }
}
/// `components/hero.tsx` — a full-viewport image, two scrims, and the block's copy pinned to the bottom edge.
fn hero(block: &Block) -> Html {
    let image = block
        .image_path
        .as_deref()
        .unwrap_or("/images/hero-villa.png");
    let alt = block
        .image_alt
        .as_deref()
        .unwrap_or("Cliffside modern villa overlooking the turquoise Caribbean sea in Culebra");
    html! {
        <section id="top" class="relative h-[100svh] w-full overflow-hidden">
            <img src={image.to_string()} alt={alt.to_string()} sizes="100vw"
                class="absolute inset-0 h-full w-full object-cover" />
            <div class="absolute inset-0 bg-gradient-to-b from-black/40 via-black/10 to-black/50"></div>
            <div class="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent"></div>
            <div class="relative flex h-full flex-col justify-end px-6 pb-20 md:px-12 md:pb-28">
                <div class="mx-auto w-full max-w-[1600px]">
                    <p class="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/70">{ block.eyebrow.clone() }</p>
                    <h1 class="max-w-4xl text-balance font-serif text-5xl font-light leading-[1.02] text-background md:text-7xl lg:text-8xl">
                        { block.title.clone() }
                    </h1>
                    <div class="mt-10 flex flex-col gap-6 border-t border-background/25 pt-8 md:flex-row md:items-end md:justify-between">
                        <p class="max-w-md text-pretty text-sm font-light leading-relaxed text-background/80">{ block.body.clone() }</p>
                        { rule_cta(block, "#properties", "bg-background") }
                    </div>
                </div>
            </div>
        </section>
    }
}

/// A block's call to action as the ruled link the design uses, or nothing when the block has no label.
pub fn rule_cta(block: &Block, fallback: &'static str, rule: &'static str) -> Html {
    let Some(label) = block.cta_label.as_deref().filter(|label| !label.is_empty()) else {
        return Html::default();
    };
    let href = block.cta_href.as_deref().unwrap_or(fallback);
    html! {
        <a href={href.to_string()} class="inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]">
            { label.to_string() }
            <span class={classes!("inline-block", "h-px", "w-10", rule)}></span>
        </a>
    }
}

impl Home {
    /// "The Collection" — three estates, each an image beside its numeral, name, facts and price, alternating sides.
    ///
    /// THE SIDES ALTERNATE BY INDEX, as the component did it: `md:[direction:rtl]` on every second article with the inner
    /// columns set back to `ltr`. The alternation is most of what the section looks like, so it is reproduced rather than
    /// simplified to one side.
    fn collection(&self, items: &[Listing]) -> Html {
        let shown = &items[..items.len().min(3)];
        html! {
            <section id="properties" class="px-6 py-28 md:px-12 md:py-40">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mb-20 md:mb-28">
                        <div class="flex flex-col gap-6 border-b border-border pb-10 md:flex-row md:items-end md:justify-between">
                            <div>
                                <p class="mb-4 text-xs font-light uppercase tracking-[0.34em] text-accent">{"The Collection"}</p>
                                <h2 class="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-foreground md:text-6xl">
                                    {"Residences chosen for their silence."}
                                </h2>
                            </div>
                            <p class="max-w-xs text-pretty text-sm font-light leading-relaxed text-muted-foreground">
                                {"Each estate is selected in person, for its light, its outlook, and its relationship to the sea."}
                            </p>
                        </div>
                    </div>
                    <div class="flex flex-col gap-28 md:gap-40">
                        if shown.is_empty() {
                            <p class="max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                                {"The next collection is being prepared."}
                            </p>
                        } else {
                            { for shown.iter().enumerate().map(|(index, listing)| {
                                let numeral = format!("{:02}", index + 1);
                                let reversed = index % 2 == 1;
                                html! {
                                    <article class={classes!(
                                        "grid", "items-center", "gap-10", "md:grid-cols-12", "md:gap-16",
                                        reversed.then_some("md:[direction:rtl]")
                                    )}>
                                        <div class="md:col-span-8 md:[direction:ltr]">
                                            <a href={format!("/properties/{}", listing.slug)} aria-label={format!("View {}", listing.name)}
                                                class="group relative block aspect-[16/10] w-full overflow-hidden">
                                                <img src={listing.image_path.clone().unwrap_or_else(|| "/placeholder.svg".to_string())}
                                                    alt={listing.image_alt.clone().unwrap_or_else(|| listing.name.clone())}
                                                    sizes="(min-width: 768px) 66vw, 100vw"
                                                    class="absolute inset-0 h-full w-full object-cover transition-transform duration-[1600ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.04]" />
                                            </a>
                                        </div>
                                        <div class="md:col-span-4 md:[direction:ltr]">
                                            <span class="font-serif text-sm font-light text-accent">{ format!("({numeral})") }</span>
                                            <h3 class="mt-4 font-serif text-3xl font-light leading-tight text-foreground md:text-4xl">
                                                <a href={format!("/properties/{}", listing.slug)} class="transition-colors duration-300 hover:text-accent">
                                                    { listing.name.clone() }
                                                </a>
                                            </h3>
                                            if let Some(location) = listing.location.clone().filter(|value| !value.is_empty()) {
                                                <p class="mt-3 text-xs font-light uppercase tracking-[0.24em] text-muted-foreground">{ location }</p>
                                            }
                                            <p class="mt-8 max-w-xs text-sm font-light leading-relaxed text-foreground/80">
                                                { listing_facts(listing, FactsStyle::Full) }
                                            </p>
                                            <div class="mt-8 flex items-center justify-between border-t border-border pt-6">
                                                <span class="text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">
                                                    { listing_price_label(listing) }
                                                </span>
                                                <a href={listing_enquire_href(listing)} class="inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground">
                                                    {"Enquire"}<span class="inline-block h-px w-6 bg-foreground"></span>
                                                </a>
                                            </div>
                                        </div>
                                    </article>
                                }
                            }) }
                        }
                    </div>
                </div>
            </section>
        }
    }
}

impl Home {
    /// The dark portfolio band: a heading, a "View All Properties" button, and four cards.
    fn portfolio(&self, block: &Block, items: &[Listing]) -> Html {
        let shown = &items[..items.len().min(4)];
        if shown.is_empty() {
            // The component rendered nothing at all with no listings, rather than an empty band.
            return Html::default();
        }
        let eyebrow = if block.eyebrow.is_empty() {
            "The Portfolio"
        } else {
            block.eyebrow.as_str()
        };
        let title = if block.title.is_empty() {
            "Find your place in Culebra."
        } else {
            block.title.as_str()
        };
        let intro = if block.body.is_empty() {
            "Exquisite properties on an extraordinary island — each chosen for its light, its outlook, and its relationship to the sea."
        } else {
            block.body.as_str()
        };
        html! {
            <section class="bg-foreground px-6 py-24 text-background md:px-12 md:py-32">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mb-14 flex flex-col gap-8 md:flex-row md:items-end md:justify-between">
                        <div class="max-w-xl">
                            <p class="mb-4 text-xs font-light uppercase tracking-[0.34em] text-background/60">{ eyebrow.to_string() }</p>
                            <h2 class="text-balance font-serif text-4xl font-light leading-[1.05] md:text-5xl">{ title.to_string() }</h2>
                            <p class="mt-5 max-w-md text-pretty text-sm font-light leading-relaxed text-background/70">{ intro.to_string() }</p>
                        </div>
                        <a href="/properties" class="inline-flex items-center gap-3 self-start border border-background/30 px-8 py-4 text-xs font-light uppercase tracking-[0.2em] transition-colors duration-500 hover:border-background md:self-auto">
                            {"View All Properties"}<span aria-hidden="true">{"\u{2192}"}</span>
                        </a>
                    </div>
                    <div class="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
                        { for shown.iter().map(|listing| html! {
                            <article class="group flex flex-col">
                                <div class="relative aspect-[5/4] w-full overflow-hidden bg-background/10">
                                    <a href={format!("/properties/{}", listing.slug)} aria-label={listing.name.clone()}>
                                        <img src={listing.image_path.clone().unwrap_or_else(|| "/placeholder.svg".to_string())}
                                            alt={listing.image_alt.clone().unwrap_or_else(|| listing.name.clone())}
                                            sizes="(min-width: 1024px) 22vw, (min-width: 640px) 45vw, 90vw"
                                            class="absolute inset-0 h-full w-full object-cover transition-transform duration-[1400ms] ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:scale-[1.05]" />
                                    </a>
                                    if listing.featured {
                                        <span class="absolute left-3 top-3 bg-background/90 px-3 py-1 text-[10px] font-light uppercase tracking-[0.18em] text-foreground">
                                            {"Featured"}
                                        </span>
                                    }
                                </div>
                                <div class="mt-5 flex items-baseline justify-between gap-4">
                                    <a href={format!("/properties/{}", listing.slug)} class="font-serif text-xl font-light transition-colors duration-300 hover:text-background/70">
                                        { listing.name.clone() }
                                    </a>
                                    <span class="whitespace-nowrap text-sm font-light text-background/80">
                                        { listing_price_label(listing) }
                                    </span>
                                </div>
                                <p class="mt-2 text-[11px] font-light uppercase tracking-[0.16em] text-background/55">{ listing_facts(listing, FactsStyle::Compact) }</p>
                            </article>
                        }) }
                    </div>
                </div>
            </section>
        }
    }
}

impl Home {
    /// `components/services.tsx` — the dark primary band: buyers over sellers.
    fn services_band(&self, buyers: &Block, sellers: &Block) -> Html {
        let buyer_items = buyers
            .items
            .iter()
            .filter(|item| item.key == "list")
            .filter_map(|item| item.value.clone())
            .collect::<Vec<_>>();
        html! {
            <div class="bg-primary text-primary-foreground">
                <section id="buyers" class="border-b border-primary-foreground/10 px-6 py-28 md:px-12 md:py-40">
                    <div class="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-2 md:gap-24">
                        <div>
                            <p class="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">{ buyers.eyebrow.clone() }</p>
                            <h2 class="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">{ buyers.title.clone() }</h2>
                        </div>
                        <div class="flex flex-col justify-center gap-10">
                            <p class="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">{ buyers.body.clone() }</p>
                            <ul class="flex flex-col divide-y divide-primary-foreground/10 border-y border-primary-foreground/10">
                                { for buyer_items.iter().map(|value| html! {
                                    <li class="py-5 text-sm font-light tracking-wide text-primary-foreground/85">{ value.clone() }</li>
                                }) }
                            </ul>
                            { rule_cta(buyers, "#contact", "bg-primary-foreground") }
                        </div>
                    </div>
                </section>
                <section id="sellers" class="px-6 py-28 md:px-12 md:py-40">
                    <div class="mx-auto grid max-w-[1600px] items-center gap-14 md:grid-cols-2 md:gap-24">
                        <div class="relative aspect-[4/5] w-full overflow-hidden">
                            <img src={sellers.image_path.clone().unwrap_or_else(|| "/images/coastline.png".to_string())}
                                alt={sellers.image_alt.clone().unwrap_or_else(|| "Aerial view of the Culebra coastline with jade and turquoise water".to_string())}
                                sizes="(min-width: 768px) 50vw, 100vw"
                                class="absolute inset-0 h-full w-full object-cover" />
                        </div>
                        <div class="flex flex-col gap-10">
                            <div>
                                <p class="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">{ sellers.eyebrow.clone() }</p>
                                <h2 class="text-balance font-serif text-4xl font-light leading-[1.06] md:text-5xl">{ sellers.title.clone() }</h2>
                            </div>
                            <p class="max-w-md text-pretty text-sm font-light leading-relaxed text-primary-foreground/75">{ sellers.body.clone() }</p>
                            { rule_cta(sellers, "#contact", "bg-primary-foreground") }
                        </div>
                    </div>
                </section>
            </div>
        }
    }
}

impl Home {
    /// `components/culture.tsx` — a full-bleed image with the title over it, then an editorial column.
    fn culture(&self, block: &Block) -> Html {
        let stats = block
            .items
            .iter()
            .filter(|item| item.key == "stat")
            .map(|item| {
                (
                    item.label.clone().unwrap_or_default(),
                    item.value.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        html! {
            <section id="culture" class="relative">
                <div class="relative h-[85svh] w-full overflow-hidden">
                    <img src={block.image_path.clone().unwrap_or_else(|| "/images/culture.png".to_string())}
                        alt={block.image_alt.clone().unwrap_or_else(|| "The white sand crescent and clear turquoise water of Flamenco Beach, Culebra".to_string())}
                        sizes="100vw" class="absolute inset-0 h-full w-full object-cover" />
                    <div class="absolute inset-0 bg-gradient-to-t from-black/55 via-black/15 to-black/25"></div>
                    <div class="absolute inset-0 flex flex-col justify-end px-6 pb-20 md:px-12 md:pb-28">
                        <div class="mx-auto w-full max-w-[1600px]">
                            <p class="mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70">{ block.eyebrow.clone() }</p>
                            <h2 class="max-w-3xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                                { block.title.clone() }
                            </h2>
                        </div>
                    </div>
                </div>
                <div class="px-6 py-24 md:px-12 md:py-32">
                    <div class="mx-auto grid max-w-[1600px] gap-14 md:grid-cols-12 md:gap-24">
                        <p class="text-xs font-light uppercase tracking-[0.28em] text-accent md:col-span-4">{ block.subtitle.clone() }</p>
                        <div class="md:col-span-8">
                            <p class="max-w-3xl text-balance font-serif text-2xl font-light leading-[1.4] text-foreground md:text-3xl">
                                { block.body.clone() }
                            </p>
                            <div class="mt-14 grid gap-10 border-t border-border pt-10 sm:grid-cols-3">
                                { for stats.iter().map(|(label, value)| html! {
                                    <div>
                                        <p class="font-serif text-xl font-light text-foreground">{ label.clone() }</p>
                                        <p class="mt-2 text-sm font-light leading-relaxed text-muted-foreground">{ value.clone() }</p>
                                    </div>
                                }) }
                            </div>
                        </div>
                    </div>
                </div>
            </section>
        }
    }
}

impl Home {
    /// `components/about.tsx` — the eyebrow, the statement, and three columns: body, first paragraph, and the stats.
    ///
    /// THE THIRD COLUMN IS THE STATS and the middle one is the block's first `paragraph` item — that is the component's
    /// structure, not a simplification: the stats sit beside the prose rather than under it.
    fn about_band(&self, block: &Block) -> Html {
        let paragraph = block
            .items
            .iter()
            .filter(|item| item.key == "paragraph")
            .filter_map(|item| item.value.clone())
            .next()
            .unwrap_or_default();
        let stats = block
            .items
            .iter()
            .filter(|item| item.key == "stat")
            .map(|item| {
                (
                    item.label.clone().unwrap_or_default(),
                    item.value.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        html! {
            <section id="about" class="px-6 py-28 md:px-12 md:py-40">
                <div class="mx-auto max-w-[1600px]">
                    <p class="mb-16 text-xs font-light uppercase tracking-[0.34em] text-accent md:mb-24">{ block.eyebrow.clone() }</p>
                    <h2 class="max-w-5xl text-balance font-serif text-3xl font-light leading-[1.2] text-foreground md:text-5xl md:leading-[1.18]">
                        { block.title.clone() }
                    </h2>
                    <div class="mt-20 grid gap-14 border-t border-border pt-16 md:mt-28 md:grid-cols-3 md:gap-16">
                        <p class="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">{ block.body.clone() }</p>
                        <p class="max-w-sm text-sm font-light leading-relaxed text-muted-foreground">{ paragraph }</p>
                        <div class="flex flex-col justify-between gap-8">
                            <div class="grid grid-cols-2 gap-8">
                                { for stats.iter().map(|(label, value)| html! {
                                    <div>
                                        <p class="font-serif text-4xl font-light text-foreground">{ label.clone() }</p>
                                        <p class="mt-2 text-xs font-light uppercase tracking-[0.2em] text-muted-foreground">{ value.clone() }</p>
                                    </div>
                                }) }
                            </div>
                        </div>
                    </div>
                </div>
            </section>
        }
    }
}

