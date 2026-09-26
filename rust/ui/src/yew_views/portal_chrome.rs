//! The portal chrome, in Rust.
//!
//! This is the Yew replacement for `components/portal/operating-shell.tsx`. It draws the same two bars the React shell
//! draws, in the same order and with the same classes, because `app/globals.css` styles them and this port is not
//! allowed to change the stylesheet:
//!
//!   * the navy top nav — the logo, then the operating worlds, then Sign out. The logo goes to the public site.
//!   * the glass rail — the listed screens of the ACTIVE surface only, horizontally scrollable on narrow screens.
//!
//! THE ACTIVE SURFACE COMES FROM THE MODEL, NOT THE URL. The React shell derived it from the pathname by longest-prefix
//! match because it had nothing better; the model already knows which screen is mounted and which surface that screen
//! belongs to, so there is no second source of truth to drift.
//!
//! THE PER-ITEM FILTERING IS HERE, in `crate::navigation`: the bar offers the worlds and the rail entries this actor
//! holds the authority and entitlement for, from the same table the registry is. It reads the actor the page wrote into
//! `#rust-actor` (`shell::adopt_actor`). This is UI VISIBILITY and not a gate — `app/portal/layout.tsx` is the
//! server-side boundary and every route re-checks.
//!
//! ONE THING THE REACT SHELL DID IS NOT HERE: the command palette. It was a client-side overlay over client and deal
//! labels fetched by the server.
//!
//! `PortalShell` (the other file) is the INNER wrapper each screen body uses for its loading and error states. This is
//! the OUTER frame. They are different things and both are needed.

use yew::prelude::*;

use crate::model::Screen;

#[derive(Properties, PartialEq)]
pub struct PortalChromeProps {
    /// The mounted screen. Its `surface` selects the rail; its `key` marks the active tab.
    pub screen: Screen,
    #[prop_or_default]
    pub children: Html,
}

#[function_component(PortalChrome)]
pub fn portal_chrome(props: &PortalChromeProps) -> Html {
    let active = props.screen.surface;
    // The actor the page handed over decides what this bar offers. See `crate::navigation`.
    let actor = crate::navigation::actor();
    let worlds = crate::navigation::visible_surfaces(&actor);
    let rail = crate::navigation::visible_items(active, &actor);

    html! {
        <div class="portal-page">
            <header class="sticky top-0 z-20">
                <div class="border-b border-[var(--portal-gold)]/25 bg-brand-navy text-white">
                    <div class="flex min-h-12 items-stretch gap-3 px-3 sm:px-6 lg:px-10">
                        <a
                            href="/"
                            aria-label="CulebraLuxe home — public site"
                            class="me-4 flex flex-none items-center py-2 lg:me-20"
                        >
                            <img
                                src="/images/culebraluxe-header-logo-test.png"
                                alt="CulebraLuxe"
                                class="h-7 w-auto max-w-[160px] flex-none object-contain sm:h-8 sm:max-w-[200px]"
                            />
                        </a>

                        <nav aria-label="Operating surface" class="portal-top-nav">
                            { for worlds.iter().map(|surface| {
                                let current = *surface == active;
                                html! {
                                    <a
                                        href={crate::navigation::home_path(*surface)}
                                        aria-current={current.then_some("page")}
                                        class="top-nav-capsule"
                                    >
                                        { surface.label() }
                                    </a>
                                }
                            })}
                        </nav>

                        <div class="flex flex-none items-center gap-3 lg:ps-6">
                            <a
                                href="/api/auth/signout"
                                class="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-ivory)]/60 transition hover:text-[var(--portal-ivory)]"
                            >
                                { "Sign out" }
                            </a>
                        </div>
                    </div>
                </div>

                <div class="portal-glass-nav">
                    <div class="overflow-x-auto px-3 py-2 sm:px-6 lg:px-10">
                        <nav
                            aria-label={format!("{} navigation", active.label())}
                            class="portal-glass-rail"
                        >
                            { for rail.iter().map(|item| {
                                let current = item.href == props.screen.path;
                                html! {
                                    <a
                                        href={item.href}
                                        aria-current={current.then_some("page")}
                                        class="portal-glass-tab"
                                    >
                                        { item.label }
                                    </a>
                                }
                            })}
                        </nav>
                    </div>
                </div>
            </header>

            <main class="portal-shell-main px-3 py-4 sm:px-6 lg:px-10 lg:py-5">
                { props.children.clone() }
            </main>
        </div>
    }
}
