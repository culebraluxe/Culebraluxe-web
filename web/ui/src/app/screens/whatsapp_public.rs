//! `/whatsapp` — the official WhatsApp Business contact page (SUPPORT; public URL kept, Meta points at it). Static: no
//! read, no messages. Ported from the old string renderer's `whatsapp_view` word for word.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};

pub struct WhatsAppPublic;

/// Nothing happens on this page, so there is no message.
#[derive(Debug, PartialEq)]
pub enum Msg {}

impl Screen for WhatsAppPublic {
    type Model = ();
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> ((), Cmd<Msg>) {
        ((), Cmd::none())
    }

    fn update(_model: &mut (), msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {}
    }

    fn view(_model: &(), _ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        html! {
            <div class="bg-[var(--brand-navy)] px-6 py-20 text-[var(--brand-ivory)] md:px-12">
                <section class="mx-auto max-w-3xl text-center">
                    <p class="mb-4 text-sm uppercase tracking-[0.3em] text-[var(--brand-gold)]">{"Official Business Contact"}</p>
                    <h1 class="font-serif text-4xl font-medium md:text-5xl">{"CulebraLuxe WhatsApp"}</h1>
                    <p class="mx-auto mt-6 max-w-xl text-base leading-7 text-[var(--brand-ivory)]/80">
                        {"Contact CulebraLuxe through our official WhatsApp Business number."}
                    </p>
                    <a href="https://wa.me/17876383333" rel="noopener"
                        class="mt-10 inline-flex rounded-full border border-[var(--brand-gold)]/60 px-8 py-4 text-lg tracking-wide transition hover:border-[var(--brand-gold)] hover:text-[var(--brand-gold)]">
                        {"+1 (787) 638-3333"}
                    </a>
                </section>
            </div>
        }
    }
}
