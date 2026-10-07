//! Public signer: the session behind one signing link, wired live.
//!
//! The token in `/sign/:token` is the credential. Init loads the session,
//! records the open, and the recipient works their own fields through the
//! edge commands — consent, field, complete, decline — re-reading the
//! session after each answer so the screen always draws server truth.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{SignerActPost, SignerSessionPost};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{SignerField, SignerSession};

/// The consent sentence shown beside the checkbox. Stored verbatim as
/// evidence, so this text and its hash are what the signer accepted.
const CONSENT_TEXT: &str =
    "I agree to use this electronic signature and intend to sign this document.";
const CONSENT_VERSION: &str = "v1";

fn consent_sha256() -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(CONSENT_TEXT.as_bytes()))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    token: String,
    session: Remote<SignerSession>,
    consent: bool,
    signature_style: usize,
    /// Text-like field values by field id.
    values: BTreeMap<String, String>,
    /// Checkbox states by field id.
    checked: BTreeMap<String, bool>,
    /// Selected option by field id (radio, dropdown).
    selected: BTreeMap<String, String>,
    working: bool,
    notice: Option<String>,
    /// The decline step is open (it asks for an optional reason before anything is sent).
    declining: bool,
    decline_reason: String,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    SessionLoaded(Result<SignerSession, ApiError>),
    ConsentChanged(bool),
    SignatureStyle(usize),
    FieldChanged(String, String),
    FieldChecked(String, bool),
    OptionSelected(String, String),
    ConsentSubmitted,
    FieldSubmitted(String),
    CompleteSubmitted,
    DeclineOpened,
    DeclineCancelled,
    DeclineReasonChanged(String),
    DeclineSubmitted,
    Acted(Result<serde_json::Value, ApiError>),
}

pub struct SignDocument;

fn act(
    token: &str,
    recipient_id: &str,
    action: &'static str,
    extra: serde_json::Value,
) -> Cmd<Msg> {
    let mut body = extra.as_object().cloned().unwrap_or_default();
    body.insert(
        "accessToken".into(),
        serde_json::Value::String(token.to_owned()),
    );
    body.insert(
        "recipientId".into(),
        serde_json::Value::String(recipient_id.to_owned()),
    );
    Cmd::request(
        SignerActPost {
            action,
            body: serde_json::Value::Object(body),
        },
        Msg::Acted,
    )
}

fn reload(token: &str) -> Cmd<Msg> {
    Cmd::request(
        SignerSessionPost {
            token: token.to_owned(),
        },
        Msg::SessionLoaded,
    )
}

