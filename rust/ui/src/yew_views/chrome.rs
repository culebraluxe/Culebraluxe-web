//! The public chrome, reproduced from the design rather than re-drawn.
//!
//! Every class, the link order, the logo, the navy bar with its gold hairline, the `top-nav-capsule` material, the
//! breakpoints and the mobile `<details>` menu are the ones `components/site-header.tsx` uses and `app/globals.css`
//! styles. Nothing here is a redesign: the active destination is still marked with `aria-current="page"`, because that is
//! the attribute `.top-nav-capsule[aria-current='page']` matches.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::yew_router::Route;

pub const CAPSULE: &str = "top-nav-capsule top-nav-capsule--tight";
pub const MOBILE_CAPSULE: &str = "top-nav-capsule top-nav-capsule--full";

/// A link the router owns: a real anchor, in the design's own markup, that navigates without a page load.
///
/// WHY NOT `yew_router`'s `Link`: it takes `classes` and `to` and nothing else, so it cannot carry `aria-current` — and
/// the active destination is styled by that attribute in the existing stylesheet, which this port is not allowed to
/// change. This renders the same `<a>` with the same class, adds the attribute, and pushes the route instead of
/// reloading the document. Modified clicks (a new tab, a download) are left to the browser.
#[derive(Properties, PartialEq)]
pub struct NavLinkProps {
    pub to: Route,
    pub classes: Classes,
    #[prop_or_default]
    pub aria_label: Option<AttrValue>,
    #[prop_or_default]
    pub current: bool,
    #[prop_or_default]
    pub children: Html,
}

#[function_component(NavLink)]
pub fn nav_link(props: &NavLinkProps) -> Html {
    let navigator = use_navigator();
    let to = props.to;
    let onclick = Callback::from(move |event: MouseEvent| {
        if event.meta_key() || event.ctrl_key() || event.shift_key() || event.alt_key() {
            return;
        }
        if let Some(navigator) = navigator.clone() {
            event.prevent_default();
            let _ = navigator.push(&to);
        }
    });
    html! {
        <a
            href={to.to_path()}
            class={props.classes.clone()}
            aria-current={props.current.then_some("page")}
            aria-label={props.aria_label.clone()}
            {onclick}
        >
            { props.children.clone() }
        </a>
    }
}


/// The header: the logo, the menu in the design's order, and the mobile menu that is a `<details>` element.
pub struct Header;

