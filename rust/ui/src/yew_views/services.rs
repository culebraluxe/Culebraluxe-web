//! `/services` — the services page, on Yew.
//!
//! THE CONTENT IS THE SAME TABLE AND THE SAME WORDS. The eight services, the process, the reasons and the principles are
//! `view::{SERVICES, SERVICE_PROCESS, SERVICE_REASONS, SERVICE_PRINCIPLES}` — the tables the string renderer reads — so
//! the two cannot disagree about what the page says while both exist.
//!
//! THE ICONS COME THROUGH `icons::icon_html`, the approved static-icon adapter: one source of geometry, no path data
//! copied into this file.

use yew::prelude::*;

use crate::icons::icon_html;
use crate::view::{SERVICES, SERVICE_PRINCIPLES, SERVICE_PROCESS, SERVICE_REASONS};
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::PageProps;

pub struct Services;

impl Component for Services {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <>
                { page_hero(
                    "Services",
                    "Real estate services, quietly handled.",
                    Some("Thoughtful advisory, research, coordination, and property support for owners, buyers, and clients across Culebra."),
                    "/images/coastline.png",
                    "Aerial view of Culebra coastline and turquoise Caribbean water",
                ) }
                { self.intro() }
                { self.cards() }
                { self.process() }
                { self.reasons() }
                { self.principles() }
                { self.closing() }
            </>
        }
    }
}
impl Services {
    /// "More than transactions." — the line the page opens its body with.
    fn intro(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#f8f6f1] px-6 py-20 md:px-12 md:py-24">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mx-auto max-w-3xl text-center">
                        <h2 class="font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                            {"More than transactions."}<br />{"Thoughtful support at every step."}
                        </h2>
                        <div class="mx-auto mt-6 h-px w-12 bg-accent"></div>
                        <p class="mx-auto mt-7 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                            {"From valuations and research to coordination and marketing, our services are designed to simplify decisions, connect the right expertise, and protect your interests on Culebra."}
                        </p>
                        <p class="mt-7 text-[10px] font-medium uppercase tracking-[0.24em] text-accent">
                            {"Local knowledge · Thoughtful coordination · Exceptional discretion"}
                        </p>
                    </div>
                </div>
            </section>
        }
    }
}

