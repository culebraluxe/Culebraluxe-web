//! The public chrome, reproduced from the design rather than re-drawn.
//!
//! Every class, the link order, the logo, the navy bar with its gold hairline, the `top-nav-capsule` material, the
//! breakpoints and the mobile `<details>` menu are the ones `components/site-header.tsx` uses and `app/globals.css`
//! styles. Nothing here is a redesign: the active destination is still marked with `aria-current="page"`, because that is
//! the attribute `.top-nav-capsule[aria-current='page']` matches.

use yew::prelude::*;

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
                            <crate::app::chrome::AppLink href="/buyers" classes={classes!(FOOTER_LINK)}>{"Buyers"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/sellers" classes={classes!(FOOTER_LINK)}>{"Sellers"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/services" classes={classes!(FOOTER_LINK)}>{"Services"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/guide" classes={classes!(FOOTER_LINK)}>{"Guide"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/about" classes={classes!(FOOTER_LINK)}>{"About"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/faq" classes={classes!(FOOTER_LINK)}>{"FAQ"}</crate::app::chrome::AppLink>
                            <crate::app::chrome::AppLink href="/contact" classes={classes!(FOOTER_LINK)}>{"Contact"}</crate::app::chrome::AppLink>
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
