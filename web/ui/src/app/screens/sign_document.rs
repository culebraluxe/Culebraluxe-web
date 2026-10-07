//! Public signer: the session behind one signing link, wired live.
//!
//! The token in `/sign/:token` is the credential. Init loads the session,
//! records the open, and the recipient works their own fields through the
//! edge commands — consent, field, complete, decline — re-reading the
//! session after each answer so the screen always draws server truth.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{SignerActPost, SignerSessionPost};
use crate::app::cmd::{ApiError, Cmd, Remote, SignatureArt, SIGNATURE_FONTS};
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
    /// The signature and initials as drawn for this signing (what is sent, and what the signer saw).
    art: SignatureArt,
    /// The answers still to send once "Sign & complete" is pressed, in order. One button, one chain.
    queue: Vec<Step>,
}

/// One answer in the chain behind "Sign & complete": agree, answer each block, finish.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Consent,
    Field(String),
    Complete,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    SessionLoaded(Result<SignerSession, ApiError>),
    ConsentChanged(bool),
    SignatureStyle(usize),
    FieldChanged(String, String),
    FieldChecked(String, bool),
    OptionSelected(String, String),
    /// The one primary action: draw the signature, then agree, answer every block and finish.
    SignAndComplete,
    SignatureReady(Result<SignatureArt, ApiError>),
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
                model.signature_style = style.min(SIGNATURE_FONTS.len() - 1);
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
            Msg::SignAndComplete => {
                let Remote::Loaded(session) = &model.session else {
                    return Cmd::none();
                };
                if model.working {
                    return Cmd::none();
                }
                if let Some(problem) = sign_blocker(model, session) {
                    model.notice = Some(problem);
                    return Cmd::none();
                }
                let (name, style) = (session.recipient.name.clone(), model.signature_style);
                model.working = true;
                model.notice = None;
                Cmd::render_signature(name, style, Msg::SignatureReady)
            }
            Msg::SignatureReady(Ok(art)) => {
                model.art = art;
                let Remote::Loaded(session) = &model.session else {
                    model.working = false;
                    return Cmd::none();
                };
                let mut queue = Vec::new();
                if !session.consented {
                    queue.push(Step::Consent);
                }
                for field in &session.fields {
                    if !session.answered_field_ids.contains(&field.id) {
                        queue.push(Step::Field(field.id.clone()));
                    }
                }
                queue.push(Step::Complete);
                model.queue = queue;
                next_step(model)
            }
            Msg::SignatureReady(Err(error)) => {
                model.working = false;
                model.notice = Some(error.message);
                Cmd::none()
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
                    model.queue.clear();
                    model.notice = Some(message);
                    Cmd::none()
                }
                None if !model.queue.is_empty() => next_step(model),
                None => reload(&model.token.clone()),
            },
            Msg::Acted(Err(error)) => {
                model.working = false;
                model.queue.clear();
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

/// The value to send for one field. A signature and initials carry the picture drawn for this signing (the very
/// pixels sealed into the document); the date needs no answer (the seal writes the day they finished).
fn field_value(field: &SignerField, model: &Model) -> serde_json::Value {
    let name = field_label_name(model, field);
    match field.field_type.as_str() {
        "checkbox" => serde_json::json!({
            "checked": model.checked.get(&field.id).copied().unwrap_or(false)
        }),
        "radio" | "dropdown" => serde_json::json!({
            "option": model.selected.get(&field.id).cloned().unwrap_or_default()
        }),
        "signature" => serde_json::json!({
            "style": model.signature_style,
            "name": name,
            "image": model.art.signature,
        }),
        "initials" => serde_json::json!({
            "style": model.signature_style,
            "name": name,
            "initialsImage": model.art.initials,
        }),
        "date" => serde_json::json!({ "auto": true }),
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

/// Fields the signer has to fill in by hand: everything but the three the page does for them.
fn is_hand_filled(field: &SignerField) -> bool {
    !matches!(field.field_type.as_str(), "signature" | "initials" | "date")
}

/// Why "Sign & complete" cannot go yet, in words for the signer; `None` when it can.
fn sign_blocker(model: &Model, session: &SignerSession) -> Option<String> {
    if !session.is_turn {
        return Some("An earlier signer has to finish first.".into());
    }
    if !session.consented && !model.consent {
        return Some("Tick the box to agree to sign electronically.".into());
    }
    for field in session
        .fields
        .iter()
        .filter(|field| is_hand_filled(field) && field.required)
    {
        let label = field
            .label
            .clone()
            .unwrap_or_else(|| field.field_key.clone());
        let filled = match field.field_type.as_str() {
            "checkbox" => model.checked.get(&field.id).copied().unwrap_or(false),
            "radio" | "dropdown" => model
                .selected
                .get(&field.id)
                .is_some_and(|v| !v.trim().is_empty()),
            _ => model
                .values
                .get(&field.id)
                .is_some_and(|v| !v.trim().is_empty()),
        };
        if !filled && !session.answered_field_ids.contains(&field.id) {
            return Some(format!("Please complete “{label}” first."));
        }
    }
    None
}

/// Send the next answer in the chain (agree, each block, finish), or re-read the session when the chain is done.
fn next_step(model: &mut Model) -> Cmd<Msg> {
    if model.queue.is_empty() {
        return reload(&model.token.clone());
    }
    let step = model.queue.remove(0);
    let Remote::Loaded(session) = &model.session else {
        model.working = false;
        model.queue.clear();
        return Cmd::none();
    };
    let (token, recipient) = (model.token.clone(), session.recipient.id.clone());
    match step {
        Step::Consent => act(
            &token,
            &recipient,
            "consent",
            serde_json::json!({
                "consentVersion": CONSENT_VERSION,
                "consentText": CONSENT_TEXT,
                "consentTextSha256": consent_sha256(),
            }),
        ),
        Step::Field(id) => match session.fields.iter().find(|field| field.id == id) {
            Some(field) => {
                let value = field_value(field, model);
                act(
                    &token,
                    &recipient,
                    "field",
                    serde_json::json!({ "fieldId": id, "value": value }),
                )
            }
            None => next_step(model),
        },
        Step::Complete => act(&token, &recipient, "complete", serde_json::json!({})),
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

/// The two beats of a signing: read it, then sign and complete.
fn stepper(_session: &SignerSession) -> Html {
    let steps = [("Review the document", true), ("Sign & complete", false)];
    html! {
        <ol class="flex items-center gap-2 text-[10px] font-medium uppercase tracking-[0.14em]" aria-label="Progress">
            { for steps.iter().enumerate().map(|(index, (label, done))| {
                let (dot, text) = if *done {
                    ("bg-emerald-600 text-white", "text-black/55")
                } else {
                    ("bg-[#041024] text-white", "text-[#041024]")
                };
                html! {
                    <>
                        if index > 0 { <span class="h-px w-6 bg-black/15 sm:w-10"></span> }
                        <li class="flex items-center gap-2">
                            <span class={classes!("flex", "h-5", "w-5", "items-center", "justify-center", "rounded-full", "text-[9px]", dot)}>
                                { if *done { "✓".to_owned() } else { (index + 1).to_string() } }
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

            <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_22rem]">
                <section class="overflow-hidden rounded-xl border border-black/10 bg-white shadow-sm">
                    <div class="border-b border-black/10 bg-white/75 px-4 py-2.5">
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"What you are signing"}</span>
                    </div>
                    <div class="space-y-4 p-4 sm:p-6">
                        <p class="text-sm font-light leading-6 text-black/60">
                            {"Read the document above. When you are ready, choose how your signature looks and press "}
                            <span class="font-normal text-[#041024]">{"Sign & complete"}</span>
                            {" — that one step signs every block below."}
                        </p>
                        <ul class="divide-y divide-black/10 rounded-lg border border-black/10">
                            { for session.fields.iter().filter(|field| !is_hand_filled(field)).map(|field| block_row(session, field)) }
                        </ul>
                        { for session.fields.iter().filter(|field| is_hand_filled(field)).map(|field| hand_field(model, session, field, link)) }
                        if session.fields.is_empty() {
                            <p class="text-sm font-light text-black/45">{"Nothing to fill in: review the document, then sign and complete."}</p>
                        }
                        <p class="text-xs font-light leading-5 text-black/40">
                            {"Your initials are made from your name in the same hand, and the date is filled in the moment you finish."}
                        </p>
                    </div>
                </section>

                <aside class="self-start rounded-xl border border-black/10 bg-white p-5 shadow-sm lg:sticky lg:top-6">
                    <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-[#a88450]">{"Your signature"}</p>

                    <div class="mt-3 rounded-lg border border-black/10 bg-[#fffdf8] px-4 py-3">
                        <p class="truncate text-[2.6rem] leading-[1.15] text-[#041024]" style={signature_face(model.signature_style)}>
                            { session.recipient.name.clone() }
                        </p>
                        <p class="mt-1 flex items-baseline gap-3 text-xs font-light text-black/40">
                            <span>{"Initials"}</span>
                            <span class="text-2xl text-[#041024]" style={signature_face(model.signature_style)}>
                                { model::forms_applied_signature::format_broker_initials(&session.recipient.name) }
                            </span>
                        </p>
                    </div>

                    <p class="mt-4 text-[9px] font-medium uppercase tracking-[0.14em] text-black/35">{"Choose a style"}</p>
                    <div class="mt-2 grid grid-cols-2 gap-2">
                        { for (0..SIGNATURE_FONTS.len()).map(|style| signature_choice(&session.recipient.name, model.signature_style, style, link)) }
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
                    }
                    <button
                        type="button"
                        disabled={model.working || !turn}
                        onclick={link.callback(|_: MouseEvent| Msg::SignAndComplete)}
                        class="mt-5 flex w-full items-center justify-center rounded-lg bg-[#041024] px-4 py-3.5 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38] disabled:cursor-not-allowed disabled:opacity-40"
                    >
                        { if model.working { "Signing…" } else { "Sign & complete" } }
                    </button>
                    <p class="mt-2 text-center text-[11px] font-light leading-4 text-black/40">
                        {"You will be emailed the signed document."}
                    </p>
                    { decline_panel(model, link) }
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

/// The CSS for the cursive face at `style` (an inline style: the family names are declared in `app.css`).
fn signature_face(style: usize) -> String {
    let family = SIGNATURE_FONTS
        .get(style)
        .map(|(family, _)| *family)
        .unwrap_or(SIGNATURE_FONTS[0].0);
    format!("font-family:'{family}',cursive;")
}

/// One block the page signs for the signer: what it is, where, and whether it is done.
fn block_row(session: &SignerSession, field: &SignerField) -> Html {
    let label = match field.field_type.as_str() {
        "signature" => "Your signature",
        "initials" => "Your initials",
        _ => "Date signed",
    };
    let done = session.answered_field_ids.contains(&field.id);
    html! {
        <li class="flex items-center justify-between gap-3 px-4 py-3">
            <span class="flex items-center gap-3 text-sm font-light text-[#041024]">
                <span class={classes!(
                    "flex", "h-5", "w-5", "items-center", "justify-center", "rounded-full", "text-[10px]",
                    if done { "bg-emerald-600 text-white" } else { "border border-black/20 text-transparent" }
                )}>{"✓"}</span>
                { label }
            </span>
            <span class="text-xs font-light text-black/40">{ format!("page {}", field.page_number) }</span>
        </li>
    }
}

/// A block only the signer can fill (text, a tick, a choice): an input, answered with everything else.
fn hand_field(
    model: &Model,
    session: &SignerSession,
    field: &SignerField,
    link: &Link<Msg>,
) -> Html {
    let locked = model.working || !session.is_turn;
    let label = field
        .label
        .clone()
        .unwrap_or_else(|| field.field_key.clone());
    let id = field.id.clone();
    let star = if field.required {
        html! { <span class="text-[#a88450]">{" *"}</span> }
    } else {
        Html::default()
    };
    match field.field_type.as_str() {
        "checkbox" => {
            let checked = model.checked.get(&field.id).copied().unwrap_or(false);
            html! {
                <label class="flex cursor-pointer items-center gap-3 text-sm font-light text-black/70">
                    <input
                        type="checkbox"
                        {checked}
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
                    { label }{ star }
                </label>
            }
        }
        "radio" | "dropdown" => {
            let value = model.selected.get(&field.id).cloned().unwrap_or_default();
            html! {
                <label class="block">
                    <span class="mb-1 block text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{ label }{ star }</span>
                    <input
                        type="text"
                        {value}
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
            let value = model.values.get(&field.id).cloned().unwrap_or_default();
            html! {
                <label class="block">
                    <span class="mb-1 block text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{ label }{ star }</span>
                    <input
                        type="text"
                        {value}
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
    }
}

/// One of the cursive faces, shown as the signer's own name in it.
fn signature_choice(name: &str, selected: usize, style: usize, link: &Link<Msg>) -> Html {
    let onclick = link.callback(move |_: MouseEvent| Msg::SignatureStyle(style));
    let label = SIGNATURE_FONTS
        .get(style)
        .map(|(_, label)| *label)
        .unwrap_or("");
    html! {
        <button
            type="button"
            {onclick}
            aria-pressed={(selected == style).to_string()}
            class={classes!(
                "flex", "min-w-0", "flex-col", "items-start", "rounded-lg", "border", "px-3", "py-2", "text-left", "transition",
                if selected == style {
                    "border-[#caa36b] bg-[#fffaf0] ring-1 ring-[#caa36b]"
                } else {
                    "border-black/10 bg-white hover:border-black/25"
                }
            )}
        >
            <span class="w-full truncate text-[1.2rem] leading-[1.3] text-[#041024]" style={signature_face(style)}>{ name.to_owned() }</span>
            <span class="text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">{ label }</span>
        </button>
    }
}

/// The public, token-bound route that serves the document being signed.
fn document_url(token: &str) -> String {
    format!("/v1/signer/document/{token}")
}

fn completed_view(model: &Model, session: &SignerSession) -> Html {
    // Three honest states: the sealed copy is ready; everyone has signed and it is being sealed; or others still have to.
    let sealed = session.envelope_status == "completed";
    let sealing = session.envelope_status == "signed";
    let name = session.document_name().map(str::to_owned);
    let (heading, body) = if sealed {
        (
            "Everyone has signed",
            format!(
                "Thank you, {}. The document is complete and sealed. Keep a copy for your records.",
                session.recipient.name
            ),
        )
    } else if sealing {
        (
            "Everyone has signed",
            format!(
                "Thank you, {}. We are sealing the document now. The signed copy will be in your email in a moment.",
                session.recipient.name
            ),
        )
    } else {
        (
            "Your signature is recorded",
            format!(
                "Thank you, {}. We will email you the signed document as soon as everyone has signed.",
                session.recipient.name
            ),
        )
    };
    page(html! {
        <section class="mx-auto mt-6 max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm">
            <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-50 text-xl text-emerald-700">{"✓"}</div>
            <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{ name.unwrap_or_else(|| "Secure signing".to_owned()) }</p>
            <h1 class="mt-2 font-serif text-3xl font-light text-[#041024]">{ heading }</h1>
            <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/55">{ body }</p>
            if sealed {
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
        let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
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

    fn field(id: &str, kind: &str, required: bool) -> crate::model::SignerField {
        crate::model::SignerField {
            id: id.into(),
            field_type: kind.into(),
            field_key: format!("{kind}-{id}"),
            page_number: 5,
            required,
            ..Default::default()
        }
    }

    fn loaded(session: SignerSession) -> (Model, ScreenCtx) {
        let ctx = ScreenCtx::default();
        let (mut model, _) = SignDocument::init(&ctx);
        model.token = "token-abc".into();
        model.session = Remote::Loaded(session);
        (model, ctx)
    }

    fn art() -> SignatureArt {
        SignatureArt {
            signature: "data:image/png;base64,SIG".into(),
            initials: "data:image/png;base64,INI".into(),
        }
    }

    #[test]
    fn one_button_signs_everything_and_asks_for_the_drawing_first() {
        let mut session = test_session(false);
        session.fields = vec![
            field("s", "signature", true),
            field("i", "initials", true),
            field("d", "date", true),
        ];
        let (mut model, ctx) = loaded(session);

        // Not agreed yet: the button says why instead of sending anything.
        let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
        assert!(cmd.into_requests().is_empty());
        assert!(model.notice.as_deref().unwrap().contains("agree"));
        assert!(!model.working);

        // Agreed: the first thing that happens is the picture is drawn (a browser command, not a request).
        model.consent = true;
        let cmd = SignDocument::update(&mut model, Msg::SignAndComplete, &ctx);
        assert!(model.working);
        assert!(
            format!("{cmd:?}").contains("RenderSignature(Ada, style 0)"),
            "{cmd:?}"
        );

        // The picture arrives: agree, then every block in order, then finish — queued as ONE chain.
        let cmd = SignDocument::update(&mut model, Msg::SignatureReady(Ok(art())), &ctx);
        assert_eq!(
            model.queue,
            vec![
                Step::Field("s".into()),
                Step::Field("i".into()),
                Step::Field("d".into()),
                Step::Complete
            ],
            "consent is the first request, already in flight"
        );
        let first = cmd.into_requests().remove(0);
        assert_eq!(first.path, "/v1/signer/consent");

        // Each success sends the next one, until the chain ends with complete and a re-read.
        let ok = serde_json::json!({ "outcome": "success" });
        let mut paths = Vec::new();
        for _ in 0..4 {
            let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(ok.clone())), &ctx);
            paths.push(cmd.into_requests().remove(0).path);
        }
        assert_eq!(
            paths,
            vec![
                "/v1/signer/field",
                "/v1/signer/field",
                "/v1/signer/field",
                "/v1/signer/complete"
            ]
        );
        let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(ok)), &ctx);
        assert_eq!(
            cmd.into_requests().remove(0).path,
            "/v1/signer/session",
            "then the page re-reads the server's truth"
        );
    }

    #[test]
    fn a_refusal_stops_the_chain_and_says_why() {
        let mut session = test_session(true);
        session.fields = vec![field("s", "signature", true)];
        let (mut model, ctx) = loaded(session);
        model.queue = vec![Step::Complete];
        model.working = true;
        let refused =
            serde_json::json!({ "outcome": "rejected", "error": { "message": "Not your turn." } });
        let cmd = SignDocument::update(&mut model, Msg::Acted(Ok(refused)), &ctx);
        assert!(cmd.into_requests().is_empty());
        assert!(model.queue.is_empty() && !model.working);
        assert_eq!(model.notice.as_deref(), Some("Not your turn."));
    }

    #[test]
    fn the_signature_and_initials_travel_as_the_pictures_drawn_and_the_date_needs_none() {
        let mut session = test_session(true);
        session.fields = vec![
            field("s", "signature", true),
            field("i", "initials", true),
            field("d", "date", true),
        ];
        let (mut model, _) = loaded(session.clone());
        model.art = art();
        model.signature_style = 2;
        let value =
            |id: &str| field_value(session.fields.iter().find(|f| f.id == id).unwrap(), &model);
        assert_eq!(value("s")["image"], "data:image/png;base64,SIG");
        assert_eq!(value("s")["style"], 2);
        assert_eq!(value("i")["initialsImage"], "data:image/png;base64,INI");
        assert_eq!(value("d"), serde_json::json!({ "auto": true }));
    }

    #[test]
    fn a_required_box_the_signer_must_fill_blocks_the_button_until_it_is() {
        let mut session = test_session(true);
        session.fields = vec![field("t", "text", true), field("s", "signature", true)];
        let (mut model, _) = loaded(session.clone());
        assert!(sign_blocker(&model, &session).unwrap().contains("complete"));
        model.values.insert("t".into(), "Casa del Mar".into());
        assert_eq!(sign_blocker(&model, &session), None);
        let mut waiting = session;
        waiting.is_turn = false;
        assert!(sign_blocker(&model, &waiting)
            .unwrap()
            .contains("earlier signer"));
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
