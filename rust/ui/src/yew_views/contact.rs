//! `/contact` — how to reach the office, on Yew.
//!
//! THE LIVE PAGE'S FORM IS NOT HERE, AND THAT IS THE HONEST PORT OF IT. `components/contact.tsx` held a form whose
//! submission state was React state and whose submit called a server action — and on the live site the request it made
//! never completed. Reproducing it would mean either an island holding React state (the thing this port is removing from
//! public pages) or a POST endpoint for a browser form to target, which is new server code rather than ported markup.
//! Neither is markup, so this renders what the page genuinely has: the hero from the content store, the section's own
//! copy, and the office and email the content store carries — the email as the `mailto:` link it already was. A form that
//! appears to send and does not is worse than a page that says how to reach someone.
//!
//! WHEN THE FORM COMES BACK it needs one thing first: an endpoint a plain `<form method="post">` can target, with the
//! honeypot, the request type, the property context and the submission id it already had. Then the fields, the interest
//! choice and the sent/failed states are Rust markup, because a form submission is a page load and a page load can carry
//! its outcome in the URL.

use yew::prelude::*;

use crate::model::{BlockItem, PageContent};
use crate::yew_views::buyers::page_hero;
use crate::yew_views::chrome::PageProps;

pub struct Contact;

impl Component for Contact {
    type Message = ();
    type Properties = PageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // No page, no page furniture: a hero with no words over a section with no address is not the Contact page.
        let Some(page) = ctx.props().model.page.as_ref() else {
            return Html::default();
        };
        let intro = page.hero.body.as_str();
        html! {
            <>
                { page_hero(
                    &page.hero.eyebrow,
                    &page.hero.title,
                    (!intro.is_empty()).then_some(intro),
                    page.hero.image_path.as_deref().unwrap_or("/images/coastline.png"),
                    page.hero.image_alt.as_deref().unwrap_or("The Culebra coastline at golden hour"),
                ) }
                <section id="contact" class="bg-primary px-6 py-28 text-primary-foreground md:px-12 md:py-40">
                    <div class="mx-auto max-w-[1600px]">
                        <div class="max-w-3xl">
                            <p class="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
                                { page.contact.eyebrow.clone() }
                            </p>
                            <h2 class="text-balance font-serif text-4xl font-light leading-[1.06] md:text-6xl">
                                { page.contact.title.clone() }
                            </h2>
                            { self.details(page) }
                        </div>
                    </div>
                </section>
            </>
        }
    }
}

impl Contact {
    /// ONE COLUMN, NOT A 12-COLUMN GRID WITH AN EMPTY HALF. The live section put the copy on the left and the form on
    /// the right; with no form there is no second column, and a two-column grid holding one column is a gap a visitor
    /// reads as something that failed to load.
    fn details(&self, page: &PageContent) -> Html {
        // The block's items are typed, not positional: `office` and `email` are the two the live page read, by key. A
        // missing one renders nothing rather than a label with no value under it.
        let item = |key: &str| page.contact.items.iter().find(|item| item.key == key);
        let office = item("office")
            .map(|item| address_block(item, false))
            .into_iter()
            .collect::<Vec<_>>();
        let email = item("email")
            .map(|item| address_block(item, true))
            .into_iter()
            .collect::<Vec<_>>();
        if office.is_empty() && email.is_empty() {
            return Html::default();
        }
        html! {
            <div class="mt-14 flex max-w-xl flex-col gap-8 border-t border-primary-foreground/10 pt-10">
                { for office }
                { for email }
            </div>
        }
    }
}

/// One way to reach the office: its label, and either the value or — for an email — the `mailto:` link it already was.
fn address_block(item: &BlockItem, link: bool) -> Html {
    let label = "text-xs font-light uppercase tracking-[0.2em] text-primary-foreground/45";
    let value = "mt-2 text-sm font-light text-primary-foreground/85";
    let name = item.label.clone().unwrap_or_default();
    let address = item.value.clone().unwrap_or_default();
    html! {
        <div>
            <p class={label}>{ name }</p>
            if link {
                <a href={format!("mailto:{address}")}
                    class={classes!(value, "transition-colors", "hover:text-primary-foreground")}>
                    { address }
                </a>
            } else {
                <p class={value}>{ address }</p>
            }
        </div>
    }
}