impl Screen for SignDocument {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let token = ctx.id.clone().unwrap_or_default();
        let mut model = Model {
            token: token.clone(),
            session: Remote::Loading,
            ..Model::default()
        };
        if token.trim().is_empty() {
            model.session =
                Remote::Failed(ApiError::network("This signing link is missing its token."));
            return (model, Cmd::none());
        }
        let cmd = reload(&token);
        (model, cmd)
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::SessionLoaded(Ok(session)) => {
                let first_sight = !matches!(model.session, Remote::Loaded(_));
                let token = model.token.clone();
                let recipient = session.recipient.id.clone();
                model.session = Remote::Loaded(session);
                model.working = false;
                // Record the open once, when the session first arrives.
                if first_sight {
                    return act(&token, &recipient, "open", serde_json::json!({}));
                }
                Cmd::none()
            }
            Msg::SessionLoaded(Err(error)) => {
                model.session = Remote::Failed(error);
                model.working = false;
                Cmd::none()
            }
            Msg::ConsentChanged(value) => {
                model.consent = value;
                Cmd::none()
            }
            Msg::SignatureStyle(style) => {
                model.signature_style = style.min(2);
                Cmd::none()
            }
            Msg::FieldChanged(id, value) => {
                model.values.insert(id, value);
                Cmd::none()
            }
            Msg::FieldChecked(id, value) => {
                model.checked.insert(id, value);
                Cmd::none()
            }
            Msg::OptionSelected(id, value) => {
                model.selected.insert(id, value);
                Cmd::none()
            }
            Msg::ConsentSubmitted => {
                let (token, recipient) = match &model.session {
                    Remote::Loaded(session) if !model.working => {
                        (model.token.clone(), session.recipient.id.clone())
                    }
                    _ => return Cmd::none(),
                };
                model.working = true;
                model.notice = None;
                act(
                    &token,
                    &recipient,
                    "consent",
                    serde_json::json!({
                        "consentVersion": CONSENT_VERSION,
                        "consentText": CONSENT_TEXT,
                        "consentTextSha256": consent_sha256(),
                    }),
                )
            }
            Msg::FieldSubmitted(id) => {
                let (token, recipient, value) = match &model.session {
                    Remote::Loaded(session) if !model.working => {
                        let field = session.fields.iter().find(|field| field.id == id);
                        let field = match field {
                            Some(field) => field,
                            None => return Cmd::none(),
                        };
                        (
                            model.token.clone(),
                            session.recipient.id.clone(),
                            field_value(field, model),
                        )
                    }
                    _ => return Cmd::none(),
                };
                model.working = true;
                model.notice = None;
                act(
                    &token,
                    &recipient,
                    "field",
                    serde_json::json!({ "fieldId": id, "value": value }),
                )
            }
            Msg::CompleteSubmitted => {
                let (token, recipient) = match &model.session {
                    Remote::Loaded(session) if !model.working => {
                        (model.token.clone(), session.recipient.id.clone())
                    }
                    _ => return Cmd::none(),
                };
                model.working = true;
                model.notice = None;
                act(&token, &recipient, "complete", serde_json::json!({}))
            }
            Msg::DeclineOpened => {
                model.declining = true;
                model.notice = None;
                Cmd::none()
            }
            Msg::DeclineCancelled => {
                model.declining = false;
                model.decline_reason.clear();
                Cmd::none()
            }
            Msg::DeclineReasonChanged(value) => {
                model.decline_reason = value;
                Cmd::none()
            }
            Msg::DeclineSubmitted => {
                let (token, recipient) = match &model.session {
                    Remote::Loaded(session) if !model.working => {
                        (model.token.clone(), session.recipient.id.clone())
                    }
                    _ => return Cmd::none(),
                };
                model.working = true;
                model.notice = None;
                let reason = model.decline_reason.trim();
                let reason = if reason.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(reason.chars().take(1000).collect())
                };
                act(
                    &token,
                    &recipient,
                    "decline",
                    serde_json::json!({ "reason": reason }),
                )
            }
            Msg::Acted(Ok(body)) => match crate::app::cmd::command_refusal(&body) {
                Some(message) => {
                    model.working = false;
                    model.notice = Some(message);
                    Cmd::none()
                }
                None => reload(&model.token.clone()),
            },
            Msg::Acted(Err(error)) => {
                model.working = false;
                model.notice = Some(error.message.clone());
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        match &model.session {
            Remote::Loading | Remote::NotAsked => page(html! {
                <div class="flex justify-center py-24">
                    { template::loading_toned(template::Tone::Site, "your signing session") }
                </div>
            }),
            Remote::Failed(error) => failure_view(error),
            Remote::Loaded(session) => {
                if session.state == "completed" {
                    return completed_view(model, session);
                }
                if session.state == "declined" {
                    return declined_view(session);
                }
                signing_view(model, session, link)
            }
        }
    }
}

/// The value to send for one field, from the screen's inputs.
fn field_value(field: &SignerField, model: &Model) -> serde_json::Value {
    match field.field_type.as_str() {
        "checkbox" => serde_json::json!({
            "checked": model.checked.get(&field.id).copied().unwrap_or(false)
        }),
        "radio" | "dropdown" => serde_json::json!({
            "option": model.selected.get(&field.id).cloned().unwrap_or_default()
        }),
        "signature" => serde_json::json!({
            "style": model.signature_style,
            "name": field_label_name(model, field),
        }),
        _ => serde_json::json!({
            "text": model.values.get(&field.id).cloned().unwrap_or_default()
        }),
    }
}

fn field_label_name(model: &Model, _field: &SignerField) -> String {
    match &model.session {
        Remote::Loaded(session) => session.recipient.name.clone(),
        _ => String::new(),
    }
}

