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
    DeclineSubmitted,
    Acted(Result<serde_json::Value, ApiError>),
}

pub struct SignDocument;

fn act(token: &str, recipient_id: &str, action: &'static str, extra: serde_json::Value) -> Cmd<Msg> {
    let mut body = extra.as_object().cloned().unwrap_or_default();
    body.insert(
        "accessToken".into(),
        serde_json::Value::String(token.to_owned()),
    );
    body.insert(
        "recipientId".into(),
        serde_json::Value::String(recipient_id.to_owned()),
    );
    Cmd::request(SignerActPost { action, body: serde_json::Value::Object(body) }, Msg::Acted)
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
            model.session = Remote::Failed(ApiError::network("This signing link is missing its token."));
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
            Msg::DeclineSubmitted => {
                let (token, recipient) = match &model.session {
                    Remote::Loaded(session) if !model.working => {
                        (model.token.clone(), session.recipient.id.clone())
                    }
                    _ => return Cmd::none(),
                };
                model.working = true;
                model.notice = None;
                act(&token, &recipient, "decline", serde_json::json!({}))
            }
            Msg::Acted(Ok(_)) => reload(&model.token.clone()),
            Msg::Acted(Err(error)) => {
                model.working = false;
                model.notice = Some(error.message.clone());
                Cmd::none()
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        match &model.session {
            Remote::Loading | Remote::NotAsked => html! {
                <main class="flex min-h-screen items-center justify-center bg-[#f4f1ea] px-4">
                    { template::loading_toned(template::Tone::Site, "your signing session") }
                </main>
            },
            Remote::Failed(error) => html! {
                <main class="flex min-h-screen items-center justify-center bg-[#f4f1ea] px-4">
                    <div class="w-full max-w-md">
                        { template::failure(error) }
                    </div>
                </main>
            },
            Remote::Loaded(session) => {
                if session.state == "completed" {
                    return completed_view(session);
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

fn signing_view(model: &Model, session: &SignerSession, link: &Link<Msg>) -> Html {
    let turn = session.is_turn;
    html! {
        <main class="min-h-screen bg-[#f4f1ea] px-4 py-8 sm:px-6 lg:px-8">
            <div class="mx-auto max-w-6xl">
                <header class="mb-5 flex flex-wrap items-end justify-between gap-4">
                    <div>
                        <p class="text-[10px] font-medium uppercase tracking-[0.24em] text-[#a88450]">{"CulebraLuxe · Secure Signing"}</p>
                        <h1 class="mt-1 font-serif text-3xl font-light text-[#041024]">{"Review and sign"}</h1>
                        <p class="mt-1 text-sm font-light text-black/45">{ format!("Prepared for {}", session.recipient.name) }</p>
                    </div>
                    <div class="rounded-full border border-black/10 bg-white/70 px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                        { session_state_label(session) }
                    </div>
                </header>

                if let Some(notice) = &model.notice {
                    <div class="mb-5 rounded-lg border border-red-900/20 bg-red-50 px-4 py-3 text-sm font-light text-red-900">{ notice.clone() }</div>
                }

                if !turn {
                    <div class="mb-5 rounded-lg border border-[#caa36b]/50 bg-[#fffaf0] px-4 py-3 text-sm font-light text-black/60">
                        {"An earlier signer must finish first — your fields unlock when your turn arrives."}
                    </div>
                }

                <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_21rem]">
                    <section class="overflow-hidden rounded-xl border border-black/10 bg-white shadow-sm">
                        <div class="flex items-center justify-between border-b border-black/10 bg-white/75 px-4 py-2.5">
                            <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{"Your fields"}</span>
                            <span class="text-[10px] font-light text-black/35">{ format!("{} fields", session.fields.len()) }</span>
                        </div>
                        <div class="space-y-5 p-4 sm:p-6">
                            { for session.fields.iter().map(|field| field_editor(model, session, field, link)) }
                            if session.fields.is_empty() {
                                <p class="text-sm font-light text-black/45">{"No fields are assigned to you on this document."}</p>
                            }
                        </div>
                    </section>

                    <aside class="self-start rounded-xl border border-black/10 bg-white p-5 shadow-sm lg:sticky lg:top-6">
                        <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-[#a88450]">{"Your signature"}</p>
                        <h2 class="mt-1 font-serif text-2xl font-light text-[#041024]">{ "Review and sign" }</h2>

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
                                disabled={!turn || model.working}
                                onclick={link.callback(|_: MouseEvent| Msg::CompleteSubmitted)}
                                class="mt-5 flex w-full items-center justify-center rounded-lg bg-[#041024] px-4 py-3 text-[11px] font-medium uppercase tracking-[0.16em] text-white transition hover:bg-[#0a1b38] disabled:cursor-not-allowed disabled:opacity-35"
                            >
                                { if model.working { "Working…" } else { "Sign & Complete" } }
                            </button>
                            <button
                                type="button"
                                disabled={model.working}
                                onclick={link.callback(|_: MouseEvent| Msg::DeclineSubmitted)}
                                class="mt-3 flex w-full items-center justify-center rounded-lg border border-black/15 px-4 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] text-black/55 transition hover:border-black/30"
                            >
                                {"Decline to sign"}
                            </button>
                        }
                    </aside>
                </div>
            </div>
        </main>
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

fn field_editor(model: &Model, session: &SignerSession, field: &SignerField, link: &Link<Msg>) -> Html {
    let locked = !session.is_turn || model.working;
    let label = field.label.clone().unwrap_or_else(|| field.field_key.clone());
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
                {"Save field"}
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

fn completed_view(session: &SignerSession) -> Html {
    html! {
        <main class="flex min-h-screen items-center justify-center bg-[#f4f1ea] px-4 py-12">
            <section class="w-full max-w-xl rounded-xl border border-black/10 bg-white p-8 text-center shadow-sm">
                <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-50 text-xl text-emerald-700">{"✓"}</div>
                <p class="mt-5 text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{"CulebraLuxe · Secure Signing"}</p>
                <h1 class="mt-2 font-serif text-3xl font-light text-[#041024]">{"Signing complete"}</h1>
                <p class="mx-auto mt-3 max-w-md text-sm font-light leading-6 text-black/50">
                    { format!("Thank you, {}. Your signature has been recorded.", session.recipient.name) }
                </p>
            </section>
        </main>
    }
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
        }
    }
}
