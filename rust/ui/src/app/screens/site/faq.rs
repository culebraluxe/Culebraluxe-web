//! `/faq` — the questions, on Yew.
//!
//! AN ACCORDION THAT NEEDS NO ISLAND. `components/faq-accordion.tsx` held `useState<number | null>(0)` and animated a
//! panel between `grid-rows-[0fr]` and `grid-rows-[1fr]`. A `<details>` opens with no JavaScript, opens on the first
//! question (which is what the component initialised to), and the plus/minus is the same two strokes with the vertical
//! one faded while the element is open. What is not reproduced is the height transition: the answer appears at once.
//! That is the same deliberate simplification the mobile menu makes, and for the same reason — an element that needs no
//! island cannot break when one fails to arrive.
//!
//! THE QUESTIONS ARE THE CONTENT STORE'S. They are the items of the `faq.list` block keyed `faq` (label = question,
//! value = answer) — the same filter `faqEntries()` applies — and the closing call to action is that block's own
//! `subtitle`, `ctaLabel` and `ctaHref`.

use yew::prelude::*;

use crate::app::screen::ScreenCtx;
use crate::app::site::{page_hero, SitePage, SitePageSpec};

pub struct Faq;

pub type FaqPage = SitePage<Faq>;

impl SitePageSpec for Faq {
    const SCREEN: &'static str = "site-faq";
    fn view(page: &crate::model::PageContent, _ctx: &ScreenCtx) -> Html {
        Faq.render(page)
    }
}

impl Faq {
    fn render(&self, page: &crate::model::PageContent) -> Html {
        let intro = page.hero.body.as_str();
        html! {
            <>
                { page_hero(
                    &page.hero.eyebrow,
                    &page.hero.title,
                    (!intro.is_empty()).then_some(intro),
                    page.hero.image_path.as_deref().unwrap_or("/images/hero-villa.png"),
                    page.hero.image_alt.as_deref().unwrap_or("A luxury villa overlooking the Culebra coastline"),
                ) }
                <section class="px-6 py-24 md:px-12 md:py-32">
                    { self.accordion(page) }
                    { self.closing(page) }
                </section>
            </>
        }
    }
}

impl Faq {
    /// The list, or nothing at all: an empty list is not a section, it is the page talking about itself.
    fn accordion(&self, page: &crate::model::PageContent) -> Html {
        // A QUESTION IS A PAIR. An item with a question and no answer — or an answer and no question — is not a question,
        // and defaulting the missing half would render a heading with nothing under it. Both halves or it is skipped,
        // which is the rule `faqEntries()` applies.
        let questions = page
            .faq
            .items
            .iter()
            .filter(|item| item.key == "faq")
            .filter_map(|item| Some((item.label.clone()?, item.value.clone()?)))
            .collect::<Vec<_>>();
        if questions.is_empty() {
            return Html::default();
        }
        html! {
            <ul class="mx-auto max-w-3xl">
                { for questions.into_iter().enumerate().map(|(index, (question, answer))| html! {
                    <li class="reveal border-b border-border">
                        // `open` on the first one only, because that is what the live component initialised to. The
                        // shared `name` makes them one accordion - opening a question closes the last, as the live one
                        // did - and `faq-details` animates the answer open (`app/globals.css`).
                        <details class="faq-details group" name="faq" open={index == 0}>
                            <summary class="flex w-full cursor-pointer list-none items-start justify-between gap-8 py-7 text-left [&::-webkit-details-marker]:hidden">
                                <span class="font-serif text-lg font-light leading-snug text-foreground md:text-xl">{ question }</span>
                                <span class="relative mt-2 inline-block h-4 w-4 shrink-0" aria-hidden="true">
                                    <span class="absolute left-0 top-1/2 h-px w-4 -translate-y-1/2 bg-accent"></span>
                                    <span class="absolute left-1/2 top-0 h-4 w-px -translate-x-1/2 bg-accent transition-opacity duration-300 group-open:opacity-0"></span>
                                </span>
                            </summary>
                            <p class="max-w-2xl pb-7 text-sm font-light leading-relaxed text-muted-foreground">{ answer }</p>
                        </details>
                    </li>
                }) }
            </ul>
        }
    }

    /// The closing band is both halves of one idea — the line and the link under it — so it appears when either half is
    /// there and each half renders only if it has words. A rule over nothing is worse than no rule.
    fn closing(&self, page: &crate::model::PageContent) -> Html {
        let heading = page.faq.subtitle.trim();
        let label = page
            .faq
            .cta_label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty());
        if heading.is_empty() && label.is_none() {
            return Html::default();
        }
        html! {
            <div class="mx-auto mt-20 flex max-w-3xl flex-col items-start gap-6 border-t border-border pt-12">
                if !heading.is_empty() {
                    <p class="text-pretty font-serif text-2xl font-light leading-snug text-foreground">{ heading }</p>
                }
                if let Some(label) = label {
                    <a href={page.faq.cta_href.clone().unwrap_or_else(|| "/contact".to_string())}
                        class="inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]">
                        { label }
                        <span class="inline-block h-px w-10 bg-accent"></span>
                    </a>
                }
            </div>
        }
    }
}
