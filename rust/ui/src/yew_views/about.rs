//! `/about` — the About page, on Yew.
//!
//! THE COPY IS LITERAL BECAUSE IT IS LITERAL IN THE PAGE. `app/about/page.tsx` reads no managed content: the founder's
//! biography, the values, the reasons, the four figures and the strip are written into the page, and they are written
//! into the shared tables here (`ABOUT_VALUES`, `ABOUT_REASONS`, `ABOUT_STATS`, `ABOUT_CREDENTIALS`, `ABOUT_LIFE`) so
//! there is one copy of them. `components/about.tsx` — the homepage's band — is a different thing, and reading it here
//! is the mistake this port has already written down.

use yew::prelude::*;

use crate::icons::icon_html;
use crate::view::{ABOUT_CREDENTIALS, ABOUT_LIFE, ABOUT_REASONS, ABOUT_STATS, ABOUT_VALUES};
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::PageProps;

pub struct About;

impl Component for About {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <>
                { page_hero(
                    "About Us",
                    "Devoted to a single island.",
                    Some("CulebraLuxe is a boutique brokerage working with few clients, few homes, and an uncommon amount of care."),
                    "/images/about/about-hero.jpg",
                    "Aerial view across Culebra and the surrounding Caribbean water",
                ) }
                { self.founder() }
                { self.values() }
                { self.reasons() }
                { self.stats() }
                { self.life() }
                { self.closing() }
            </>
        }
    }
}

