//! OPS desk for native document signing, wired live.
//!
//! The envelope list comes from `documentSign.list`; selecting one reads its
//! detail (`documentSign.get`) with live recipient states. Resend and void
//! go through the durable command dispatcher as the signed-in operator.

use yew::prelude::*;

use crate::app::api::{SigningDeskCommand, SigningDeskList, SigningEnvelopeGet};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template::{self, PANEL};
use crate::model::{SigningEnvelopeRecipient, SigningEnvelopeSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeStatus {
    Waiting,
    Viewed,
    Completed,
    Declined,
    Voided,
}

impl EnvelopeStatus {
    fn of(status: &str) -> Self {
        match status {
            "completed" => Self::Completed,
            "declined" => Self::Declined,
            "voided" => Self::Voided,
            "viewed" => Self::Viewed,
            _ => Self::Waiting,
        }
    }

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

#[derive(Debug, Clone, Default, PartialEq)]
struct EnvelopeDetail {
    subject: String,
    status: String,
    expires: String,
    recipients: Vec<SigningEnvelopeRecipient>,
}

fn parse_detail(value: &serde_json::Value) -> EnvelopeDetail {
    let text = |pointer: &str| {
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let recipients = value
        .pointer("/recipients")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    EnvelopeDetail {
        subject: text("/config/subject"),
        status: text("/signatureRequest/status"),
        expires: text("/config/expiresAt"),
        recipients,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    envelopes: Remote<Vec<SigningEnvelopeSummary>>,
    selected_id: Option<String>,
    detail: Remote<EnvelopeDetail>,
    notice: Option<String>,
    seq: u64,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    ListLoaded(Result<Vec<SigningEnvelopeSummary>, ApiError>),
    DetailLoaded(Result<serde_json::Value, ApiError>),
    Select(String),
    ResendRecipient(String),
    VoidEnvelope,
    ImportFields,
    Acted(Result<serde_json::Value, ApiError>),
    ClearNotice,
}

/// Clock for command ids and request stamps. Zero off the browser, where
/// only tests run this.
fn now_rfc3339() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "1970-01-01T00:00:00Z".into()
    }
}

pub struct DocumentSigning;

fn reload_list() -> Cmd<Msg> {
    Cmd::request(SigningDeskList, Msg::ListLoaded)
}

fn read_detail(id: &str) -> Cmd<Msg> {
    Cmd::request(
        SigningEnvelopeGet {
            signature_request_id: id.to_owned(),
        },
        Msg::DetailLoaded,
    )
}

impl Screen for DocumentSigning {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                envelopes: Remote::Loading,
                ..Model::default()
            },
            reload_list(),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::ListLoaded(Ok(envelopes)) => {
                if model.selected_id.is_none() {
                    model.selected_id = envelopes
                        .first()
                        .map(|row| row.signature_request_id.clone());
                }
                model.envelopes = Remote::Loaded(envelopes);
                match model.selected_id.clone() {
                    Some(id) => read_detail(&id),
                    None => Cmd::none(),
                }
            }
            Msg::ListLoaded(Err(error)) => {
                model.envelopes = Remote::Failed(error);
                Cmd::none()
            }
            Msg::DetailLoaded(Ok(value)) => {
                model.detail = Remote::Loaded(parse_detail(&value));
                Cmd::none()
            }
            Msg::DetailLoaded(Err(error)) => {
                model.detail = Remote::Failed(error);
                Cmd::none()
            }
            Msg::Select(id) => {
                model.selected_id = Some(id.clone());
                model.detail = Remote::Loading;
                model.notice = None;
                read_detail(&id)
            }
            Msg::ResendRecipient(recipient_id) => {
                let Some(id) = model.selected_id.clone() else {
                    return Cmd::none();
                };
                model.seq += 1;
                let seq = model.seq;
                Cmd::request(
                    SigningDeskCommand {
                        command_id: format!("desk-resend-{seq}"),
                        command_type: "documentSign.resend",
                        signature_request_id: id,
                        requested_at: now_rfc3339(),
                        input: serde_json::json!({ "recipientId": recipient_id }),
                    },
                    Msg::Acted,
                )
            }
            Msg::ImportFields => {
                let Some(id) = model.selected_id.clone() else {
                    return Cmd::none();
                };
                model.seq += 1;
                let seq = model.seq;
                Cmd::request(
                    SigningDeskCommand {
                        command_id: format!("desk-import-{seq}"),
                        command_type: "documentSign.importFields",
                        signature_request_id: id,
                        requested_at: now_rfc3339(),
                        input: serde_json::json!({}),
                    },
                    Msg::Acted,
                )
            }
            Msg::VoidEnvelope => {
                let Some(id) = model.selected_id.clone() else {
                    return Cmd::none();
                };
                model.seq += 1;
                let seq = model.seq;
                Cmd::request(
                    SigningDeskCommand {
                        command_id: format!("desk-void-{seq}"),
                        command_type: "documentSign.void",
                        signature_request_id: id,
                        requested_at: now_rfc3339(),
                        input: serde_json::json!({}),
                    },
                    Msg::Acted,
                )
            }
            Msg::Acted(Ok(body)) if crate::app::cmd::command_refusal(&body).is_some() => {
                model.notice = crate::app::cmd::command_refusal(&body);
                Cmd::none()
            }
            Msg::Acted(Ok(_)) => {
                model.notice = Some("Done — the desk is re-reading.".into());
                model.detail = Remote::Loading;
                Cmd::batch(vec![
                    reload_list(),
                    match model.selected_id.clone() {
                        Some(id) => read_detail(&id),
                        None => Cmd::none(),
                    },
                ])
            }
            Msg::Acted(Err(error)) => {
                model.notice = Some(error.message.clone());
                Cmd::none()
            }
            Msg::ClearNotice => {
                model.notice = None;
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        html! {
            <div class="space-y-5">
                { template::portal_heading(
                    "Operations",
                    "Document Signing",
                    "Send, follow and finish native signing envelopes.",
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
                        { list_body(model, link) }
                    </section>

                    { detail_body(model, link) }
                </div>
            </div>
        }
    }
}

fn list_body(model: &Model, link: &Link<Msg>) -> Html {
    match &model.envelopes {
        Remote::Loading | Remote::NotAsked => template::loading_line("envelopes"),
        Remote::Failed(error) => template::failure(error),
        Remote::Loaded(rows) if rows.is_empty() => {
            template::empty_panel("No native envelopes yet. Prepare one to begin.")
        }
        Remote::Loaded(rows) => html! {
            { for rows.iter().map(|row| {
                let selected = model.selected_id.as_deref() == Some(row.signature_request_id.as_str());
                envelope_row(row, selected, link)
            }) }
        },
    }
}

fn metrics(model: &Model) -> Html {
    let (waiting, completed, attention) = match &model.envelopes {
        Remote::Loaded(rows) => {
            let waiting = rows
                .iter()
                .filter(|row| {
                    matches!(
                        EnvelopeStatus::of(&row.status),
                        EnvelopeStatus::Waiting | EnvelopeStatus::Viewed
                    )
                })
                .count();
            let completed = rows
                .iter()
                .filter(|row| EnvelopeStatus::of(&row.status) == EnvelopeStatus::Completed)
                .count();
            let attention = rows
                .iter()
                .filter(|row| {
                    matches!(
                        EnvelopeStatus::of(&row.status),
                        EnvelopeStatus::Declined | EnvelopeStatus::Voided
                    )
                })
                .count();
            (waiting, completed, attention)
        }
        _ => (0, 0, 0),
    };
    html! {
        <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            { template::metric("Out for signature", &waiting.to_string(), "Waiting or viewed") }
            { template::metric("Completed", &completed.to_string(), "Signed documents") }
            { template::metric("Needs attention", &attention.to_string(), "Declined or voided") }
            { template::metric("Recipients", &recipient_count(model).to_string(), "Across envelopes") }
        </div>
    }
}

fn recipient_count(model: &Model) -> usize {
    match &model.envelopes {
        Remote::Loaded(rows) => rows.iter().map(|row| row.recipient_total as usize).sum(),
        _ => 0,
    }
}

fn envelope_row(
    row: &crate::model::SigningEnvelopeSummary,
    selected: bool,
    link: &Link<Msg>,
) -> Html {
    let id = row.signature_request_id.clone();
    let onclick = link.callback(move |_: MouseEvent| Msg::Select(id.clone()));
    let status = EnvelopeStatus::of(&row.status);
    let title = row
        .subject
        .clone()
        .unwrap_or_else(|| short_id(&row.signature_request_id));
    let client = row.client_name.clone().unwrap_or_default();
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
                <span class="block truncate font-serif text-[15px] font-light text-[var(--portal-navy)]">{ title }</span>
                <span class="mt-0.5 block truncate text-[10px] font-light text-black/40">{ short_id(&row.signature_request_id) }</span>
            </span>
            <span class="min-w-0 self-center truncate text-xs font-light text-black/65">{ client }</span>
            <span class={classes!("self-center", "justify-self-start", "rounded-full", "px-2.5", "py-1", "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]", status.tone())}>
                { status.label() }
            </span>
            <span class="self-center text-right font-mono text-[11px] text-black/45">{ format!("{} / {}", row.completed_total, row.recipient_total) }</span>
        </button>
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn detail_body(model: &Model, link: &Link<Msg>) -> Html {
    match &model.detail {
        Remote::Loading | Remote::NotAsked => html! {
            <aside class={classes!(PANEL, "p-5")}>
                { template::loading_line("the envelope") }
            </aside>
        },
        Remote::Failed(error) => html! {
            <aside class={classes!(PANEL, "p-5")}>
                { template::failure(error) }
            </aside>
        },
        Remote::Loaded(detail) => detail_panel(detail, link),
    }
}

fn detail_panel(detail: &EnvelopeDetail, link: &Link<Msg>) -> Html {
    let status = EnvelopeStatus::of(&detail.status);
    html! {
        <aside class={classes!(PANEL, "flex", "min-h-0", "flex-col", "overflow-hidden")}>
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{"Envelope"}</p>
                <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{ detail.subject.clone() }</h2>
                <p class="mt-1 text-xs font-light text-black/45">
                    <span class={classes!("rounded-full", "px-2", "py-0.5", "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]", status.tone())}>{ status.label() }</span>
                    { detail.expires.clone() }
                </p>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
                <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-black/35">{"Recipients"}</p>
                <div class="mt-2 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-border)] bg-white/55">
                    { for detail.recipients.iter().map(|recipient| {
                        let resend = {
                            let id = recipient.id.clone();
                            link.callback(move |_: MouseEvent| Msg::ResendRecipient(id.clone()))
                        };
                        html! {
                            <div class="border-b border-[var(--portal-border)] px-3 py-2.5 last:border-b-0">
                                <div class="flex items-center justify-between gap-3">
                                    <div class="min-w-0">
                                        <p class="truncate text-sm font-medium text-[var(--portal-navy)]">{ recipient.name.clone() }</p>
                                        <p class="truncate text-[10px] font-light text-black/40">{ recipient.email.clone() }</p>
                                    </div>
                                    <span class="shrink-0 text-[10px] font-medium uppercase tracking-[0.1em] text-black/50">{ recipient.state.clone().unwrap_or_else(|| "—".into()) }</span>
                                </div>
                                <div class="mt-2">
                                    <button type="button" onclick={resend}
                                        class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 py-1.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]">
                                        {"Resend"}
                                    </button>
                                </div>
                            </div>
                        }
                    }) }
                </div>
            </div>

            <div class="border-t border-[var(--portal-panel-border)] p-4">
                <button type="button" onclick={link.callback(|_: MouseEvent| Msg::ImportFields)}
                    class="flex w-full items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:bg-white/60">
                    {"Import fields from template"}
                </button>
                <button type="button" onclick={link.callback(|_: MouseEvent| Msg::VoidEnvelope)}
                    class="mt-2 flex w-full items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-archive)]/25 px-3 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-archive)]">
                    {"Void envelope"}
                </button>
            </div>
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desk_reads_live_and_selects_detail() {
        let (mut model, cmd) = DocumentSigning::init(&ScreenCtx::default());
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/v1/services/dispatch");

        let rows = vec![crate::model::SigningEnvelopeSummary {
            signature_request_id: "req-1".into(),
            transaction_document_id: "doc-1".into(),
            subject: Some("Listing Contract".into()),
            client_name: Some("Ada".into()),
            signing_mode: "sequential".into(),
            status: "sent".into(),
            issued_at: None,
            expires_at: None,
            recipient_total: 2,
            completed_total: 1,
        }];
        let cmd =
            DocumentSigning::update(&mut model, Msg::ListLoaded(Ok(rows)), &ScreenCtx::default());
        // First row auto-selects and reads its detail.
        assert_eq!(model.selected_id.as_deref(), Some("req-1"));
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/v1/services/dispatch");

        // Actions address the durable dispatcher with verified ids.
        model.seq = 7;
        let cmd = DocumentSigning::update(&mut model, Msg::VoidEnvelope, &ScreenCtx::default());
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/v1/commands/dispatch");
    }

    #[test]
    fn import_sends_the_envelope_without_geometry() {
        let (mut model, _) = DocumentSigning::init(&ScreenCtx::default());
        model.selected_id = Some("req-9".into());
        model.seq = 3;
        let cmd = DocumentSigning::update(&mut model, Msg::ImportFields, &ScreenCtx::default());
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/v1/commands/dispatch");
    }

    #[test]
    fn status_buckets_match_the_metrics() {
        assert_eq!(EnvelopeStatus::of("completed"), EnvelopeStatus::Completed);
        assert_eq!(EnvelopeStatus::of("bogus"), EnvelopeStatus::Waiting);
    }
}
