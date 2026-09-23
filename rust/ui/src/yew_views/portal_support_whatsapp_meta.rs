//! `/portal/admin/whatsapp-meta` — what Meta says about this deployment's WhatsApp number.
//!
//! PARITY WITH `app/portal/admin/whatsapp-meta/page.tsx` at `141df386`: the navy page, "Private diagnostic" over "WhatsApp Meta
//! IDs", the sentence that says the query happens server-side and the token never reaches the browser, the WABA ID and token
//! card, and then whichever of the four outcomes Meta gave — not configured, refused, no numbers, or the numbers.
//!
//! NOTHING HERE TALKS TO META. The call is made in the bridge, on the server, with the token that lives there. This component
//! renders the answer and has no credentials, no endpoint and no way to reach one — which is the property that matters, not
//! the layout.

use yew::prelude::*;

use crate::model::{Msg, PortalWhatsAppMeta, PortalWhatsAppPhone};
use crate::yew_views::portal_shell::PortalShell;

/// The navy surface the live page used, with the gold eyebrow.
const PANEL: &str = "rounded-2xl border border-brand-gold/25 bg-white/5 p-6";
const WARNING: &str = "rounded-2xl border border-amber-400/30 bg-amber-400/10 p-6";
const LABEL: &str = "text-xs uppercase tracking-wider text-brand-ivory/50";

#[derive(Properties, PartialEq)]
pub struct WhatsAppMetaProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct WhatsAppMeta;

impl Component for WhatsAppMeta {
    type Message = ();
    type Properties = WhatsAppMetaProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("whatsapp-meta").expect("the WhatsApp diagnostic is in the registry");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                <main class="min-h-screen bg-brand-navy px-6 py-12 text-brand-ivory">
                    <div class="mx-auto max-w-3xl space-y-8">
                        { self.heading() }
                        { self.body(&props.model) }
                    </div>
                </main>
            </PortalShell>
        }
    }
}

impl WhatsAppMeta {
    fn heading(&self) -> Html {
        html! {
            <div>
                <p class="text-xs uppercase tracking-[0.28em] text-brand-gold">{"Private diagnostic"}</p>
                <h1 class="mt-3 font-serif text-3xl">{"WhatsApp Meta IDs"}</h1>
                <p class="mt-3 text-sm text-brand-ivory/70">
                    {"This page queries Meta server-side. The access token is never sent to the browser."}
                </p>
            </div>
        }
    }
}

impl WhatsAppMeta {
    /// The four outcomes, in the order the live page presented them: the two facts, then the problem if there is one, then
    /// the numbers if there are any.
    ///
    /// A screen that has not been answered yet says so. It must not show an empty list of numbers, because "Meta returned
    /// nothing" and "we have not asked yet" are different facts and this is the screen whose job is telling them apart.
    fn body(&self, model: &crate::model::Model) -> Html {
        let read = model
            .page
            .as_ref()
            .and_then(|page| page.portal.as_ref())
            .and_then(|portal| portal.support.as_ref())
            .and_then(|support| support.whats_app_meta.clone());
        let Some(read) = read else {
            return html! {
                <section class={PANEL}>
                    <p class="text-sm">{"Asking Meta…"}</p>
                </section>
            };
        };
        html! {
            <>
                { self.facts(&read) }
                { self.problem(&read) }
                { for read.phones.iter().map(phone_card) }
            </>
        }
    }

    /// The WABA ID and whether a token is configured — the two things the reader needs before any error makes sense.
    fn facts(&self, read: &PortalWhatsAppMeta) -> Html {
        html! {
            <section class={PANEL}>
                <dl class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <dt class={LABEL}>{"WABA ID"}</dt>
                        <dd class="mt-1 font-mono text-sm">{ read.waba_id.clone() }</dd>
                    </div>
                    <div>
                        <dt class={LABEL}>{"Meta token"}</dt>
                        <dd class="mt-1 text-sm">
                            { if read.token_configured { "Configured" } else { "Missing" } }
                        </dd>
                    </div>
                </dl>
            </section>
        }
    }

    /// What went wrong, in Meta's own words where Meta gave any — and, when the token is what is missing, the one line that
    /// says what to do about it, which is what the live page said too.
    fn problem(&self, read: &PortalWhatsAppMeta) -> Html {
        let Some(error) = read.error.clone() else {
            return Html::default();
        };
        html! {
            <section class={WARNING}>
                <h2 class="font-medium">{"Meta query not available yet"}</h2>
                <p class="mt-2 text-sm text-brand-ivory/80">{ error }</p>
                if !read.token_configured {
                    <p class="mt-4 text-sm text-brand-ivory/70">
                        {"Add one Vercel Production variable named "}<code>{"WHATSAPP_ACCESS_TOKEN"}</code>{", then redeploy."}
                    </p>
                }
            </section>
        }
    }
}

/// One number, as Meta described it.
///
/// A field Meta did not return prints the live page's own placeholder for it — "Unknown" for the number, "Not returned" for
/// the id, an em dash for the rest — rather than an empty space that reads as a layout bug.
fn phone_card(phone: &PortalWhatsAppPhone) -> Html {
    html! {
        <section class={PANEL}>
            <p class="text-xs uppercase tracking-[0.22em] text-brand-gold">{"WhatsApp number"}</p>
            <h2 class="mt-2 text-2xl font-medium">
                { phone.display_phone_number.clone().unwrap_or_else(|| "Unknown".to_string()) }
            </h2>
            <div class="mt-6 grid gap-4 sm:grid-cols-2">
                <div>
                    <p class={LABEL}>{"Phone Number ID"}</p>
                    <p class="mt-1 break-all font-mono text-lg text-brand-gold">
                        { phone.id.clone().unwrap_or_else(|| "Not returned".to_string()) }
                    </p>
                </div>
                { field("Verified name", phone.verified_name.as_deref()) }
                { field("Quality", phone.quality_rating.as_deref()) }
                { field("Verification", phone.code_verification_status.as_deref()) }
            </div>
        </section>
    }
}

fn field(label: &str, value: Option<&str>) -> Html {
    html! {
        <div>
            <p class={LABEL}>{ label }</p>
            <p class="mt-1 text-sm">{ value.unwrap_or("—") }</p>
        </div>
    }
}

