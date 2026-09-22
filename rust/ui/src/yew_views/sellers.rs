//! `/sellers` — the Sellers page, on Yew.
//!
//! SEVEN SECTIONS, ALL OF THEM THE PAGE'S OWN COPY, and all of it from the shared tables (`SELLER_WHY_US`,
//! `SELLER_PROCESS`, `SELLER_MARKET_LEFT/RIGHT`, `SELLER_PRESENTATION`, `SELLER_DISTRIBUTION`,
//! `SELLER_REPRESENTATION`) so the page cannot say two different things. The icons come through `icons::icon_html`, the
//! approved static-icon adapter — the workflow's arrows, the market diagram's dimensions, the exposure lists and the six
//! stages all draw from the one table.
//!
//! The page reads no managed content: it is an editorial screen, the payload arrives, and it is deliberately unused —
//! exactly as the string renderer does.

use yew::prelude::*;

use crate::icons::icon_html;
use crate::view::{
    SELLER_DISTRIBUTION, SELLER_MARKET_LEFT, SELLER_MARKET_RIGHT, SELLER_PRESENTATION,
    SELLER_PROCESS, SELLER_REPRESENTATION, SELLER_WHY_US,
};
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::PageProps;

/// The label a section puts above its heading.
fn section_number(number: &str) -> Html {
    html! { <p class="text-xs font-light uppercase tracking-[0.28em] text-accent">{ number.to_string() }</p> }
}

/// An icon, or a visible em dash when the table has no such name — a missing icon is a mistake, not an empty box.
fn icon(name: &str, class: &str, weight: &str) -> Html {
    match icon_html(name, class, weight) {
        Some(svg) => svg,
        None => html! { <span aria-hidden="true">{"\u{2014}"}</span> },
    }
}

pub struct Sellers;

impl Component for Sellers {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <>
                { page_hero(
                    "Selling on Culebra",
                    "Presented to the few who truly belong here.",
                    Some("Extraordinary properties deserve more than exposure — they deserve understanding, strategy, and representation."),
                    "/images/coastline.png",
                    "Culebra coastline and homes overlooking the Caribbean",
                ) }
                { self.why_us() }
                { self.process() }
                { self.market() }
                { self.presentation() }
                { self.representation() }
                { self.closing() }
            </>
        }
    }
}
impl Sellers {
    /// 01 — Why CulebraLuxe: the argument on the left, three reasons to its right.
    fn why_us(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#f6f3ed] px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-14 md:grid-cols-12 md:gap-16">
                        <div class="md:col-span-3">
                            { section_number("01") }
                            <h2 class="mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">{"Why CulebraLuxe"}</h2>
                            <p class="mt-6 text-sm font-medium leading-relaxed text-foreground">{"Selling on Culebra is different."}</p>
                            <p class="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                                {"This is not a conventional real estate market. Inventory is limited, data is fragmented, and every property is unique. Value is shaped by factors an algorithm will never see."}
                            </p>
                        </div>
                        <div class="grid gap-10 md:col-span-9 md:grid-cols-3 md:gap-0">
                            { for SELLER_WHY_US.iter().map(|(title, body, icon_name)| html! {
                                <div class="h-full md:border-l md:border-border md:px-10">
                                    { icon(icon_name, "h-10 w-10 text-accent", "1.25") }
                                    <h3 class="mt-6 text-sm font-medium uppercase tracking-[0.15em] text-foreground">{ *title }</h3>
                                    <p class="mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                                </div>
                            }) }
                        </div>
                    </div>
                </div>
            </section>
        }
    }
}