/// Every state of the signing page sits on the same ground, under the same brand bar. The site's own header is not
/// drawn here (`Entry::chrome_free`): a signer sees the document and what is asked of them, nothing to wander off to.
fn page(content: Html) -> Html {
    html! {
        <div class="min-h-screen bg-[#f4f1ea]">
            <header class="bg-[#041024]">
                <div class="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6 lg:px-8">
                    <img src="/images/culebraluxe-header-logo-test.png" alt="CulebraLuxe" width="2050" height="300"
                        class="h-7 w-auto max-w-[60%] object-contain" />
                    <span class="flex items-center gap-2 text-[10px] font-medium uppercase tracking-[0.2em] text-[#caa36b]">
                        <span class="inline-block h-1.5 w-1.5 rounded-full bg-[#caa36b]"></span>
                        {"Secure signing"}
                    </span>
                </div>
            </header>
            <main class="px-4 py-8 sm:px-6 lg:px-8">
                <div class="mx-auto max-w-6xl">{ content }</div>
            </main>
            <footer class="px-4 pb-10 text-center text-[11px] font-light leading-5 text-black/40">
                {"Your signature, the time and the details of this signing are recorded as part of the document's audit trail."}
            </footer>
        </div>
    }
}

/// What a signer is told when their link does not open a session, in words they can act on.
fn failure_view(error: &ApiError) -> Html {
    let (heading, body) = match error.code.as_str() {
        "SIGNER_ACCESS_EXPIRED" => (
            "This signing link has expired",
            "Ask the person who sent it to you to send a new one.",
        ),
        "SIGNER_ACCESS_INVALID" => (
            "This signing link is not valid",
            "It may have been copied incorrectly, replaced by a newer link, or closed because the signing ended. Check your email for the most recent message, or ask the sender for a new link.",
        ),
        _ => (
            "We could not open this signing page",
            "Please try the link again in a moment. If it keeps failing, ask the sender for a new one.",
        ),
    };
    page(html! {
        <section class="mx-auto mt-10 max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm" role="alert" data-screen-state="failed">
            <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-[#fffaf0] text-xl text-[#a88450]">{"!"}</div>
            <h1 class="mt-5 font-serif text-3xl font-light text-[#041024]">{ heading }</h1>
            <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/55">{ body }</p>
            <p class="mt-6 text-[10px] uppercase tracking-[0.16em] text-black/30">{ error.code.clone() }</p>
        </section>
    })
}

/// The three beats of a signing: read it, accept signing electronically, sign.
fn stepper(session: &SignerSession) -> Html {
    let done = [true, session.consented, false];
    let current = if !session.consented { 1 } else { 2 };
    let labels = ["Review", "Accept", "Sign"];
    html! {
        <ol class="flex items-center gap-2 text-[10px] font-medium uppercase tracking-[0.14em]" aria-label="Progress">
            { for labels.iter().enumerate().map(|(index, label)| {
                let state = if done[index] { "done" } else if index == current { "current" } else { "todo" };
                let (dot, text) = match state {
                    "done" => ("bg-emerald-600 text-white", "text-black/55"),
                    "current" => ("bg-[#041024] text-white", "text-[#041024]"),
                    _ => ("border border-black/20 text-black/35", "text-black/35"),
                };
                html! {
                    <>
                        if index > 0 { <span class="h-px w-6 bg-black/15 sm:w-10"></span> }
                        <li class="flex items-center gap-2">
                            <span class={classes!("flex", "h-5", "w-5", "items-center", "justify-center", "rounded-full", "text-[9px]", dot)}>
                                { if state == "done" { "✓".to_owned() } else { (index + 1).to_string() } }
                            </span>
                            <span class={text}>{ *label }</span>
                        </li>
                    </>
                }
            }) }
        </ol>
    }
}

fn party_state(state: &str) -> (&'static str, &'static str) {
    match state {
        "completed" => ("Signed", "bg-emerald-50 text-emerald-700"),
        "declined" => ("Declined", "bg-red-50 text-red-800"),
        "expired" | "revoked" => ("Closed", "bg-black/5 text-black/45"),
        "viewed" | "in_progress" => ("Reviewing", "bg-[#fffaf0] text-[#a88450]"),
        _ => ("Waiting", "bg-black/5 text-black/50"),
    }
}

/// Everyone on the envelope and how far along they are: names and states only.
fn parties_panel(session: &SignerSession) -> Html {
    if session.parties.len() < 2 {
        return Html::default();
    }
    html! {
        <section class="mb-5 rounded-xl border border-black/10 bg-white px-4 py-3 shadow-sm">
            <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"Who is signing"}</p>
            <ul class="mt-2 flex flex-wrap gap-x-6 gap-y-2">
                { for session.parties.iter().map(|party| {
                    let (label, tone) = party_state(&party.state);
                    html! {
                        <li class="flex items-center gap-2 text-sm font-light text-[#041024]">
                            <span>{ if party.is_you { format!("{} (you)", party.name) } else { party.name.clone() } }</span>
                            <span class={classes!("rounded-full", "px-2", "py-0.5", "text-[9px]", "font-medium", "uppercase", "tracking-[0.1em]", tone)}>{ label }</span>
                        </li>
                    }
                }) }
            </ul>
        </section>
    }
}

