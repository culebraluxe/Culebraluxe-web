//! OPS desk for native document signing, wired live.
//!
//! The envelope list comes from `documentSign.list`; selecting one reads its
//! detail (`documentSign.get`) with live recipient states. Resend and void
//! go through the durable command dispatcher as the signed-in operator.

use yew::prelude::*;

use crate::app::api::{
    SigningDeskCommand, SigningDeskList, SigningDocumentsList, SigningEnvelopeGet,
};
use crate::app::cmd::{command_refusal, ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::screens::signing_compose::{self as compose, Draft, RecipientDraft, Stage};
use crate::app::template::{self, PANEL};
use crate::model::{SigningDocumentOption, SigningEnvelopeRecipient, SigningEnvelopeSummary};

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
    compose: Option<Compose>,
}

/// The "Send for signature" panel: the draft, the documents to pick from, and where the command chain is.
#[derive(Debug, Clone, Default, PartialEq)]
struct Compose {
    draft: Draft,
    documents: Remote<Vec<SigningDocumentOption>>,
    stage: Stage,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComposeEdit {
    Document(String),
    Subject(String),
    Message(String),
    Days(String),
    Page(String),
    Sequential(bool),
    Name(usize, String),
    Email(usize, String),
    Approver(usize, bool),
    AddPerson,
    RemovePerson(usize),
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
    OpenCompose,
    CloseCompose,
    DocumentsLoaded(Result<Vec<SigningDocumentOption>, ApiError>),
    Compose(ComposeEdit),
    SendCompose,
    ComposeStepped(Result<serde_json::Value, ApiError>),
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

fn apply_edit(draft: &mut Draft, edit: ComposeEdit) {
    match edit {
        ComposeEdit::Document(value) => draft.document_id = value,
        ComposeEdit::Subject(value) => draft.subject = value,
        ComposeEdit::Message(value) => draft.message = value,
        ComposeEdit::Days(value) => draft.expires_in_days = value,
        ComposeEdit::Page(value) => draft.signature_page = value,
        ComposeEdit::Sequential(value) => draft.sequential = value,
        ComposeEdit::Name(i, value) => {
            if let Some(person) = draft.recipients.get_mut(i) {
                person.name = value;
            }
        }
        ComposeEdit::Email(i, value) => {
            if let Some(person) = draft.recipients.get_mut(i) {
                person.email = value;
            }
        }
        ComposeEdit::Approver(i, value) => {
            if let Some(person) = draft.recipients.get_mut(i) {
                person.approver = value;
            }
        }
        ComposeEdit::AddPerson => draft.recipients.push(RecipientDraft::default()),
        ComposeEdit::RemovePerson(i) => {
            if draft.recipients.len() > 1 && i < draft.recipients.len() {
                draft.recipients.remove(i);
            }
        }
    }
}

/// The expiry instant for "N days from now". Needs the browser clock; off the browser only tests run this.
fn expiry_in_days(draft: &Draft) -> Option<String> {
    let days: f64 = draft.expires_in_days.trim().parse().ok()?;
    #[cfg(target_arch = "wasm32")]
    {
        let at = js_sys::Date::new_0();
        at.set_time(at.get_time() + days * 86_400_000.0);
        at.to_iso_string().as_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = days;
        None
    }
}

/// The send finished: on success the panel closes and the list re-reads; on a refusal the panel stays, with the
/// server's words, and nothing was written (the send is one transaction).
fn compose_stepped(model: &mut Model, result: Result<serde_json::Value, ApiError>) -> Cmd<Msg> {
    let outcome = match result {
        Ok(body) => match command_refusal(&body) {
            Some(reason) => Err(reason),
            None => Ok(body),
        },
        Err(error) => Err(error.message),
    };
    match outcome {
        Ok(_) => {
            let notice = model
                .compose
                .as_ref()
                .map(|c| compose::sent_notice(&c.draft))
                .unwrap_or_default();
            model.compose = None;
            model.notice = Some(notice);
            model.detail = Remote::Loading;
            model.selected_id = None;
            reload_list()
        }
        Err(reason) => {
            if let Some(c) = model.compose.as_mut() {
                c.error = Some(reason);
                c.stage = Stage::Idle;
            }
            Cmd::none()
        }
    }
}

/// A command id that is unique across page loads. The dispatcher treats a repeated id as a replay and answers the
/// old result, so a per-page counter alone would make the second session's "void" a no-op.
fn command_id(model: &mut Model, what: &str) -> String {
    model.seq += 1;
    format!("desk-{what}-{}-{}", now_rfc3339(), model.seq)
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
                let command_id = command_id(model, "resend");
                Cmd::request(
                    SigningDeskCommand::on_envelope(
                        command_id,
                        "documentSign.resend",
                        id,
                        now_rfc3339(),
                        serde_json::json!({ "recipientId": recipient_id }),
                    ),
                    Msg::Acted,
                )
            }
            Msg::ImportFields => {
                let Some(id) = model.selected_id.clone() else {
                    return Cmd::none();
                };
                let command_id = command_id(model, "import");
                Cmd::request(
                    SigningDeskCommand::on_envelope(
                        command_id,
                        "documentSign.importFields",
                        id,
                        now_rfc3339(),
                        serde_json::json!({}),
                    ),
                    Msg::Acted,
                )
            }
            Msg::VoidEnvelope => {
                let Some(id) = model.selected_id.clone() else {
                    return Cmd::none();
                };
                let command_id = command_id(model, "void");
                Cmd::request(
                    SigningDeskCommand::on_envelope(
                        command_id,
                        "documentSign.void",
                        id,
                        now_rfc3339(),
                        serde_json::json!({}),
                    ),
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
            Msg::OpenCompose => {
                model.notice = None;
                model.compose = Some(Compose {
                    documents: Remote::Loading,
                    ..Compose::default()
                });
                Cmd::request(SigningDocumentsList, Msg::DocumentsLoaded)
            }
            Msg::CloseCompose => {
                if model.compose.as_ref().is_some_and(|c| !c.stage.busy()) {
                    model.compose = None;
                }
                Cmd::none()
            }
            Msg::DocumentsLoaded(result) => {
                if let Some(c) = model.compose.as_mut() {
                    c.documents = match result {
                        Ok(rows) => Remote::Loaded(rows),
                        Err(error) => Remote::Failed(error),
                    };
                }
                Cmd::none()
            }
            Msg::Compose(edit) => {
                if let Some(c) = model.compose.as_mut().filter(|c| !c.stage.busy()) {
                    c.error = None;
                    apply_edit(&mut c.draft, edit);
                }
                Cmd::none()
            }
            Msg::SendCompose => {
                let Some(c) = model.compose.as_mut().filter(|c| !c.stage.busy()) else {
                    return Cmd::none();
                };
                if let Err(reason) = compose::validate(&c.draft) {
                    c.error = Some(reason);
                    return Cmd::none();
                }
                c.error = None;
                c.stage = Stage::Sending;
                let input = compose::send_input(&c.draft, expiry_in_days(&c.draft));
                let document_id = c.draft.document_id.clone();
                let id = command_id(model, "send");
                Cmd::request(
                    SigningDeskCommand::send(id, document_id, now_rfc3339(), input),
                    Msg::ComposeStepped,
                )
            }
            Msg::ComposeStepped(result) => compose_stepped(model, result),
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

                <div class="flex justify-end">
                    <button type="button" onclick={link.callback(|_: MouseEvent| Msg::OpenCompose)}
                        class="rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:opacity-90">
                        {"Send for signature"}
                    </button>
                </div>

                if let Some(compose) = model.compose.as_ref() {
                    { compose_panel(compose, link) }
                }

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

const INPUT: &str = "w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 py-2 text-sm font-light text-[var(--portal-navy)] disabled:opacity-50";
const LABEL: &str = "mb-1 block text-[9px] font-medium uppercase tracking-[0.15em] text-black/40";

fn text_input(
    link: &Link<Msg>,
    value: &str,
    placeholder: &'static str,
    disabled: bool,
    edit: impl Fn(String) -> ComposeEdit + 'static,
) -> Html {
    html! {
        <input type="text" class={INPUT} {placeholder} {disabled} value={value.to_owned()}
            oninput={link.callback(move |event: InputEvent| {
                let value = event
                    .target_dyn_into::<web_sys::HtmlInputElement>()
                    .map(|input| input.value())
                    .unwrap_or_default();
                Msg::Compose(edit(value))
            })} />
    }
}

fn compose_panel(c: &Compose, link: &Link<Msg>) -> Html {
    let busy = c.stage.busy();
    let draft = &c.draft;
    html! {
        <section class={classes!(PANEL, "space-y-4", "p-5")}>
            <div class="flex items-start justify-between gap-3">
                <div>
                    <p class="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{"New envelope"}</p>
                    <h2 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">{"Send for signature"}</h2>
                </div>
                <button type="button" disabled={busy} onclick={link.callback(|_: MouseEvent| Msg::CloseCompose)}
                    class="text-lg text-black/40 hover:text-black/70 disabled:opacity-40">{"×"}</button>
            </div>

            <div>
                <label class={LABEL}>{"Document"}</label>
                { match &c.documents {
                    Remote::Loading | Remote::NotAsked => template::loading_line("documents"),
                    Remote::Failed(error) => template::failure(error),
                    Remote::Loaded(rows) if rows.is_empty() => html! {
                        <p class="text-xs font-light text-black/50">{"No issued documents yet. Issue one from Forms first."}</p>
                    },
                    Remote::Loaded(rows) => html! {
                        <select class={INPUT} disabled={busy}
                            onchange={link.callback(|event: Event| {
                                let value = event
                                    .target_dyn_into::<web_sys::HtmlSelectElement>()
                                    .map(|select| select.value())
                                    .unwrap_or_default();
                                Msg::Compose(ComposeEdit::Document(value))
                            })}>
                            <option value="" selected={draft.document_id.is_empty()}>{"Choose a document…"}</option>
                            { for rows.iter().map(|row| html! {
                                <option value={row.id.clone()} selected={row.id == draft.document_id}>{ row.label() }</option>
                            }) }
                        </select>
                    },
                } }
            </div>

            <div class="grid gap-3 sm:grid-cols-2">
                <div>
                    <label class={LABEL}>{"Subject (optional)"}</label>
                    { text_input(link, &draft.subject, "Purchase agreement for signature", busy, ComposeEdit::Subject) }
                </div>
                <div>
                    <label class={LABEL}>{"Message to signers (optional)"}</label>
                    { text_input(link, &draft.message, "Please sign by Friday.", busy, ComposeEdit::Message) }
                </div>
            </div>

            <div>
                <label class={LABEL}>{"People"}</label>
                <div class="space-y-2">
                    { for draft.recipients.iter().enumerate().map(|(i, person)| html! {
                        <div class="grid items-center gap-2 sm:grid-cols-[1fr_1.3fr_auto_auto]">
                            { text_input(link, &person.name, "Full name", busy, move |v| ComposeEdit::Name(i, v)) }
                            { text_input(link, &person.email, "email@example.com", busy, move |v| ComposeEdit::Email(i, v)) }
                            <label class="flex items-center gap-1.5 text-[11px] font-light text-black/60">
                                <input type="checkbox" disabled={busy} checked={person.approver}
                                    onchange={link.callback(move |event: Event| {
                                        let checked = event
                                            .target_dyn_into::<web_sys::HtmlInputElement>()
                                            .map(|input| input.checked())
                                            .unwrap_or(false);
                                        Msg::Compose(ComposeEdit::Approver(i, checked))
                                    })} />
                                {"Approves only"}
                            </label>
                            <button type="button" disabled={busy || draft.recipients.len() < 2}
                                onclick={link.callback(move |_: MouseEvent| Msg::Compose(ComposeEdit::RemovePerson(i)))}
                                class="text-black/35 hover:text-black/70 disabled:opacity-30">{"Remove"}</button>
                        </div>
                    }) }
                </div>
                <button type="button" disabled={busy}
                    onclick={link.callback(|_: MouseEvent| Msg::Compose(ComposeEdit::AddPerson))}
                    class="mt-2 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy)] disabled:opacity-40">
                    {"+ Add person"}
                </button>
            </div>

            <div class="grid gap-3 sm:grid-cols-3">
                <div>
                    <label class={LABEL}>{"Order"}</label>
                    <select class={INPUT} disabled={busy}
                        onchange={link.callback(|event: Event| {
                            let value = event
                                .target_dyn_into::<web_sys::HtmlSelectElement>()
                                .map(|select| select.value())
                                .unwrap_or_default();
                            Msg::Compose(ComposeEdit::Sequential(value == "sequential"))
                        })}>
                        <option value="parallel" selected={!draft.sequential}>{"Everyone at once"}</option>
                        <option value="sequential" selected={draft.sequential}>{"One after another, as listed"}</option>
                    </select>
                </div>
                <div>
                    <label class={LABEL}>{"Expires in (days)"}</label>
                    { text_input(link, &draft.expires_in_days, "14", busy, ComposeEdit::Days) }
                </div>
                <div>
                    <label class={LABEL}>{"Signature on page (blank = last)"}</label>
                    { text_input(link, &draft.signature_page, "Last page", busy, ComposeEdit::Page) }
                </div>
            </div>

            if let Some(error) = c.error.as_deref() {
                <p class="rounded-[var(--portal-tab-radius)] bg-[var(--portal-archive-pale)] px-3 py-2 text-xs font-light text-[var(--portal-archive)]">{ error }</p>
            }

            <div class="flex items-center justify-end gap-3">
                <span class="text-xs font-light text-black/50">{ c.stage.label() }</span>
                <button type="button" disabled={busy} onclick={link.callback(|_: MouseEvent| Msg::SendCompose)}
                    class="rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-5 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-50">
                    {"Send invitations"}
                </button>
            </div>
        </section>
    }
}

fn list_body(model: &Model, link: &Link<Msg>) -> Html {
    match &model.envelopes {
        Remote::Loading | Remote::NotAsked => template::loading_line("envelopes"),
        Remote::Failed(error) => template::failure(error),
        Remote::Loaded(rows) if rows.is_empty() => {
            template::empty_panel("No envelopes yet. Choose “Send for signature” to start one.")
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
    fn sending_is_one_command_and_closes_the_panel_when_it_succeeds() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = DocumentSigning::init(&ctx);
        DocumentSigning::update(&mut model, Msg::OpenCompose, &ctx);
        for edit in [
            ComposeEdit::Document("doc-1".into()),
            ComposeEdit::Name(0, "Ada".into()),
            ComposeEdit::Email(0, "ada@example.com".into()),
        ] {
            DocumentSigning::update(&mut model, Msg::Compose(edit), &ctx);
        }
        // An incomplete draft is refused without a request.
        let mut bad = model.clone();
        DocumentSigning::update(
            &mut bad,
            Msg::Compose(ComposeEdit::Email(0, "nope".into())),
            &ctx,
        );
        let cmd = DocumentSigning::update(&mut bad, Msg::SendCompose, &ctx);
        assert!(cmd.into_requests().is_empty());
        assert!(bad.compose.as_ref().unwrap().error.is_some());

        let cmd = DocumentSigning::update(&mut model, Msg::SendCompose, &ctx);
        assert_eq!(model.compose.as_ref().unwrap().stage, Stage::Sending);
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1, "one command, not a chain");
        assert_eq!(requests[0].path, "/v1/commands/dispatch");

        let sent = serde_json::json!({ "outcome": "success", "value": { "issued": {} } });
        DocumentSigning::update(&mut model, Msg::ComposeStepped(Ok(sent)), &ctx);
        assert!(model.compose.is_none());
        assert!(model
            .notice
            .as_deref()
            .unwrap()
            .starts_with("Sent for signature"));
    }

    #[test]
    fn a_refused_send_keeps_the_panel_and_says_why() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = DocumentSigning::init(&ctx);
        model.compose = Some(Compose {
            stage: Stage::Sending,
            ..Compose::default()
        });
        let refused = serde_json::json!({
            "outcome": "rejected",
            "error": { "message": "Page 5 does not exist: the document has 3 page(s)." }
        });
        DocumentSigning::update(&mut model, Msg::ComposeStepped(Ok(refused)), &ctx);
        let compose = model.compose.as_ref().unwrap();
        assert_eq!(compose.stage, Stage::Idle);
        assert!(compose.error.as_deref().unwrap().contains("3 page"));
    }

    #[test]
    fn status_buckets_match_the_metrics() {
        assert_eq!(EnvelopeStatus::of("completed"), EnvelopeStatus::Completed);
        assert_eq!(EnvelopeStatus::of("bogus"), EnvelopeStatus::Waiting);
    }
}
