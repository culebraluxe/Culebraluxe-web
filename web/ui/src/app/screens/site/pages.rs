//! The interactive site pages as screens: each names its page read and what it needs from the device, and draws the
//! shared visitor model (`visitor.rs`) with its own view.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::site::ErrorView;

use super::visitor::{self, Model, Msg, Needs};

/// The page while it is arriving, or the interruption if it could not.
fn frame(
    model: &Model,
    link: &Link<Msg>,
    draw: impl FnOnce(&Model, &Callback<Msg>) -> Html,
) -> Html {
    if model.failed {
        return html! { <ErrorView /> };
    }
    if model.page.is_none() {
        return html! { <div class="min-h-[80svh]" data-screen-state="loading" aria-busy="true"></div> };
    }
    draw(model, &link.callback(|msg: Msg| msg))
}

macro_rules! visitor_page {
    ($name:ident, $key:literal, $needs:expr, $scope:expr, $draw:expr) => {
        pub struct $name;

        impl Screen for $name {
            type Model = Model;
            type Msg = Msg;

            fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
                let scope: fn(&ScreenCtx) -> Option<String> = $scope;
                visitor::init($key, scope(ctx), ctx, $needs)
            }

            fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
                visitor::update(model, msg)
            }

            fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
                frame(model, link, $draw)
            }
        }
    };
}

const FAVORITES: Needs = Needs {
    favorites: true,
    buyer_tools: false,
};
const BUYER: Needs = Needs {
    favorites: true,
    buyer_tools: true,
};
const NOTHING: Needs = Needs {
    favorites: false,
    buyer_tools: false,
};

visitor_page!(Home, "site-home", FAVORITES, |_| None, |model, on_msg| {
    super::home::Home.render(model, on_msg)
});
visitor_page!(Buyers, "site-buyers", BUYER, |_| None, |model, on_msg| {
    super::buyers::Buyers.render(model, on_msg)
});
visitor_page!(
    Favorites,
    "site-favorites",
    FAVORITES,
    |_| None,
    |model, on_msg| super::favorites::Favorites.render(model, on_msg)
);
visitor_page!(
    Contact,
    "site-contact",
    NOTHING,
    |ctx| ctx.query("propertyId").map(str::to_owned),
    |model, on_msg| super::contact::Contact.render(model, on_msg)
);
visitor_page!(
    PropertyDetail,
    "site-property-detail",
    FAVORITES,
    |ctx| ctx.id.clone(),
    |model, on_msg| super::property_detail::PropertyDetail.render(model, on_msg)
);

visitor_page!(
    Properties,
    "site-buyers",
    FAVORITES,
    |_| None,
    |model, on_msg| properties(model, on_msg)
);

/// `/properties` — every published listing, the homepage's "View All": the collection as cards, saveable, with the
/// Buyers page one step away for searching.
fn properties(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let listings = model
        .page
        .as_ref()
        .map(|page| page.listings.as_slice())
        .unwrap_or(&[]);
    html! {
        <>
            { crate::app::site::page_hero(
                "The Collection",
                "Every residence and parcel we represent.",
                Some("Homes, villas and land across Culebra, presented in full. Search and compare on the Buyers page."),
                "/images/hero-villa.png",
                "A modern luxury villa overlooking the Culebra coastline",
            ) }
            <section class="px-6 py-20 md:px-12 md:py-28">
                <div class="mx-auto max-w-[1600px]">
                    <div class="mb-12 flex flex-wrap items-baseline justify-between gap-4">
                        <p class="text-xs font-light uppercase tracking-[0.34em] text-muted-foreground">
                            { format!("{} {}", listings.len(), if listings.len() == 1 { "property" } else { "properties" }) }
                        </p>
                        <a href="/buyers" class="text-xs font-light uppercase tracking-[0.22em] text-foreground underline-offset-4 hover:underline">
                            {"Search & compare \u{2192}"}
                        </a>
                    </div>
                    if listings.is_empty() {
                        <p class="text-sm font-light text-muted-foreground">{"No properties are published right now."}</p>
                    } else {
                        <div class="grid gap-x-8 gap-y-16 md:grid-cols-2 xl:grid-cols-3">
                            { for listings.iter().map(|listing| super::buyers::card(
                                listing,
                                model.saved_listings.contains(&listing.id),
                                None,
                                on_msg,
                            )) }
                        </div>
                    }
                </div>
            </section>
        </>
    }
}