impl About {
    /// The founder: her portrait, her biography, her words and her credentials.
    fn founder(&self) -> Html {
        html! {
            <section class="px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid items-stretch gap-12 md:grid-cols-2 md:gap-0">
                        <div class="h-full min-h-[520px] overflow-hidden bg-muted md:min-h-[680px]">
                            <img src="/images/about/lisa-portrait.jpg" alt="Lisa Penfield, founder and broker of CulebraLuxe"
                                class="h-full w-full object-cover" />
                        </div>
                        <div class="flex h-full flex-col justify-center px-0 py-4 md:px-16 lg:px-20">
                            <p class="text-xs font-light uppercase tracking-[0.28em] text-accent">{"Lisa Penfield"}</p>
                            <h2 class="mt-5 font-serif text-4xl font-light leading-[1.1] text-foreground md:text-5xl">
                                {"Founder & Broker"}
                            </h2>
                            <div class="mt-8 max-w-xl space-y-5 text-sm font-light leading-relaxed text-muted-foreground">
                                <p>{"Lisa Penfield has spent more than two decades reading Culebra — first as a world-champion windsurfer studying its wind and water, later as a broker studying its land and light."}</p>
                                <p>{"The same discipline that wins a championship applies just as well to representing a home: patience, precision, and knowing exactly when to act."}</p>
                                <p>{"Licensed in Puerto Rico since 2002, Lisa also brings 20 years of experience in luxury hospitality and corporate travel at Hyatt Dorado Beach. A full-time resident of Culebra, she works with a deliberately small number of clients each year — enough to give every search, listing, and negotiation her full attention."}</p>
                            </div>
                            <p class="mt-8 font-serif text-xl font-light italic text-accent">
                                {"“Here, it’s personal. Always has been.”"}
                            </p>
                            <div class="mt-10 divide-y divide-border border-y border-border">
                                { for ABOUT_CREDENTIALS.iter().map(|item| html! {
                                    <div class="flex items-center gap-4 py-4">
                                        <span class="text-accent">{"○"}</span>
                                        <p class="text-sm font-light text-foreground">{ *item }</p>
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

impl About {
    /// "What we value" — three centred cards, each with its icon from the table.
    fn values(&self) -> Html {
        html! {
            <section class="px-6 py-20 md:px-12 md:py-24">
                <div class="mx-auto max-w-[1600px]">
                    <h2 class="text-center font-serif text-3xl font-light text-foreground md:text-4xl">{"What we value"}</h2>
                    <div class="mt-14 grid gap-14 md:grid-cols-3 md:gap-0">
                        { for ABOUT_VALUES.iter().map(|(title, body, icon_name)| html! {
                            <div class="px-4 text-center md:border-r md:border-border md:px-12">
                                <div class="mx-auto mb-6 flex h-12 w-12 items-center justify-center text-accent">
                                    { match icon_html(icon_name, "h-10 w-10", "1.25") {
                                        Some(svg) => svg,
                                        None => html! { <span aria-hidden="true">{"\u{2014}"}</span> },
                                    } }
                                </div>
                                <h3 class="text-sm font-medium uppercase tracking-[0.16em] text-foreground">{ *title }</h3>
                                <p class="mx-auto mt-4 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        }
    }

    /// "Why clients choose CulebraLuxe" — the four reasons beside a photograph of Lisa at work.
    fn reasons(&self) -> Html {
        html! {
            <section class="bg-muted/30 px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid items-center gap-12 md:grid-cols-2 md:gap-16">
                        <div>
                            <h2 class="font-serif text-3xl font-light leading-tight text-foreground md:text-4xl">
                                {"Why clients choose CulebraLuxe"}
                            </h2>
                            <div class="mt-10 space-y-7">
                                { for ABOUT_REASONS.iter().map(|(title, body)| html! {
                                    <div class="flex gap-4">
                                        <div class="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-accent text-xs text-accent">
                                            {"✓"}
                                        </div>
                                        <p class="text-sm font-light leading-relaxed text-muted-foreground">
                                            <span class="font-medium text-foreground">{ *title }</span>
                                            { " — " }
                                            { *body }
                                        </p>
                                    </div>
                                }) }
                            </div>
                        </div>
                        <div class="aspect-[4/3] overflow-hidden bg-muted">
                            <img src="/images/about/lisa-work.jpg" alt="Lisa Penfield working with clients in Culebra"
                                class="h-full w-full object-cover" />
                        </div>
                    </div>
                </div>
            </section>
        }
    }

    /// The four figures.
    fn stats(&self) -> Html {
        html! {
            <section class="px-6 py-16 md:px-12 md:py-20">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid grid-cols-2 gap-y-10 md:grid-cols-4">
                        { for ABOUT_STATS.iter().map(|(figure, label)| html! {
                            <div class="text-center md:border-r md:border-border">
                                <p class="font-serif text-4xl font-light text-accent md:text-5xl">{ *figure }</p>
                                <p class="mt-3 text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">{ *label }</p>
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        }
    }
}


impl About {
    /// "Life on the island" — eight photographs in a single scrolling row.
    fn life(&self) -> Html {
        html! {
            <section class="px-6 pb-20 pt-8 md:px-12 md:pb-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="text-center">
                        <h2 class="font-serif text-3xl font-light text-foreground md:text-4xl">{"Life on the island"}</h2>
                        <p class="mx-auto mt-4 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                            {"A quiet collection of business and personal moments that reflect the pace, place, and perspective behind the brand."}
                        </p>
                    </div>
                    <div class="mt-12 flex gap-2 overflow-x-auto pb-2">
                        { for ABOUT_LIFE.iter().enumerate().map(|(index, src)| html! {
                            <div class="h-[220px] w-[180px] shrink-0 overflow-hidden bg-muted md:h-[260px] md:w-[220px]">
                                <img src={src.to_string()} alt={format!("Life on Culebra {}", index + 1)}
                                    class="h-full w-full object-cover" />
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        }
    }

    /// The closing band.
    fn closing(&self) -> Html {
        html! {
            <section class="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
                <div class="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
                    <h2 class="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                        {"We would be glad to know what you are looking for."}
                    </h2>
                    <a href="/contact" class="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]">
                        {"Start a conversation"}
                        <span class="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16"></span>
                    </a>
                </div>
            </section>
        }
    }
}

