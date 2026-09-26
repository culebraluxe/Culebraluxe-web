//! `/portal/admin/whatsapp-meta` — what Meta says about the WhatsApp Business account: the WABA, whether a token is
//! configured, the phone numbers, or the problem. Read-only: one read on open.

use yew::prelude::*;

use crate::app::api::PortalScreenPage;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalPage, PortalWhatsAppMeta, PortalWhatsAppPhone};

pub struct WhatsAppMeta;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalWhatsAppMeta>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
}

impl Screen for WhatsAppMeta {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
            },
            Cmd::request(PortalScreenPage::of("whatsapp-meta"), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        let Msg::Loaded(answer) = msg;
        model.read = Remote::from_result(answer.and_then(|page| {
            page.support
                .and_then(|support| support.whats_app_meta)
                .ok_or_else(|| ApiError::decode("The answer had no WhatsApp diagnostic in it."))
        }));
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        html! { <div>{ WhatsAppMeta.heading() }{ WhatsAppMeta.body(model) }</div> }
    }
}

/// The navy surface the live page used, with the gold eyebrow.
const PANEL: &str = "rounded-2xl border border-brand-gold/25 bg-white/5 p-6";
const WARNING: &str = "rounded-2xl border border-amber-400/30 bg-amber-400/10 p-6";
const LABEL: &str = "text-xs uppercase tracking-wider text-brand-ivory/50";

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
    fn body(&self, model: &Model) -> Html {
        if let Remote::Failed(error) = &model.read {
            return template::failure(error);
        }
        let Some(read) = model.read.loaded().cloned() else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_answer_decodes() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = WhatsAppMeta::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(
            request.path,
            "/api/portal/rust-ui/page?screen=whatsapp-meta"
        );
        let answer: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/portal-page-whatsapp-meta.json"
        ))
        .unwrap();
        WhatsAppMeta::update(&mut model, request.respond(Ok(answer)), &ctx);
        assert!(model
            .read
            .loaded()
            .is_some_and(|read| read.token_configured));
    }
}
