//! `/guide` — the Island Guide, on Yew.
//!
//! A CATALOGUE, NOT COPY. Nine sections whose headings are the page's structure (literal, as in the string renderer) and
//! whose cards arrive from `guide_item` through the same page payload. The section table is `view::GUIDE_SECTIONS`, the
//! very one the string renderer reads, so the two cannot disagree about the island's sections while both exist.
//!
//! The jump nav is nine in-page anchors — real `<a href="#id">`, and `id` is on the section with `scroll-mt-24` so the
//! sticky header does not cover the heading it just scrolled to.

use yew::prelude::*;

use crate::model::GuideItem;
use crate::view::GUIDE_SECTIONS;
use crate::yew_router::Route;
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::{NavLink, PageProps};

pub struct Guide;

impl Component for Guide {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
                { page_hero(
                    "Island Guide",
                    "A slower rhythm, kept intentionally intact.",
                    Some("No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by noon, and evenings measured in shades of gold."),
                    "/images/culture.png",
                    "The white sand crescent and turquoise water of Flamenco Beach, Culebra",
                ) }
                { self.jump_nav() }
                { self.sections(&ctx.props().model) }
                { self.closing() }
            </>
        }
    }
}

impl Guide {
    /// The nine-section index under the hero, in the page's own order.
    fn jump_nav(&self) -> Html {
        html! {
            <section class="border-b border-border px-6 md:px-12">
                <div class="mx-auto max-w-[1600px] overflow-x-auto">
                    <nav class="flex min-w-max gap-8 py-6 md:gap-10">
                        { for GUIDE_SECTIONS.iter().map(|(id, _, title, _, _)| html! {
                            <a href={format!("#{id}")}
                                class="text-[11px] font-light uppercase tracking-[0.22em] text-muted-foreground transition-colors hover:text-foreground">
                                { *title }
                            </a>
                        }) }
                    </nav>
                </div>
            </section>
        }
    }

    /// Every section, with the cards that belong to it — filtered by the entry's own `section`, which is exactly how the
    /// string renderer grouped them and how the database groups them.
    fn sections(&self, model: &crate::model::Model) -> Html {
        let entries: &[GuideItem] = model
            .page
            .as_ref()
            .map(|page| page.guide.as_slice())
            .unwrap_or(&[]);
        html! {
            <section class="px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="space-y-24 md:space-y-32">
                        { for GUIDE_SECTIONS.iter().map(|(id, number, title, headline, description)| html! {
                            <section id={*id} class="scroll-mt-24">
                                <div class="grid gap-10 md:grid-cols-12 md:gap-12">
                                    <div class="md:col-span-3">
                                        <p class="text-xs font-light uppercase tracking-[0.28em] text-accent">
                                            { format!("{number} / {title}") }
                                        </p>
                                        <h2 class="mt-5 font-serif text-2xl font-light leading-[1.2] text-foreground md:text-3xl">
                                            { *headline }
                                        </h2>
                                        <p class="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                                            { *description }
                                        </p>
                                    </div>
                                    <div class="min-w-0 md:col-span-9">
                                        <div class="flex gap-5 overflow-x-auto pb-4">
                                            { for entries.iter().filter(|item| item.section == *id).map(guide_card) }
                                        </div>
                                    </div>
                                </div>
                            </section>
                        }) }
                    </div>
                </div>
            </section>
        }
    }

    /// The closing band: the invitation, and the way to act on it.
    fn closing(&self) -> Html {
        html! {
            <section class="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
                <div class="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
                    <h2 class="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                        {"When you are ready to find your place here."}
                    </h2>
                    <NavLink to={Route::Buyers}
                        classes={classes!("group", "inline-flex", "items-center", "gap-3", "text-xs", "font-light", "uppercase", "tracking-[0.24em]")}>
                        {"Explore buying on Culebra"}
                        <span class="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16"></span>
                    </NavLink>
                </div>
            </section>
        }
    }
}

/// One place in the guide: its photograph, its area, its name, what it is, and how to reach it.
///
/// The three optional lines are each shown only when the entry has them, and the overline picks the first of
/// neighbourhood, area and eyebrow that exists — the string renderer's rule, kept, so a card cannot gain an empty line.
fn guide_card(item: &GuideItem) -> Html {
    let overline = item
        .subtitle
        .as_deref()
        .or(item.area.as_deref())
        .or(item.eyebrow.as_deref())
        .unwrap_or("");
    html! {
        <article class="w-[78vw] max-w-[280px] shrink-0 sm:w-[240px] lg:w-[220px]">
            <div class="aspect-[4/3] overflow-hidden bg-muted">
                if let Some(src) = item.image_path.clone() {
                    <img src={src}
                        alt={item.image_alt.clone().unwrap_or_else(|| item.name.clone())}
                        class="h-full w-full object-cover" />
                } else {
                    // An entry with no photograph says so, rather than showing a broken frame.
                    <div class="flex h-full items-center justify-center">
                        <span class="text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                            {"Image coming soon"}
                        </span>
                    </div>
                }
            </div>
            <div class="pt-5">
                <p class="text-[10px] font-light uppercase tracking-[0.22em] text-accent">{ overline }</p>
                <h3 class="mt-2 font-serif text-xl font-light leading-tight text-foreground">{ item.name.clone() }</h3>
                <p class="mt-3 text-sm font-light leading-relaxed text-muted-foreground">{ item.description.clone() }</p>
                if let Some(address) = item.address.clone() {
                    <p class="mt-4 text-xs font-light leading-relaxed text-muted-foreground">{ address }</p>
                }
                if let Some(phone) = item.phone.clone() {
                    <a href={format!("tel:{phone}")} class="mt-2 block text-xs font-light text-foreground">{ phone }</a>
                }
                if let Some(website) = item.website_url.clone() {
                    <a href={website} target="_blank" rel="noreferrer"
                        class="mt-3 inline-block text-[10px] font-light uppercase tracking-[0.2em] text-foreground">
                        {"Visit website →"}
                    </a>
                }
            </div>
        </article>
    }
}
