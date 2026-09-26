//! The master chrome: the public site's header and footer, and the portal's top nav and rail. Every menu is generated
//! from the registry; no link list is written here.
//!
//! The markup and classes are the design's (`components/site-header.tsx`, `components/portal/operating-shell.tsx`),
//! styled by `app/globals.css`. The active destination is `aria-current="page"`, which is what the stylesheet matches.

use yew::prelude::*;
use yew_router::prelude::*;
use yew_router::AnyRoute;

use crate::app::registry::{self, Area};
use crate::model::Surface;

// ---- Links ---------------------------------------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct AppLinkProps {
    pub href: AttrValue,
    #[prop_or_default]
    pub classes: Classes,
    #[prop_or_default]
    pub current: bool,
    #[prop_or_default]
    pub aria_label: Option<AttrValue>,
    #[prop_or_default]
    pub children: Html,
}

/// THE ONE LINK. A real `<a href>` always (so a new tab, a copy and a crawler all work), and an in-app navigation when
/// the target is a screen this app renders in the SAME area. A link across areas (site ↔ portal) or to a page Next
/// renders is left to the browser, because the portal's server guard and actor only exist on a portal document.
#[function_component(AppLink)]
pub fn app_link(props: &AppLinkProps) -> Html {
    let navigator = use_navigator();
    let here = use_location()
        .map(|location| location.path().to_string())
        .unwrap_or_default();
    let href = props.href.to_string();
    let onclick = Callback::from(move |event: MouseEvent| {
        if event.meta_key()
            || event.ctrl_key()
            || event.shift_key()
            || event.alt_key()
            || event.button() != 0
        {
            return;
        }
        if in_app(&here, &href) {
            if let Some(navigator) = &navigator {
                event.prevent_default();
                navigator.push(&AnyRoute::new(href.clone()));
            }
        }
    });
    html! {
        <a
            href={props.href.clone()}
            class={props.classes.clone()}
            aria-current={props.current.then_some("page")}
            aria-label={props.aria_label.clone()}
            {onclick}
        >
            { props.children.clone() }
        </a>
    }
}

/// Whether following `href` from `here` stays inside this app. Pure, so the rule is tested.
pub fn in_app(here: &str, href: &str) -> bool {
    if !href.starts_with('/') || href.starts_with("//") {
        return false;
    }
    let leaving_island = registry::resolve(here).is_some_and(|(entry, _)| entry.needs_document());
    match registry::resolve(href) {
        Some((entry, _)) => {
            !leaving_island && !entry.needs_document() && entry.area() == registry::area_of(here)
        }
        None => false,
    }
}

// ---- The public site ----------------------------------------------------------------------------------------------

const CAPSULE: &str = "top-nav-capsule top-nav-capsule--tight";
const MOBILE_CAPSULE: &str = "top-nav-capsule top-nav-capsule--full";

#[derive(Properties, PartialEq)]
pub struct FrameProps {
    #[prop_or_default]
    pub children: Html,
}

/// The site: header, the screen, footer.
#[function_component(SiteFrame)]
pub fn site_frame(props: &FrameProps) -> Html {
    html! {
        <div class="flex min-h-screen flex-col bg-background text-foreground">
            <SiteHeader />
            <main class="min-w-0 flex-1">{ props.children.clone() }</main>
            <crate::yew_views::chrome::Footer />
        </div>
    }
}

#[function_component(SiteHeader)]
fn site_header() -> Html {
    let path = use_location()
        .map(|location| location.path().to_string())
        .unwrap_or_default();
    let items = |class: &'static str| -> Html {
        registry::header_items()
            .map(|(label, entry)| {
                html! {
                    <AppLink href={entry.path} classes={classes!(class)} current={registry::is_current(entry, &path)}>{ label }</AppLink>
                }
            })
            .chain(std::iter::once(html! { <a href="/portal/dashboard" class={class}>{"Portal"}</a> }))
            .collect()
    };
    html! {
        <>
            <header class="fixed inset-x-0 top-0 z-50 border-b border-brand-gold/15 bg-brand-navy py-6">
                <div class="mx-auto flex max-w-[1600px] items-center justify-between px-6 md:px-12">
                    <AppLink href="/" classes={classes!("flex", "h-7", "w-[250px]", "flex-none", "items-center")}
                        aria_label={AttrValue::Static("CulebraLuxe home")}>
                        <img src="/images/culebraluxe-header-logo-test.png" alt="CulebraLuxe" width="2050"
                            height="300" class="h-9 max-h-9 w-auto max-w-full flex-none object-contain" />
                    </AppLink>
                    <nav class="hidden items-center gap-1 lg:flex" aria-label="Primary">{ items(CAPSULE) }</nav>
                    <details class="group lg:hidden">
                        <summary class="flex cursor-pointer list-none flex-col items-end gap-1.5 text-brand-ivory [&::-webkit-details-marker]:hidden" aria-label="Menu">
                            <span class="block h-px w-6 bg-current transition-all duration-300 group-open:translate-y-[7px] group-open:rotate-45"></span>
                            <span class="block h-px w-6 bg-current transition-all duration-300 group-open:opacity-0"></span>
                            <span class="block h-px w-6 bg-current transition-all duration-300 group-open:-translate-y-[7px] group-open:-rotate-45"></span>
                        </summary>
                        <nav onclick={close_menu()} class="absolute inset-x-0 top-full flex max-h-[75svh] flex-col gap-2 overflow-y-auto border-t border-brand-gold/25 bg-brand-navy px-4 py-4 backdrop-blur-md animate-[fadeUp_0.4s_ease-out_both]" aria-label="Mobile">
                            { items(MOBILE_CAPSULE) }
                        </nav>
                    </details>
                </div>
            </header>
            <div class="h-[76px] flex-none shrink-0 lg:h-[92px]" aria-hidden="true"></div>
        </>
    }
}

