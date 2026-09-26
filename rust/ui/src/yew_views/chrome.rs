//! The public chrome, reproduced from the design rather than re-drawn.
//!
//! Every class, the link order, the logo, the navy bar with its gold hairline, the `top-nav-capsule` material, the
//! breakpoints and the mobile `<details>` menu are the ones `components/site-header.tsx` uses and `app/globals.css`
//! styles. Nothing here is a redesign: the active destination is still marked with `aria-current="page"`, because that is
//! the attribute `.top-nav-capsule[aria-current='page']` matches.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::yew_router::Route;

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
    let to = props.to.clone();
    let click_target = to.clone();
    let onclick = Callback::from(move |event: MouseEvent| {
        if event.meta_key() || event.ctrl_key() || event.shift_key() || event.alt_key() {
            return;
        }
        if let Some(navigator) = navigator.clone() {
            event.prevent_default();
            let _ = navigator.push(&click_target);
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
                            <NavLink to={Route::Sellers} classes={classes!(FOOTER_LINK)}>{"Sellers"}</NavLink>
                            <NavLink to={Route::Services} classes={classes!(FOOTER_LINK)}>{"Services"}</NavLink>
                            <NavLink to={Route::Guide} classes={classes!(FOOTER_LINK)}>{"Guide"}</NavLink>
                            <NavLink to={Route::About} classes={classes!(FOOTER_LINK)}>{"About"}</NavLink>
                            <NavLink to={Route::Faq} classes={classes!(FOOTER_LINK)}>{"FAQ"}</NavLink>
                            <NavLink to={Route::Contact} classes={classes!(FOOTER_LINK)}>{"Contact"}</NavLink>
                        </nav>
                    </div>
                    <div class="mt-14 flex flex-col gap-3 border-t border-border pt-8 text-xs font-light uppercase tracking-[0.16em] text-muted-foreground md:flex-row md:justify-between">
                        <p>{ format!("\u{00a9} {} CulebraLuxe. All rights reserved.", js_sys::Date::new_0().get_full_year()) }</p>
                        <p>{"Culebra \u{00b7} Puerto Rico"}</p>
                    </div>
                </div>
            </footer>
        }
    }
}

pub const FOOTER_LINK: &str = "text-xs font-light uppercase tracking-[0.2em] text-muted-foreground transition-colors hover:text-foreground";

/// The props every screen component takes: the model, and the one dispatch callback that carries every intent into the
/// reducer. A screen that needs nothing else is a function of these two.
#[derive(Properties, PartialEq)]
pub struct PageProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<crate::model::Msg>,
}