/// "Prepared for María Alvarez · Open until 2026-10-14" (one string: a text node loses a leading space).
fn prepared_for(session: &SignerSession) -> String {
    let who = format!("Prepared for {}", session.recipient.name);
    let until = expiry_phrase(&session.expires_at);
    if until.is_empty() {
        who
    } else {
        format!("{who} \u{b7} {until}")
    }
}

fn expiry_phrase(expires_at: &str) -> String {
    match expires_at.get(..10) {
        Some(day) if !day.is_empty() => format!("Open until {day}"),
        _ => String::new(),
    }
}

fn signing_view(model: &Model, session: &SignerSession, link: &Link<Msg>) -> Html {
    let turn = session.is_turn;
    let heading = session
        .document_name()
        .map(str::to_owned)
        .unwrap_or_else(|| "Review and sign".to_owned());
    page(html! {
        <>
            <header class="mb-6 flex flex-wrap items-end justify-between gap-4">
                <div class="min-w-0">
                    <p class="text-[10px] font-medium uppercase tracking-[0.24em] text-[#a88450]">{"Review and sign"}</p>
                    <h1 class="mt-1 break-words font-serif text-3xl font-light text-[#041024] sm:text-4xl">{ heading }</h1>
                    <p class="mt-1 text-sm font-light text-black/50">
                        { prepared_for(session) }
                    </p>
                </div>
                <div class="flex flex-col items-start gap-3 sm:items-end">
                    { stepper(session) }
                    <div class="rounded-full border border-black/10 bg-white/70 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-black/45">
                        { session_state_label(session) }
                    </div>
                </div>
            </header>

            if let Some(notice) = &model.notice {
                <div class="mb-5 rounded-lg border border-red-900/20 bg-red-50 px-4 py-3 text-sm font-light text-red-900" role="alert">{ notice.clone() }</div>
            }

            if let Some(message) = session.message.as_deref().map(str::trim).filter(|message| !message.is_empty()) {
                <section class="mb-5 rounded-xl border border-[#caa36b]/40 bg-[#fffaf0] px-5 py-4">
                    <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-[#a88450]">{"A note from the sender"}</p>
                    <p class="mt-1 whitespace-pre-wrap text-sm font-light leading-6 text-black/70">{ message.to_owned() }</p>
                </section>
            }

            if !turn {
                <div class="mb-5 rounded-lg border border-[#caa36b]/50 bg-[#fffaf0] px-4 py-3 text-sm font-light text-black/60">
                    {"An earlier signer must finish first — your fields unlock when your turn arrives."}
                </div>
            }

            { parties_panel(session) }

            <section class="mb-5 overflow-hidden rounded-xl border border-black/10 bg-white shadow-sm">
                <div class="flex flex-wrap items-center justify-between gap-2 border-b border-black/10 bg-white/75 px-4 py-2.5">
                    <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"The document"}</span>
                    <span class="flex gap-4 text-[11px] font-light">
                        <a class="text-[#041024] underline decoration-black/20 underline-offset-2" href={document_url(&model.token)} target="_blank" rel="noopener">{"Open in a new tab"}</a>
                        <a class="text-[#041024] underline decoration-black/20 underline-offset-2" href={format!("{}?download=1", document_url(&model.token))}>{"Download"}</a>
                    </span>
                </div>
                <iframe class="block h-[60vh] min-h-[24rem] max-h-[44rem] w-full bg-[#f4f1ea]" title="The document you are asked to sign" src={format!("{}#navpanes=0&view=FitH", document_url(&model.token))}></iframe>
            </section>

            <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_21rem]">
                <section class="overflow-hidden rounded-xl border border-black/10 bg-white shadow-sm">
                    <div class="flex items-center justify-between border-b border-black/10 bg-white/75 px-4 py-2.5">
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"What is asked of you"}</span>
                        <span class="text-[10px] font-light text-black/35">
                            { format!("{} of {} done", session.answered_field_ids.len().min(session.fields.len()), session.fields.len()) }
                        </span>
                    </div>
                    <div class="space-y-5 p-4 sm:p-6">
                        if !session.consented && !session.fields.is_empty() {
                            <p class="rounded-lg bg-[#fffaf0] px-3 py-2 text-xs font-light text-black/55">
                                {"First, read the document above and accept on the right. These unlock once you have."}
                            </p>
                        }
                        { for session.fields.iter().map(|field| field_editor(model, session, field, link)) }
                        if session.fields.is_empty() {
                            <p class="text-sm font-light text-black/45">{"Nothing to fill in: review the document, then sign and complete."}</p>
                        }
                    </div>
                </section>

                <aside class="self-start rounded-xl border border-black/10 bg-white p-5 shadow-sm lg:sticky lg:top-6">
                    <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-[#a88450]">{"Your signature"}</p>
                    <h2 class="mt-1 font-serif text-2xl font-light text-[#041024]">{ session.recipient.name.clone() }</h2>

                    <div class="mt-5">
                        <p class="text-[9px] font-medium uppercase tracking-[0.14em] text-black/35">{"Choose appearance"}</p>
                        <div class="mt-2 space-y-2">
                            { for (0..3).map(|style| signature_choice(&session.recipient.name, model.signature_style, style, link)) }
                        </div>
                    </div>

                    if !session.consented {
                        <label class="mt-5 flex cursor-pointer items-start gap-3 text-xs font-light leading-5 text-black/60">
                            <input
                                type="checkbox"
                                checked={model.consent}
                                onchange={link.callback(|event: Event| {
                                    let checked = event
                                        .target_dyn_into::<web_sys::HtmlInputElement>()
                                        .map(|input| input.checked())
                                        .unwrap_or(false);
                                    Msg::ConsentChanged(checked)
                                })}
                                class="mt-1 h-4 w-4 accent-[#041024]"
                            />
                            <span>{ CONSENT_TEXT }</span>
                        </label>
                        <button
                            type="button"
                            disabled={!model.consent || model.working}
                            onclick={link.callback(|_: MouseEvent| Msg::ConsentSubmitted)}
                            class="mt-5 flex w-full items-center justify-center rounded-lg bg-[#041024] px-4 py-3 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38] disabled:cursor-not-allowed disabled:opacity-35"
                        >
                            { if model.working { "Working…" } else { "Accept & Continue" } }
                        </button>
                    } else {
                        <button
                            type="button"
                            disabled={!turn || model.working || !session.fields_answered()}
                            onclick={link.callback(|_: MouseEvent| Msg::CompleteSubmitted)}
                            class="mt-5 flex w-full items-center justify-center rounded-lg bg-[#041024] px-4 py-3 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38] disabled:cursor-not-allowed disabled:opacity-35"
                        >
                            { if model.working { "Working…" } else { "Sign & Complete" } }
                        </button>
                        if turn && !session.fields_answered() {
                            <p class="mt-2 text-[11px] font-light leading-4 text-black/45">{"Finish the fields on the left first, then complete."}</p>
                        }
                        { decline_panel(model, link) }
                    }
                </aside>
            </div>
        </>
    })
}