impl Component for Header {
    type Message = ();
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // The current route, from the router's own scope: no prop threading, and no second copy of the URL.
        let current = ctx.link().route::<Route>();
        let item = |route: Route, label: &'static str, class: &'static str| {
            html! {
                <NavLink to={route} classes={classes!(class)} current={current == Some(route)}>
                    { label }
                </NavLink>
            }
        };
        // EVERY OTHER DESTINATION IS AN ORDINARY ANCHOR, and that is the routing rule rather than a shortcut: this app
        // owns `/buyers` and nothing else yet, so the links that leave it must be full page loads to the runtimes that
        // still serve them. Claiming them here would put two routers on one URL.
        let outside = |href: &'static str, label: &'static str, class: &'static str| {
            html! { <a href={href} class={class}>{ label }</a> }
        };
        html! {
            <>
                <header class="fixed inset-x-0 top-0 z-50 border-b border-brand-gold/15 bg-brand-navy py-6">
                    <div class="mx-auto flex max-w-[1600px] items-center justify-between px-6 md:px-12">
                        <a href="/" aria-label="CulebraLuxe home" class="flex h-7 w-[250px] flex-none items-center">
                            <img src="/images/culebraluxe-header-logo-test.png" alt="CulebraLuxe" width="2050"
                                height="300" class="h-9 max-h-9 w-auto max-w-full flex-none object-contain" />
                        </a>
                        <nav class="hidden items-center gap-1 lg:flex" aria-label="Primary">
                            { item(Route::Buyers, "Buyers", CAPSULE) }
                            { outside("/sellers", "Sellers", CAPSULE) }
                            { outside("/services", "Services", CAPSULE) }
                            { outside("/guide", "Guide", CAPSULE) }
                            { outside("/about", "About", CAPSULE) }
                            { outside("/faq", "FAQ", CAPSULE) }
                            { outside("/contact", "Contact", CAPSULE) }
                            { outside("/portal/dashboard", "Portal", CAPSULE) }
                        </nav>
                        <details class="lg:hidden">
                            <summary class="flex cursor-pointer list-none flex-col items-end gap-1.5 text-brand-ivory [&::-webkit-details-marker]:hidden" aria-label="Menu">
                                <span class="block h-px w-6 bg-current"></span>
                                <span class="block h-px w-6 bg-current"></span>
                                <span class="block h-px w-6 bg-current"></span>
                            </summary>
                            <nav class="absolute inset-x-0 top-full flex max-h-[75svh] flex-col gap-2 overflow-y-auto border-t border-brand-gold/25 bg-brand-navy px-4 py-4 backdrop-blur-md" aria-label="Mobile">
                                { item(Route::Buyers, "Buyers", MOBILE_CAPSULE) }
                                { outside("/sellers", "Sellers", MOBILE_CAPSULE) }
                                { outside("/services", "Services", MOBILE_CAPSULE) }
                                { outside("/guide", "Guide", MOBILE_CAPSULE) }
                                { outside("/about", "About", MOBILE_CAPSULE) }
                                { outside("/faq", "FAQ", MOBILE_CAPSULE) }
                                { outside("/contact", "Contact", MOBILE_CAPSULE) }
                                { outside("/portal/dashboard", "Portal", MOBILE_CAPSULE) }
                            </nav>
                        </details>
                    </div>
                </header>
                <div class="h-[76px] flex-none shrink-0 lg:h-[92px]" aria-hidden="true"></div>
            </>
        }
    }
}

/// The footer, as `components/site-footer.tsx` renders it.
pub struct Footer;

impl Component for Footer {
    type Message = ();
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <footer class="border-t border-border px-6 py-16 md:px-12">
                <div class="mx-auto max-w-[1600px]">
                    <div class="flex flex-col gap-12 md:flex-row md:items-end md:justify-between">
                        <div>
                            <p class="font-serif text-lg font-normal uppercase tracking-[0.35em] text-foreground">{"CulebraLuxe"}</p>
                            <p class="mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
                                {"Architectural estates and beachfront residences on the island of Culebra, Puerto Rico."}
                            </p>
                        </div>
                        <nav class="flex flex-wrap gap-x-8 gap-y-3" aria-label="Footer">
                            <NavLink to={Route::Buyers} classes={classes!(FOOTER_LINK)}>{"Buyers"}</NavLink>
                            <a href="/sellers" class={FOOTER_LINK}>{"Sellers"}</a>
                            <a href="/services" class={FOOTER_LINK}>{"Services"}</a>
                            <a href="/guide" class={FOOTER_LINK}>{"Guide"}</a>
                            <a href="/about" class={FOOTER_LINK}>{"About"}</a>
                            <a href="/faq" class={FOOTER_LINK}>{"FAQ"}</a>
                            <a href="/contact" class={FOOTER_LINK}>{"Contact"}</a>
                        </nav>
                    </div>
                    <div class="mt-14 flex flex-col gap-3 border-t border-border pt-8 text-xs font-light uppercase tracking-[0.16em] text-muted-foreground md:flex-row md:justify-between">
                        <p>{"\u{00a9} CulebraLuxe. All rights reserved."}</p>
                        <p>{"Culebra \u{00b7} Puerto Rico"}</p>
                    </div>
                </div>
            </footer>
        }
    }
}

pub const FOOTER_LINK: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground transition-colors hover:text-foreground";
