//! The signing screen's drawing: the signing view and the field/signature rendering it is made of, moved out
//! of `sign_document.rs` on 2026-10-14 to keep that file under the boundedness ceiling. Move-only apart from
//! `pub(super)` on the moved functions and the `use super::*;` below — the parent keeps calling them
//! unqualified through its own glob import, so nothing in the state machine changes.
//!
//! They read the session the parent's state machine keeps and draw it. What a signer may do — consent, field,
//! complete, decline — stays in `sign_document.rs`, where the edge commands are answered.

use super::*;

/// The signing page: the ACTION on the left, the signature page as it will be sealed on the right. Most signers have read
/// the document already and are here for the last step, so the page opens on the page they sign, with their own
/// signature drawn in place, rather than at the top of a long document.
pub(super) fn signing_view(model: &Model, session: &SignerSession, link: &Link<Msg>) -> Html {
    let turn = session.is_turn;
    let heading = session
        .document_name()
        .map(str::to_owned)
        .unwrap_or_else(|| "Review and sign".to_owned());
    let message = session
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty());
    page(html! {
        <>
            if let Some(notice) = &model.notice {
                <div class="mb-5 rounded-lg border border-red-900/20 bg-red-50 px-4 py-3 text-sm font-light text-red-900" role="alert">{ notice.clone() }</div>
            }
            <div class="grid gap-5 md:grid-cols-[22rem_minmax(0,1fr)]">
                <aside class="self-start rounded-xl border border-black/10 bg-white p-5 shadow-sm md:sticky md:top-6">
                    <p class="text-[10px] font-medium uppercase tracking-[0.2em] text-[#a88450]">{"Ready to sign"}</p>
                    <h1 class="mt-1 break-words font-serif text-[1.7rem] font-light leading-8 text-[#041024]">{ heading }</h1>
                    <p class="mt-1 text-sm font-light text-black/50">{ prepared_for(session) }</p>

                    if let Some(message) = message {
                        <p class="mt-4 rounded-lg border border-[#caa36b]/40 bg-[#fffaf0] px-3 py-2.5 text-sm font-light leading-5 text-black/65">
                            { format!("“{message}”") }
                        </p>
                    }
                    if !turn {
                        <p class="mt-4 rounded-lg border border-[#caa36b]/50 bg-[#fffaf0] px-3 py-2.5 text-sm font-light text-black/60">
                            {"An earlier signer must finish first — you can sign when your turn arrives."}
                        </p>
                    }

                    <p class="mt-5 text-[9px] font-medium uppercase tracking-[0.14em] text-black/35">{"You will sign"}</p>
                    <ul class="mt-2 divide-y divide-black/10 rounded-lg border border-black/10">
                        { for session.fields.iter().filter(|field| !is_hand_filled(field)).map(|field| block_row(session, field)) }
                    </ul>
                    <div class="mt-3 space-y-3">
                        { for session.fields.iter().filter(|field| is_hand_filled(field)).map(|field| hand_field(model, session, field, link)) }
                    </div>

                    <p class="mt-5 text-[9px] font-medium uppercase tracking-[0.14em] text-black/35">{"Your signature"}</p>
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

                { document_panel(model, session) }
            </div>
        </>
    })
}

/// The right-hand side: the signature page as it will be sealed (the signer's own picture, initials and today's date in
/// place), zoomed to the page's width. If it cannot be drawn the full document stands in, so there is always something
/// to read.
pub(super) fn document_panel(model: &Model, _session: &SignerSession) -> Html {
    let full = document_url(&model.token);
    let (title, source) = match (&model.preview_url, model.preview_failed) {
        (Some(url), _) => (
            "Your signature page, as it will look",
            Some(format!("{url}#toolbar=0&navpanes=0&view=FitH")),
        ),
        (None, true) => ("The document", Some(format!("{full}#navpanes=0&view=FitH"))),
        (None, false) => ("Your signature page", None),
    };
    html! {
        <section class="min-w-0 overflow-hidden rounded-xl border border-black/10 bg-white shadow-sm">
            <div class="flex flex-wrap items-center justify-between gap-2 border-b border-black/10 bg-white/75 px-4 py-2.5">
                <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-black/45">{ title }</span>
                <span class="flex gap-4 text-[11px] font-light">
                    <a class="text-[#041024] underline decoration-black/20 underline-offset-2" href={full.clone()} target="_blank" rel="noopener">{"Read the full document"}</a>
                    <a class="text-[#041024] underline decoration-black/20 underline-offset-2" href={format!("{full}?download=1")}>{"Download"}</a>
                </span>
            </div>
            if let Some(source) = source {
                <iframe class="block h-[82vh] min-h-[34rem] w-full bg-[#f4f1ea]" title="The page you are signing" src={source}></iframe>
            } else {
                <div class="flex h-[82vh] min-h-[34rem] items-center justify-center bg-[#f4f1ea] text-sm font-light text-black/40" data-screen-state="loading">
                    {"Drawing your signature page…"}
                </div>
            }
        </section>
    }
}

/// "Decline to sign" is a deliberate two-step: it ends the signing for everyone, so it asks first, and lets the signer
/// say why (the sender is told).
pub(super) fn decline_panel(model: &Model, link: &Link<Msg>) -> Html {
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

/// The CSS for the cursive face at `style` (an inline style: the family names are declared in `app.css`).
pub(super) fn signature_face(style: usize) -> String {
    let family = SIGNATURE_FONTS
        .get(style)
        .map(|(family, _)| *family)
        .unwrap_or(SIGNATURE_FONTS[0].0);
    format!("font-family:'{family}',cursive;")
}

/// One block the page signs for the signer: what it is, where, and whether it is done.
pub(super) fn block_row(session: &SignerSession, field: &SignerField) -> Html {
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
pub(super) fn hand_field(
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
pub(super) fn signature_choice(
    name: &str,
    selected: usize,
    style: usize,
    link: &Link<Msg>,
) -> Html {
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