/// "Decline to sign" is a deliberate two-step: it ends the signing for everyone, so it asks first, and lets the signer
/// say why (the sender is told).
fn decline_panel(model: &Model, link: &Link<Msg>) -> Html {
    if !model.declining {
        return html! {
            <button
                type="button"
                disabled={model.working}
                onclick={link.callback(|_: MouseEvent| Msg::DeclineOpened)}
                class="mt-3 flex w-full items-center justify-center rounded-lg border border-black/15 px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-black/55 transition hover:border-black/30"
            >
                {"Decline to sign"}
            </button>
        };
    }
    html! {
        <div class="mt-4 rounded-lg border border-red-900/20 bg-red-50/50 p-3">
            <p class="text-xs font-light leading-5 text-black/70">
                {"Declining ends this signing for everyone, and the sender is told. You can say why (optional)."}
            </p>
            <textarea
                rows="3"
                maxlength="1000"
                value={model.decline_reason.clone()}
                disabled={model.working}
                placeholder="Reason (optional)"
                oninput={link.callback(|event: InputEvent| {
                    let value = event
                        .target_dyn_into::<web_sys::HtmlTextAreaElement>()
                        .map(|input| input.value())
                        .unwrap_or_default();
                    Msg::DeclineReasonChanged(value)
                })}
                class="mt-2 w-full rounded-md border border-black/15 bg-white px-3 py-2 text-sm font-light text-[#041024]"
            />
            <div class="mt-2 flex gap-2">
                <button
                    type="button"
                    disabled={model.working}
                    onclick={link.callback(|_: MouseEvent| Msg::DeclineSubmitted)}
                    class="flex-1 rounded-lg bg-red-900 px-3 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-40"
                >
                    { if model.working { "Working…" } else { "Confirm decline" } }
                </button>
                <button
                    type="button"
                    disabled={model.working}
                    onclick={link.callback(|_: MouseEvent| Msg::DeclineCancelled)}
                    class="rounded-lg border border-black/15 px-3 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-black/55"
                >
                    {"Cancel"}
                </button>
            </div>
        </div>
    }
}

