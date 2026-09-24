//! `/favorites` — saved properties, on Yew. The port of `components/property/favorites-view.tsx`.
//!
//! SAVED LIVES ON THE DEVICE. The heart on a card writes the browser's favorites store; this page reads it
//! (`model.saved_listings`) and shows the published listings it names, in the order they were saved. The payload is the
//! published inventory, so a saved property that has since left the market cannot appear. Each card is the buyers
//! inventory card, and its filled heart removes it from here.

use yew::prelude::*;

use crate::model::Listing;
use crate::yew_views::buyers::card;
use crate::yew_views::chrome::PageProps;
use crate::yew_views::contact::{quick_enquiry, QuickEnquiry};

pub struct Favorites;

/// The lead's message: each saved property by name, price and link, so the team can answer without looking it up.
fn shortlist_message(saved: &[&Listing]) -> String {
    let lines = saved
        .iter()
        .map(|listing| {
            format!(
                "- {} ({}) /properties/{}",
                listing.name,
                crate::format::listing_price_label(listing),
                listing.slug
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Please send me details on my saved properties:\n{lines}")
}

impl Component for Favorites {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let model = &ctx.props().model;
        let on_msg = &ctx.props().on_msg;
        let Some(page) = model.page.as_ref() else {
            return Html::default();
        };
        let saved = model
            .saved_listings
            .iter()
            .filter_map(|id| page.listings.iter().find(|listing| &listing.id == id))
            .collect::<Vec<&Listing>>();
        html! {
            <section class="px-6 py-16 md:px-12 md:py-20">
                <div class="mx-auto max-w-[1600px]">
                    <p class="mb-3 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Saved"}</p>
                    <h1 class="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">{"Saved properties"}</h1>
                    <p class="mt-4 max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                        {"Properties you have saved on this device for a closer look."}
                    </p>
                    if saved.is_empty() {
                        <div class="mt-12 border-t border-border pt-12">
                            <p class="font-serif text-2xl font-light text-foreground">{"Nothing saved yet."}</p>
                            <p class="mt-3 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                                {"Tap the heart on any property to keep it here for later."}
                            </p>
                            <a href="/buyers" class="group mt-6 inline-flex items-center gap-2 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors hover:text-accent">
                                {"Explore properties"}
                                <span class="inline-block transition-transform duration-500 group-hover:translate-x-1" aria-hidden="true">{"\u{2192}"}</span>
                            </a>
                        </div>
                    } else {
                        <div class="mt-12 grid gap-x-7 gap-y-14 md:grid-cols-2 xl:grid-cols-3">
                            { for saved.iter().map(|listing| card(listing, true, None, on_msg)) }
                        </div>
                        // The shortlist, as a lead: the team sees exactly which properties this visitor kept.
                        { quick_enquiry(model, on_msg, QuickEnquiry {
                            eyebrow: "Your shortlist",
                            title: "Send me these properties.",
                            body: "Leave your details and the CulebraLuxe team will send you the full particulars of your saved properties, and arrange viewings if you wish.",
                            button: "Send me the details",
                            sent: "Your shortlist has reached us. A member of the CulebraLuxe team will be in touch within one business day.",
                            message: shortlist_message(&saved),
                        }) }
                    }
                </div>
            </section>
        }
    }
}