impl Sellers {
    /// 02 — Our Process: six steps, each cut into an arrow on a wide screen.
    fn process(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-12 md:grid-cols-12 md:gap-12">
                        <div class="md:col-span-2">
                            { section_number("02") }
                            <h2 class="mt-4 font-serif text-3xl font-light text-foreground">{"Our Process"}</h2>
                            <p class="mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                                {"A considered approach from first conversation to closing."}
                            </p>
                        </div>
                        <div class="md:col-span-10">
                            <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-6">
                                { for SELLER_PROCESS.iter().map(|(title, body, icon_name)| html! {
                                    <div class="relative h-full bg-[#f8f6f1] px-5 py-7 text-center lg:[clip-path:polygon(0_0,88%_0,100%_50%,88%_100%,0_100%,12%_50%)] lg:px-7">
                                        { icon(icon_name, "mx-auto h-8 w-8 text-accent", "1.25") }
                                        <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.16em] text-foreground">{ *title }</p>
                                        <p class="mt-4 text-xs font-light leading-relaxed text-muted-foreground">{ *body }</p>
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

impl Sellers {
    /// 03 — Market Positioning: five dimensions in, one considered position out.
    fn market(&self) -> Html {
        let inputs = |rows: &[(&str, &str, &str)]| {
            rows.iter()
                .map(|(title, body, icon_name)| {
                    html! {
                        <div class="flex gap-4">
                            { icon(icon_name, "mt-1 h-7 w-7 shrink-0 text-accent", "1.25") }
                            <div>
                                <p class="text-[10px] font-medium uppercase tracking-[0.15em] text-foreground">{ *title }</p>
                                <p class="mt-2 text-xs font-light leading-relaxed text-muted-foreground">{ *body }</p>
                            </div>
                        </div>
                    }
                })
                .collect::<Vec<_>>()
        };
        html! {
            <section class="border-b border-border bg-[#f8f6f1] px-6 py-24 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-14 md:grid-cols-12 md:gap-14">
                        <div class="md:col-span-3">
                            { section_number("03") }
                            <h2 class="mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">{"Market Positioning"}</h2>
                            <p class="mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                                {"We analyze five key dimensions to determine how your property should be positioned in today's Culebra market."}
                            </p>
                        </div>
                        <div class="md:col-span-9">
                            <div class="rounded-sm border border-border bg-[#fcfbf8] px-6 py-10 md:px-10 md:py-12">
                                <div class="grid items-center gap-10 lg:grid-cols-[1fr_auto_1.3fr_auto_1fr] lg:gap-5">
                                    <div class="space-y-9">{ for inputs(&SELLER_MARKET_LEFT) }</div>
                                    <div class="hidden items-center justify-center lg:flex">
                                        <span class="select-none font-serif text-[150px] font-extralight leading-none text-accent/35">{"}"}</span>
                                    </div>
                                    <div class="flex min-w-0 flex-col items-center">
                                        <div class="flex aspect-square w-full max-w-[310px] flex-col items-center justify-center rounded-full border border-accent/30 bg-[#eee8de] px-10 text-center shadow-[0_8px_30px_rgba(0,0,0,0.025)]">
                                            <p class="font-serif text-4xl font-light text-accent">{"CL"}</p>
                                            <p class="mt-5 text-xs font-medium uppercase tracking-[0.2em] text-foreground">
                                                {"CulebraLuxe"}<br />{"Analysis"}
                                            </p>
                                            <div class="mt-5 space-y-1 text-xs font-light text-muted-foreground">
                                                <p>{"Local knowledge."}</p>
                                                <p>{"Market intelligence."}</p>
                                                <p>{"Individual judgment."}</p>
                                            </div>
                                        </div>
                                        <div class="h-12 w-px bg-accent/30"></div>
                                        <div class="border border-accent/20 bg-[#f3eee6] px-10 py-5 text-center">
                                            <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground">{"Market Position"}</p>
                                            <p class="mt-2 font-serif text-sm font-light text-muted-foreground">{"Price · Strategy · Timing"}</p>
                                        </div>
                                    </div>
                                    <div class="hidden items-center justify-center lg:flex">
                                        <span class="select-none font-serif text-[150px] font-extralight leading-none text-accent/35">{"{"}</span>
                                    </div>
                                    <div class="space-y-12">{ for inputs(&SELLER_MARKET_RIGHT) }</div>
                                </div>
                                <div class="mt-10 border-t border-border pt-6 text-center lg:hidden">
                                    <p class="text-[10px] font-light uppercase tracking-[0.18em] text-muted-foreground">
                                        {"Five dimensions inform one considered market position."}
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </section>
        }
    }
}

impl Sellers {
    /// 04 — Presentation & Exposure: the two lists, and the line that ends the section.
    fn presentation(&self) -> Html {
        let panel = |label: &'static str, rows: &[(&str, &str)]| {
            html! {
                <div class="h-full border-border bg-[#faf8f4] p-8 md:border-l md:px-12 md:py-10">
                    <p class="text-xs font-medium uppercase tracking-[0.18em] text-foreground">{ label }</p>
                    <div class="mt-8 space-y-6">
                        { for rows.iter().map(|(title, icon_name)| html! {
                            <div class="flex items-center gap-4">
                                { icon(icon_name, "h-5 w-5 shrink-0 text-accent", "1.25") }
                                <p class="text-sm font-light text-foreground">{ *title }</p>
                            </div>
                        }) }
                    </div>
                </div>
            }
        };
        html! {
            <section class="border-b border-border bg-[#f0ece5] px-6 py-24 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-14 md:grid-cols-12 md:gap-14">
                        <div class="md:col-span-3">
                            { section_number("04") }
                            <h2 class="mt-4 font-serif text-3xl font-light leading-[1.05] text-foreground md:text-4xl">
                                {"Presentation"}<br />{"& Exposure"}
                            </h2>
                            <p class="mt-5 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                                {"The right property deserves more than a listing."}
                            </p>
                            <div class="relative mt-10 aspect-[4/3] overflow-hidden">
                                <img src="/images/hero-villa.png" alt="Culebra property prepared for market presentation"
                                    sizes="(min-width: 768px) 25vw, 100vw" class="absolute inset-0 h-full w-full object-cover" />
                            </div>
                        </div>
                        <div class="grid gap-12 md:col-span-9 md:grid-cols-2 md:gap-0">
                            { panel("Presentation", &SELLER_PRESENTATION) }
                            { panel("Distribution", &SELLER_DISTRIBUTION) }
                        </div>
                    </div>
                    <p class="mt-14 border-t border-border pt-8 text-center font-serif text-lg font-light text-foreground">
                        {"Exposure is not the strategy. Exposure serves the strategy."}
                    </p>
                </div>
            </section>
        }
    }

    /// 05 — Representation: six stages across the width.
    fn representation(&self) -> Html {
        html! {
            <section class="bg-[#f7f4ef] px-6 py-24 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-12 md:grid-cols-12 md:gap-12">
                        <div class="md:col-span-2">
                            { section_number("05") }
                            <h2 class="mt-4 font-serif text-3xl font-light text-foreground">{"Representation"}</h2>
                            <p class="mt-5 text-sm font-light leading-relaxed text-muted-foreground">{"From first showing to closing."}</p>
                        </div>
                        <div class="grid gap-10 sm:grid-cols-2 md:col-span-10 lg:grid-cols-6 lg:gap-0">
                            { for SELLER_REPRESENTATION.iter().map(|(title, body, icon_name)| html! {
                                <div class="h-full text-center lg:border-l lg:border-border lg:px-6">
                                    { icon(icon_name, "mx-auto h-8 w-8 text-accent", "1.25") }
                                    <h3 class="mt-5 text-[10px] font-medium uppercase tracking-[0.15em] text-foreground">{ *title }</h3>
                                    <p class="mt-4 text-xs font-light leading-relaxed text-muted-foreground">{ *body }</p>
                                </div>
                            }) }
                        </div>
                    </div>
                </div>
            </section>
        }
    }

    /// The closing band.
    fn closing(&self) -> Html {
        html! {
            <section class="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px] text-center">
                    <h2 class="font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                        {"Every property starts with a conversation."}
                    </h2>
                    <p class="mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70">
                        {"Tell us about your property. We'll discuss your objectives, the market, and whether working together makes sense."}
                    </p>
                    <a href="/contact" class="mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary">
                        {"Discuss your property"}
                    </a>
                </div>
            </section>
        }
    }
}