/// Close the mobile `<details>` once a link in it is chosen: the page changes without a load, so nothing else would.
fn close_menu() -> Callback<MouseEvent> {
    Callback::from(|event: MouseEvent| {
        use wasm_bindgen::JsCast;
        let link = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .and_then(|element| element.closest("a").ok().flatten());
        if let Some(menu) = link.and_then(|link| link.closest("details").ok().flatten()) {
            let _ = menu.remove_attribute("open");
        }
    })
}

// ---- The portal ---------------------------------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct PortalFrameProps {
    /// The operating world the current screen belongs to: it selects the rail and marks the capsule.
    pub surface: Surface,
    #[prop_or_default]
    pub children: Html,
}

/// The portal: the navy top nav (worlds), the glass rail (the active world's screens), the screen.
#[function_component(PortalFrame)]
pub fn portal_frame(props: &PortalFrameProps) -> Html {
    let path = use_location()
        .map(|location| location.path().to_string())
        .unwrap_or_default();
    let actor = crate::navigation::actor();
    let active = props.surface;
    let worlds = registry::visible_surfaces(&actor);
    let rail = registry::rail_items(active, &actor);
    html! {
        <div class="portal-page">
            <header class="sticky top-0 z-20">
                <div class="border-b border-[var(--portal-gold)]/25 bg-brand-navy text-white">
                    <div class="flex min-h-12 items-stretch gap-3 px-3 sm:px-6 lg:px-10">
                        <a href="/" aria-label="CulebraLuxe home — public site" class="me-4 flex flex-none items-center py-2 lg:me-20">
                            <img src="/images/culebraluxe-header-logo-test.png" alt="CulebraLuxe"
                                class="h-7 w-auto max-w-[160px] flex-none object-contain sm:h-8 sm:max-w-[200px]" />
                        </a>
                        <nav aria-label="Operating surface" class="portal-top-nav">
                            { for worlds.iter().map(|surface| html! {
                                <AppLink href={crate::navigation::home_path(*surface)} classes={classes!("top-nav-capsule")}
                                    current={*surface == active}>{ surface.label() }</AppLink>
                            }) }
                        </nav>
                        <div class="flex flex-none items-center gap-3 lg:ps-6">
                            <a href="/api/auth/signout" class="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-ivory)]/60 transition hover:text-[var(--portal-ivory)]">
                                {"Sign out"}
                            </a>
                        </div>
                    </div>
                </div>
                <div class="portal-glass-nav">
                    <div class="overflow-x-auto px-3 py-2 sm:px-6 lg:px-10">
                        <nav aria-label={format!("{} navigation", active.label())} class="portal-glass-rail">
                            { for rail.iter().map(|(label, entry)| html! {
                                <AppLink href={entry.path} classes={classes!("portal-glass-tab")} current={registry::is_current(entry, &path)}>
                                    { *label }
                                </AppLink>
                            }) }
                        </nav>
                    </div>
                </div>
            </header>
            <main class="portal-shell-main px-3 py-4 sm:px-6 lg:px-10 lg:py-5">{ props.children.clone() }</main>
        </div>
    }
}

/// The area's frame around a screen. The surface only matters in the portal.
pub fn frame(area: Area, surface: Surface, content: Html) -> Html {
    match area {
        Area::Site => html! { <SiteFrame>{ content }</SiteFrame> },
        Area::Portal => html! { <PortalFrame surface={surface}>{ content }</PortalFrame> },
    }
}

#[cfg(test)]
mod tests {
    use super::in_app;

    #[test]
    fn a_link_stays_in_the_app_only_within_one_area_and_to_a_screen_this_app_renders() {
        assert!(
            in_app("/portal/dashboard", "/portal/clients"),
            "portal to portal"
        );
        assert!(in_app("/buyers", "/sellers"), "site to site");
        assert!(
            in_app("/portal/clients", "/portal/clients/abc"),
            "into a record"
        );
        assert!(
            !in_app("/buyers", "/portal/dashboard"),
            "site to portal loads the document (server guard, actor)"
        );
        assert!(
            !in_app("/portal/dashboard", "/"),
            "portal to site loads the document"
        );
        assert!(
            !in_app("/portal/dashboard", "/portal/forms"),
            "a page Next renders"
        );
        assert!(
            !in_app("/portal/dashboard", "/api/auth/signout"),
            "not a screen"
        );
        assert!(!in_app("/buyers", "https://example.com/"), "another site");
        assert!(
            !in_app("/portal/forms", "/portal/clients"),
            "from a page Next renders"
        );
        assert!(
            in_app("/portal/dashboard", "/portal/property-admin"),
            "the Workbench's video is an <Island>, so it moves in the app"
        );
        assert!(
            in_app("/portal/dashboard", "/portal/projects"),
            "a screen whose widgets are <Island>s moves in the app"
        );
    }
}