impl Services {
    /// The eight service cards, from `SERVICES`, each with its own image and its enquiry link.
    fn cards(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#f3efe8] px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mb-12 md:mb-16">
                        <p class="text-xs font-light uppercase tracking-[0.28em] text-accent">{"What we can help with"}</p>
                        <h2 class="mt-4 max-w-2xl font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                            {"Practical expertise around island property."}
                        </h2>
                    </div>
                    <div class="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
                        { for SERVICES.iter().map(|(number, title, body, cta, href, image)| html! {
                            <article class="group flex h-full flex-col overflow-hidden border border-border bg-[#fbfaf7] transition-transform duration-500 hover:-translate-y-1">
                                <div class="relative aspect-[4/2.7] overflow-hidden bg-muted">
                                    <img src={image.to_string()} alt={title.to_string()}
                                        sizes="(min-width: 1024px) 25vw, (min-width: 640px) 50vw, 100vw"
                                        class="absolute inset-0 h-full w-full object-cover transition-transform duration-700 group-hover:scale-[1.03]" />
                                </div>
                                <div class="flex flex-1 flex-col p-6">
                                    <span class="text-[10px] font-medium uppercase tracking-[0.22em] text-accent">{ *number }</span>
                                    <h3 class="mt-3 font-serif text-xl font-light leading-tight text-foreground">{ *title }</h3>
                                    <p class="mt-4 flex-1 text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                                    <a href={href.to_string()} class="group/link mt-7 inline-flex items-center gap-3 text-[10px] font-medium uppercase tracking-[0.18em] text-accent">
                                        { *cta }
                                        <span class="inline-block h-px w-6 bg-accent transition-all duration-500 group-hover/link:w-10"></span>
                                    </a>
                                </div>
                            </article>
                        }) }
                    </div>
                </div>
            </section>
        }
    }

    /// "How it works" — three numbered steps, each with its icon from the table.
    fn process(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#efebe3] px-6 py-20 md:px-12 md:py-24">
                <div class="mx-auto max-w-[1600px]">
                    <div class="text-center">
                        <h2 class="font-serif text-3xl font-light text-foreground md:text-4xl">{"How it works"}</h2>
                        <div class="mx-auto mt-5 h-px w-12 bg-accent"></div>
                    </div>
                    <div class="mx-auto mt-14 grid max-w-6xl gap-12 md:grid-cols-3 md:gap-0">
                        { for SERVICE_PROCESS.iter().map(|(number, title, body, icon_name)| html! {
                            <div class="h-full px-6 text-center md:border-r md:border-border md:px-14">
                                <div class="mx-auto flex h-14 w-14 items-center justify-center text-accent">
                                    { match icon_html(icon_name, "h-11 w-11", "1.15") {
                                        Some(svg) => svg,
                                        None => html! { <span aria-hidden="true">{"\u{2014}"}</span> },
                                    } }
                                </div>
                                <div class="mt-7 flex items-baseline justify-center gap-4">
                                    <span class="font-serif text-3xl font-light text-accent">{ *number }</span>
                                    <h3 class="font-serif text-xl font-light text-foreground">{ *title }</h3>
                                </div>
                                <p class="mx-auto mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        }
    }
}

impl Services {
    /// "Why clients come to CulebraLuxe" — the photograph beside the four reasons.
    fn reasons(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#f8f6f1]">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid items-stretch md:grid-cols-2">
                        <div class="relative min-h-[420px] overflow-hidden bg-muted md:min-h-[620px]">
                            <img src="/images/hero-villa.png" alt="Culebra property overlooking the Caribbean"
                                sizes="(min-width: 768px) 50vw, 100vw" class="absolute inset-0 h-full w-full object-cover" />
                        </div>
                        <div class="flex h-full flex-col justify-center px-6 py-16 md:px-14 md:py-20 lg:px-20">
                            <p class="text-xs font-light uppercase tracking-[0.28em] text-accent">{"Why CulebraLuxe"}</p>
                            <h2 class="mt-4 font-serif text-3xl font-light leading-[1.1] text-foreground md:text-4xl">
                                {"Why clients come to CulebraLuxe"}
                            </h2>
                            <div class="mt-9 space-y-7">
                                { for SERVICE_REASONS.iter().map(|(title, body, icon_name)| html! {
                                    <div class="flex gap-5">
                                        <div class="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-accent/60 text-accent">
                                            { match icon_html(icon_name, "h-4 w-4", "1.3") {
                                                Some(svg) => svg,
                                                None => html! { <span aria-hidden="true">{"\u{2014}"}</span> },
                                            } }
                                        </div>
                                        <div>
                                            <h3 class="text-sm font-medium text-foreground">{ *title }</h3>
                                            <p class="mt-1.5 max-w-lg text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
                                        </div>
                                    </div>
                                }) }
                            </div>
                        </div>
                    </div>
                </div>
            </section>
        }
    }

    /// The three-column strip: research, the right people, follow-through.
    fn principles(&self) -> Html {
        html! {
            <section class="border-b border-border bg-[#f2ede5] px-6 py-16 md:px-12 md:py-20">
                <div class="mx-auto max-w-[1600px]">
                    <div class="grid gap-10 md:grid-cols-3 md:gap-0">
                        { for SERVICE_PRINCIPLES.iter().map(|(icon_name, heading, body)| html! {
                            <div class="px-5 text-center md:border-r md:border-border md:px-12">
                                { match icon_html(icon_name, "mx-auto h-8 w-8", "1.2") {
                                    Some(svg) => svg,
                                    None => html! { <span class="block" aria-hidden="true">{"\u{2014}"}</span> },
                                } }
                                <p class="mt-5 text-xs font-medium uppercase tracking-[0.16em] text-foreground">{ *heading }</p>
                                <p class="mx-auto mt-3 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">{ *body }</p>
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
            <section class="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px] text-center">
                    <p class="text-xs font-light uppercase tracking-[0.28em] text-primary-foreground/60">{"Culebra · Puerto Rico"}</p>
                    <h2 class="mx-auto mt-5 max-w-3xl font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                        {"Let's begin a quiet conversation."}
                    </h2>
                    <p class="mx-auto mt-5 max-w-xl text-sm font-light leading-relaxed text-primary-foreground/70">
                        {"Tell us what you need. We'll help determine the right next step and whether CulebraLuxe can help."}
                    </p>
                    <a href="/contact" class="mt-10 inline-flex border border-primary-foreground/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] transition-colors hover:bg-primary-foreground hover:text-primary">
                        {"Start a conversation"}
                    </a>
                </div>
            </section>
        }
    }
}

