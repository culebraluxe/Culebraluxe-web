//! OPS — disconnected MVI prototype for the native document-signing desk.
//!
//! This screen is deliberately local-only. It proves the workflow and visual hierarchy before any endpoint,
//! DocumentSignService call, or persistence contract is attached.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template::{self, PANEL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeStatus {
    Waiting,
    Viewed,
    Completed,
    Declined,
    Voided,
}

impl EnvelopeStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting",
            Self::Viewed => "Viewed",
            Self::Completed => "Completed",
            Self::Declined => "Declined",
            Self::Voided => "Voided",
        }
    }

    fn tone(self) -> &'static str {
        match self {
            Self::Waiting => "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]",
            Self::Viewed => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
            Self::Completed => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
            Self::Declined | Self::Voided => {
                "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Recipient {
    name: String,
    email: String,
    state: String,
    activity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Envelope {
    id: String,
    form_type: String,
    client: String,
    property: String,
    status: EnvelopeStatus,
    progress: String,
    sent: String,
    expires: String,
    original_pdf: String,
    signed_pdf: Option<String>,
    recipients: Vec<Recipient>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    envelopes: Vec<Envelope>,
    selected_id: String,
    notice: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        let envelopes = vec![
            Envelope {
                id: "env-listing-001".into(),
                form_type: "Listing Contract".into(),
                client: "María Rivera".into(),
                property: "Casa Luar".into(),
                status: EnvelopeStatus::Viewed,
                progress: "1 / 2".into(),
                sent: "Today · 2:05 PM".into(),
                expires: "Oct 8".into(),
                original_pdf: "Casa-Luar-Listing-Agreement.pdf".into(),
                signed_pdf: None,
                recipients: vec![
                    Recipient {
                        name: "María Rivera".into(),
                        email: "maria@example.com".into(),
                        state: "Signed".into(),
                        activity: "Today · 3:42 PM".into(),
                    },
                    Recipient {
                        name: "Carlos Rivera".into(),
                        email: "carlos@example.com".into(),
                        state: "Waiting".into(),
                        activity: "Viewed · 3:51 PM".into(),
                    },
                ],
            },
            Envelope {
                id: "env-offer-002".into(),
                form_type: "Offer Letter".into(),
                client: "James Lee".into(),
                property: "Zoni Bluff".into(),
                status: EnvelopeStatus::Waiting,
                progress: "0 / 1".into(),
                sent: "Today · 11:18 AM".into(),
                expires: "Oct 8".into(),
                original_pdf: "Zoni-Bluff-Offer.pdf".into(),
                signed_pdf: None,
                recipients: vec![Recipient {
                    name: "James Lee".into(),
                    email: "james@example.com".into(),
                    state: "Waiting".into(),
                    activity: "Sent · 11:18 AM".into(),
                }],
            },
            Envelope {
                id: "env-ps-003".into(),
                form_type: "Purchase & Sale".into(),
                client: "Ana Pérez / Luis Soto".into(),
                property: "Alturas de Zoni".into(),
                status: EnvelopeStatus::Completed,
                progress: "2 / 2".into(),
                sent: "Sep 29 · 4:12 PM".into(),
                expires: "Complete".into(),
                original_pdf: "Alturas-Purchase-Sale.pdf".into(),
                signed_pdf: Some("Alturas-Purchase-Sale-SIGNED.pdf".into()),
                recipients: vec![
                    Recipient {
                        name: "Ana Pérez".into(),
                        email: "ana@example.com".into(),
                        state: "Signed".into(),
                        activity: "Sep 29 · 5:03 PM".into(),
                    },
                    Recipient {
                        name: "Luis Soto".into(),
                        email: "luis@example.com".into(),
                        state: "Signed".into(),
                        activity: "Sep 29 · 5:19 PM".into(),
                    },
                ],
            },
            Envelope {
                id: "env-showing-004".into(),
                form_type: "Showing Report".into(),
                client: "Thomas Grant".into(),
                property: "Villa Mar".into(),
                status: EnvelopeStatus::Declined,
                progress: "0 / 1".into(),
                sent: "Sep 28 · 9:30 AM".into(),
                expires: "Closed".into(),
                original_pdf: "Villa-Mar-Showing-Report.pdf".into(),
                signed_pdf: None,
                recipients: vec![Recipient {
                    name: "Thomas Grant".into(),
                    email: "thomas@example.com".into(),
                    state: "Declined".into(),
                    activity: "Sep 28 · 9:44 AM".into(),
                }],
            },
        ];

        Self {
            selected_id: envelopes[0].id.clone(),
            envelopes,
            notice: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Select(String),
    Resend,
    Void,
    ClearNotice,
}

pub struct DocumentSigning;

impl Screen for DocumentSigning {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (Model::default(), Cmd::none())
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Select(id) => {
                model.selected_id = id;
                model.notice = None;
            }
            Msg::Resend => {
                model.notice = Some("Demo only · invitation would be re-sent here.".into());
            }
            Msg::Void => {
                if let Some(envelope) = model
                    .envelopes
                    .iter_mut()
                    .find(|envelope| envelope.id == model.selected_id)
                {
                    envelope.status = EnvelopeStatus::Voided;
                    envelope.expires = "Voided".into();
                }
                model.notice = Some("Demo only · envelope marked void in local MVI state.".into());
            }
            Msg::ClearNotice => model.notice = None,
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let selected = model
            .envelopes
            .iter()
            .find(|envelope| envelope.id == model.selected_id)
            .or_else(|| model.envelopes.first());

        html! {
            <div class="space-y-5">
                { template::portal_heading(
                    "Operations",
                    "Document Signing",
                    "Send, follow and finish the four CulebraLuxe forms. This prototype is disconnected MVI with fake data only.",
                ) }

                { metrics(model) }

                if let Some(notice) = model.notice.as_deref() {
                    <div class="flex items-center justify-between gap-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/75 px-4 py-2.5 text-xs font-light text-[var(--portal-navy)]">
                        <span>{ notice }</span>
                        <button type="button" onclick={link.callback(|_: MouseEvent| Msg::ClearNotice)} class="text-black/40 hover:text-black/70">{"×"}</button>
                    </div>
                }

                <div class="grid min-h-[34rem] gap-4 xl:grid-cols-[minmax(0,1.35fr)_minmax(22rem,0.65fr)]">
                    <section class={classes!(PANEL, "overflow-hidden")}>
                        <div class="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)_90px_90px] gap-3 border-b border-[var(--portal-panel-border)] px-4 py-2 text-[9px] font-medium uppercase tracking-[0.15em] text-black/35">
                            <span>{"Document"}</span>
                            <span>{"Client"}</span>
                            <span>{"Status"}</span>
                            <span class="text-right">{"Progress"}</span>
                        </div>
                        { for model.envelopes.iter().map(|envelope| envelope_row(envelope, envelope.id == model.selected_id, link)) }
                    </section>

                    { selected.map(|envelope| detail(envelope, link)).unwrap_or_default() }
                </div>
            </div>
        }
    }
}

fn metrics(model: &Model) -> Html {
    let waiting = model
        .envelopes
        .iter()
        .filter(|row| matches!(row.status, EnvelopeStatus::Waiting | EnvelopeStatus::Viewed))
        .count();
    let completed = model
        .envelopes
        .iter()
        .filter(|row| row.status == EnvelopeStatus::Completed)
        .count();
    let attention = model
        .envelopes
        .iter()
        .filter(|row| {
            matches!(
                row.status,
                EnvelopeStatus::Declined | EnvelopeStatus::Voided
            )
        })
        .count();

    html! {
        <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            { template::metric("Out for signature", &waiting.to_string(), "Waiting or viewed") }
            { template::metric("Completed", &completed.to_string(), "Signed documents") }
            { template::metric("Needs attention", &attention.to_string(), "Declined or voided") }
            { template::metric("Forms supported", "4", "Listing · Offer · P&S · Showing") }
        </div>
    }
}

fn envelope_row(envelope: &Envelope, selected: bool, link: &Link<Msg>) -> Html {
    let id = envelope.id.clone();
    let onclick = link.callback(move |_: MouseEvent| Msg::Select(id.clone()));
    html! {
        <button
            type="button"
            {onclick}
            class={classes!(
                "grid", "w-full", "grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)_90px_90px]", "gap-3",
                "border-b", "border-[var(--portal-border)]", "px-4", "py-3", "text-left", "last:border-b-0", "transition",
                if selected { "bg-[var(--portal-blue-pale)]/65" } else { "hover:bg-white/45" }
            )}
        >
            <span class="min-w-0">
                <span class="block truncate font-serif text-[15px] font-light text-[var(--portal-navy)]">{ envelope.form_type.clone() }</span>
                <span class="mt-0.5 block truncate text-[10px] font-light text-black/40">{ envelope.property.clone() }</span>
            </span>
            <span class="min-w-0 self-center truncate text-xs font-light text-black/65">{ envelope.client.clone() }</span>
            <span class={classes!("self-center", "justify-self-start", "rounded-full", "px-2.5", "py-1", "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]", envelope.status.tone())}>
                { envelope.status.label() }
            </span>
            <span class="self-center text-right font-mono text-[11px] text-black/45">{ envelope.progress.clone() }</span>
        </button>
    }
}

fn detail(envelope: &Envelope, link: &Link<Msg>) -> Html {
    html! {
        <aside class={classes!(PANEL, "flex", "min-h-0", "flex-col", "overflow-hidden")}>
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{"Envelope"}</p>
                <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{ envelope.form_type.clone() }</h2>
                <p class="mt-1 text-xs font-light text-black/45">{ format!("{} · {}", envelope.client, envelope.property) }</p>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
                <dl class="grid grid-cols-2 gap-x-4 gap-y-3 text-xs">
                    <div><dt class="text-[9px] uppercase tracking-[0.12em] text-black/35">{"Sent"}</dt><dd class="mt-0.5 text-black/65">{ envelope.sent.clone() }</dd></div>
                    <div><dt class="text-[9px] uppercase tracking-[0.12em] text-black/35">{"Expires"}</dt><dd class="mt-0.5 text-black/65">{ envelope.expires.clone() }</dd></div>
                </dl>

                <div class="mt-5">
                    <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-black/35">{"Recipients"}</p>
                    <div class="mt-2 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-border)] bg-white/55">
                        { for envelope.recipients.iter().map(|recipient| html! {
                            <div class="border-b border-[var(--portal-border)] px-3 py-2.5 last:border-b-0">
                                <div class="flex items-center justify-between gap-3">
                                    <div class="min-w-0">
                                        <p class="truncate text-sm font-medium text-[var(--portal-navy)]">{ recipient.name.clone() }</p>
                                        <p class="truncate text-[10px] font-light text-black/40">{ recipient.email.clone() }</p>
                                    </div>
                                    <span class="shrink-0 text-[10px] font-medium uppercase tracking-[0.1em] text-black/50">{ recipient.state.clone() }</span>
                                </div>
                                <p class="mt-1 text-[10px] font-light text-black/35">{ recipient.activity.clone() }</p>
                            </div>
                        }) }
                    </div>
                </div>

                <div class="mt-5 space-y-2">
                    { document_line("Original PDF", &envelope.original_pdf) }
                    if let Some(signed) = envelope.signed_pdf.as_deref() {
                        { document_line("Signed PDF", signed) }
                    }
                </div>
            </div>

            <div class="border-t border-[var(--portal-panel-border)] p-4">
                <a
                    href="/sign/demo-seller-001"
                    class="flex w-full items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)]"
                >
                    {"Test Signer View"}
                </a>
                <div class="mt-2 grid grid-cols-2 gap-2">
                    <button type="button" onclick={link.callback(|_: MouseEvent| Msg::Resend)} class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]">
                        {"Resend"}
                    </button>
                    <button type="button" onclick={link.callback(|_: MouseEvent| Msg::Void)} class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-archive)]/25 px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-archive)]">
                        {"Void"}
                    </button>
                </div>
                <p class="mt-2 text-center text-[9px] font-light text-black/35">{"Prototype actions change local MVI state only."}</p>
            </div>
        </aside>
    }
}

fn document_line(label: &'static str, filename: &str) -> Html {
    html! {
        <div class="flex items-center justify-between gap-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-border)] bg-white/55 px-3 py-2">
            <div class="min-w-0">
                <p class="text-[9px] uppercase tracking-[0.12em] text-black/35">{ label }</p>
                <p class="truncate text-xs font-light text-[var(--portal-navy)]">{ filename.to_owned() }</p>
            </div>
            <span class="text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{"Open"}</span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prototype_is_disconnected_and_selects_locally() {
        let (mut model, cmd) = DocumentSigning::init(&ScreenCtx::default());
        assert!(
            cmd.into_requests().is_empty(),
            "the prototype must not call an endpoint"
        );
        assert_eq!(model.selected_id, "env-listing-001");

        DocumentSigning::update(
            &mut model,
            Msg::Select("env-ps-003".into()),
            &ScreenCtx::default(),
        );
        assert_eq!(model.selected_id, "env-ps-003");
    }
}