fn session_state_label(session: &SignerSession) -> String {
    match session.state.as_str() {
        "completed" => "Signed".into(),
        "notified" | "pending" => "Waiting for you".into(),
        other => {
            let mut label = other.replace('_', " ");
            if let Some(first) = label.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

fn field_editor(
    model: &Model,
    session: &SignerSession,
    field: &SignerField,
    link: &Link<Msg>,
) -> Html {
    // Nothing is saved before the signer has agreed to sign electronically: the buttons are off until they have.
    let locked = !session.is_turn || model.working || !session.consented;
    let label = field
        .label
        .clone()
        .unwrap_or_else(|| field.field_key.clone());
    let submit = {
        let id = field.id.clone();
        link.callback(move |_: MouseEvent| Msg::FieldSubmitted(id.clone()))
    };
    let input = match field.field_type.as_str() {
        "checkbox" => {
            let id = field.id.clone();
            let checked = model.checked.get(&field.id).copied().unwrap_or(false);
            html! {
                <label class="flex cursor-pointer items-center gap-3 text-sm font-light text-black/70">
                    <input
                        type="checkbox"
                        checked={checked}
                        disabled={locked}
                        onchange={link.callback(move |event: Event| {
                            let value = event
                                .target_dyn_into::<web_sys::HtmlInputElement>()
                                .map(|input| input.checked())
                                .unwrap_or(false);
                            Msg::FieldChecked(id.clone(), value)
                        })}
                        class="h-4 w-4 accent-[#041024]"
                    />
                    { label }
                    if field.required { <span class="text-[#a88450]">{"*"}</span> }
                </label>
            }
        }
        "signature" => html! {
            <div>
                <span class="block text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">
                    { if field.label.is_some() { label.clone() } else { "Your signature".to_owned() } }
                    if field.required { <span class="text-[#a88450]">{" *"}</span> }
                </span>
                <p class="mt-2 text-sm font-light text-black/60">
                    { format!("Signs as {} in the appearance you choose on the right.", session.recipient.name) }
                </p>
            </div>
        },
        "radio" | "dropdown" => {
            let id = field.id.clone();
            let selected = model.selected.get(&field.id).cloned().unwrap_or_default();
            html! {
                <label class="block">
                    <span class="mb-1 block text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">
                        { label }
                        if field.required { <span class="text-[#a88450]">{" *"}</span> }
                    </span>
                    <input
                        type="text"
                        value={selected}
                        disabled={locked}
                        placeholder="Your answer"
                        oninput={link.callback(move |event: InputEvent| {
                            let value = event
                                .target_dyn_into::<web_sys::HtmlInputElement>()
                                .map(|input| input.value())
                                .unwrap_or_default();
                            Msg::OptionSelected(id.clone(), value)
                        })}
                        class="w-full rounded-md border border-black/15 bg-white px-3 py-2 text-sm font-light text-[#041024] disabled:opacity-50"
                    />
                </label>
            }
        }
        _ => {
            let id = field.id.clone();
            let value = model.values.get(&field.id).cloned().unwrap_or_default();
            html! {
                <label class="block">
                    <span class="mb-1 block text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">
                        { label }
                        if field.required { <span class="text-[#a88450]">{" *"}</span> }
                    </span>
                    <input
                        type="text"
                        value={value}
                        disabled={locked}
                        placeholder="Type here"
                        oninput={link.callback(move |event: InputEvent| {
                            let value = event
                                .target_dyn_into::<web_sys::HtmlInputElement>()
                                .map(|input| input.value())
                                .unwrap_or_default();
                            Msg::FieldChanged(id.clone(), value)
                        })}
                        class="w-full rounded-md border border-black/15 bg-white px-3 py-2 text-sm font-light text-[#041024] disabled:opacity-50"
                    />
                </label>
            }
        }
    };
    html! {
        <div class="rounded-lg border border-black/10 bg-[#f7f4ed] p-4">
            { input }
            <button
                type="button"
                disabled={locked}
                onclick={submit}
                class="mt-3 rounded-md border border-[#caa36b] bg-[#fffaf0] px-4 py-2 text-[10px] font-medium uppercase tracking-[0.14em] text-[#041024] transition hover:bg-[#caa36b]/20 disabled:cursor-not-allowed disabled:opacity-40"
            >
                { if field.field_type == "signature" { "Sign here" } else { "Save field" } }
            </button>
        </div>
    }
}

fn signature_choice(name: &str, selected: usize, style: usize, link: &Link<Msg>) -> Html {
    let onclick = link.callback(move |_: MouseEvent| Msg::SignatureStyle(style));
    html! {
        <button
            type="button"
            {onclick}
            class={classes!(
                "flex", "w-full", "items-center", "justify-between", "rounded-lg", "border", "px-3", "py-2.5", "text-left", "transition",
                if selected == style {
                    "border-[#caa36b] bg-[#fffaf0]"
                } else {
                    "border-black/10 bg-white hover:border-black/20"
                }
            )}
        >
            { signature_text_for(name, style, "text-lg") }
            if selected == style {
                <span class="text-[9px] font-medium uppercase tracking-[0.12em] text-[#a88450]">{"Selected"}</span>
            }
        </button>
    }
}

fn signature_text_for(name: &str, style: usize, size: &'static str) -> Html {
    let style_class = match style {
        1 => "font-serif italic tracking-wide",
        2 => "font-serif font-semibold italic",
        _ => "font-serif italic",
    };
    html! {
        <span class={classes!(size, style_class, "text-[#041024]")}>{ name.to_owned() }</span>
    }
}

/// The public, token-bound route that serves the document being signed.
fn document_url(token: &str) -> String {
    format!("/v1/signer/document/{token}")
}

fn completed_view(model: &Model, session: &SignerSession) -> Html {
    let everyone = session.envelope_status == "completed";
    let name = session.document_name().map(str::to_owned);
    page(html! {
        <section class="mx-auto mt-6 max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm">
            <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-50 text-xl text-emerald-700">{"✓"}</div>
            <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{ name.unwrap_or_else(|| "Secure signing".to_owned()) }</p>
            <h1 class="mt-2 font-serif text-3xl font-light text-[#041024]">{ if everyone { "Everyone has signed" } else { "Your signature is recorded" } }</h1>
            <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/55">
                { if everyone {
                    format!("Thank you, {}. The document is complete and sealed. Keep a copy for your records.", session.recipient.name)
                } else {
                    format!("Thank you, {}. We will email you the signed document as soon as everyone has signed.", session.recipient.name)
                } }
            </p>
            if everyone {
                <a href={format!("/v1/signer/signed/{}", model.token)}
                    class="mt-6 inline-flex items-center justify-center rounded-lg bg-[#041024] px-6 py-3 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38]">
                    {"Download the signed copy"}
                </a>
            }
            <div class="mt-6 text-left">{ parties_panel(session) }</div>
        </section>
    })
}

fn declined_view(session: &SignerSession) -> Html {
    page(html! {
        <section class="mx-auto mt-6 max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm">
            <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-red-50 text-xl text-red-800">{"×"}</div>
            <h1 class="mt-5 font-serif text-3xl font-light text-[#041024]">{"You declined to sign"}</h1>
            <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/55">
                { format!("{}, the sender has been told. Nothing further is needed from you; this signing has ended.", session.recipient.name) }
            </p>
        </section>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_loads_then_consent_unlocks_completion() {
        let ctx = ScreenCtx {
            id: Some("token-abc".into()),
            ..ScreenCtx::default()
        };
        let (mut model, cmd) = SignDocument::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/v1/signer/session");
        assert_eq!(model.token, "token-abc");

        let session = test_session(false);
        SignDocument::update(&mut model, Msg::SessionLoaded(Ok(session)), &ctx);
        // The open is recorded automatically on first sight.
        // Consent is a separate, deliberate act.
        assert!(!model.consent);
    }

    #[test]
    fn working_guards_double_submit() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = SignDocument::init(&ctx);
        model.token = "token-abc".into();
        model.session = Remote::Loaded(test_session(true));
        model.working = true;
        let cmd = SignDocument::update(&mut model, Msg::CompleteSubmitted, &ctx);
        assert!(cmd.into_requests().is_empty());
    }

    fn test_session(consented: bool) -> SignerSession {
        SignerSession {
            signature_request_id: "req-1".into(),
            recipient: crate::model::SignerRecipient {
                id: "r-1".into(),
                signature_request_id: "req-1".into(),
                name: "Ada".into(),
                email: "ada@example.test".into(),
                role: "signer".into(),
                signer_order: 1,
                signing_step: 1,
            },
            state: "in_progress".into(),
            fields: vec![],
            consented,
            is_turn: true,
            expires_at: "2026-12-01T00:00:00+00:00".into(),
            ..SignerSession::default()
        }
    }

    #[test]
    fn declining_is_two_steps_and_sends_the_reason() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = SignDocument::init(&ctx);
        model.token = "token-abc".into();
        model.session = Remote::Loaded(test_session(true));

        // Opening the step sends nothing: it only asks.
        let cmd = SignDocument::update(&mut model, Msg::DeclineOpened, &ctx);
        assert!(model.declining);
        assert!(cmd.into_requests().is_empty());

        SignDocument::update(
            &mut model,
            Msg::DeclineReasonChanged("  Price is wrong  ".into()),
            &ctx,
        );
        let cmd = SignDocument::update(&mut model, Msg::DeclineSubmitted, &ctx);
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/signer/decline");
        assert!(requests[0]
            .body
            .as_ref()
            .unwrap()
            .to_string()
            .contains("Price is wrong"));
    }

    #[test]
    fn the_page_names_the_document_by_the_senders_subject_first_and_joins_the_line_with_spaces() {
        let mut session = test_session(true);
        session.document_title = Some("docsign proof".into());
        assert_eq!(session.document_name(), Some("docsign proof"));
        session.subject = Some("  Listing Agreement  ".into());
        assert_eq!(session.document_name(), Some("Listing Agreement"));
        assert_eq!(
            prepared_for(&session),
            "Prepared for Ada \u{b7} Open until 2026-12-01"
        );
        session.expires_at.clear();
        assert_eq!(prepared_for(&session), "Prepared for Ada");
    }

    #[test]
    fn cancelling_a_decline_forgets_the_reason() {
        let ctx = ScreenCtx::default();
        let (mut model, _) = SignDocument::init(&ctx);
        model.declining = true;
        model.decline_reason = "x".into();
        SignDocument::update(&mut model, Msg::DeclineCancelled, &ctx);
        assert!(!model.declining && model.decline_reason.is_empty());
    }

    #[test]
    fn complete_unlocks_only_when_every_required_field_has_an_answer() {
        let mut session = test_session(true);
        session.fields = vec![crate::model::SignerField {
            id: "f1".into(),
            required: true,
            ..Default::default()
        }];
        assert!(!session.fields_answered());
        session.answered_field_ids = vec!["f1".into()];
        assert!(session.fields_answered());
    }
}
