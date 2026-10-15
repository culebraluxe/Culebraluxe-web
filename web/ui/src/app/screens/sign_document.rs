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

mod components;
use components::*;

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
    /// The style `art` was drawn in, so pressing "Sign & complete" does not redraw what the preview already drew.
    art_style: Option<usize>,
    /// The signature page as it will be sealed (a `blob:` address), drawn with the signer's own picture.
    preview_url: Option<String>,
    previewing: bool,
    /// The preview could not be drawn: the page falls back to the full document rather than showing nothing.
    preview_failed: bool,
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
    /// The signature was drawn for the preview (and is kept for signing); the style says which request this answers.
    PreviewArt(usize, Result<SignatureArt, ApiError>),
    PreviewReady(usize, Result<String, ApiError>),
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
                // Record the open once, when the session first arrives, and draw the signature page the signer will see.
                if first_sight {
                    return Cmd::batch(vec![
                        act(&token, &recipient, "open", serde_json::json!({})),
                        refresh_preview(model),
                    ]);
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
                refresh_preview(model)
            }
            Msg::PreviewArt(style, Ok(art)) if style == model.signature_style => {
                model.art = art.clone();
                model.art_style = Some(style);
                Cmd::preview_pdf(
                    "/v1/signer/preview",
                    serde_json::json!({
                        "accessToken": model.token,
                        "image": art.signature,
                    }),
                    move |result| Msg::PreviewReady(style, result),
                )
            }
            Msg::PreviewArt(_, Ok(_)) => Cmd::none(), // an answer for a style no longer chosen
            Msg::PreviewArt(style, Err(_)) => {
                if style == model.signature_style {
                    model.previewing = false;
                    model.preview_failed = true;
                }
                Cmd::none()
            }
            Msg::PreviewReady(style, result) => {
                if style == model.signature_style {
                    model.previewing = false;
                    match result {
                        Ok(url) => {
                            model.preview_url = Some(url);
                            model.preview_failed = false;
                        }
                        Err(_) => model.preview_failed = true,
                    }
                }
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
                if model.art_style == Some(style) {
                    // The preview already drew this signature: sign with exactly that picture.
                    let art = model.art.clone();
                    return Self::update(model, Msg::SignatureReady(Ok(art)), _ctx);
                }
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

/// The value to send for one field. A signature carries the picture drawn for this signing (the very pixels sealed into
/// the document); initials and the date need no picture (the seal typesets them like the brokerage's).
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
        // Initials are typeset by the document from the name, like the brokerage's: no picture.
        "initials" => serde_json::json!({
            "style": model.signature_style,
            "name": name,
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

/// Draw the signature in the chosen style (answered as `PreviewArt`, then the preview PDF is requested with it).
fn refresh_preview(model: &mut Model) -> Cmd<Msg> {
    let Remote::Loaded(session) = &model.session else {
        return Cmd::none();
    };
    let (name, style) = (session.recipient.name.clone(), model.signature_style);
    model.previewing = true;
    Cmd::render_signature(name, style, move |result| Msg::PreviewArt(style, result))
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
                        {"Luxesign"}
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
            <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{ name.unwrap_or_else(|| "Luxesign".to_owned()) }</p>
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
mod tests;
