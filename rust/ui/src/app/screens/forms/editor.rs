//! The Forms workspace view: the open form, its sections and fields, the preview and the actions.

#[allow(unused_imports)]
use super::*;

pub(super) fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
    if model.loading {
        return crate::app::template::loading_panel("Forms");
    }
    let Some(page) = model.page.as_ref() else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                { error_band(model.error.as_deref()) }
            </section>
        };
    };
    let (Some(form), Some(template)) = (page.selected.as_ref(), page.template.as_ref()) else {
        return empty_forms_view(page, model, link);
    };

    let working = model.busy;
    let status_cue = if model.draft_saving {
        "Saving…".to_owned()
    } else if model.dirty {
        "Unsaved".to_owned()
    } else if let Some(issued) = page.issued.as_ref() {
        format!("Vault v{}", issued.issued_version)
    } else if form.status == "issued" {
        "Issued".into()
    } else {
        "Draft".into()
    };
    let is_listing = template.id == "LISTING-01";
    let listing_is_active = template.version == template.active_version;
    let listing_locked = page.issued.is_some() || form.status == "issued";
    let signature_active = page
        .signature
        .as_ref()
        .is_some_and(|signature| matches!(signature.status.as_str(), "requested" | "sent" | "viewed" | "signed"));
    let status_text = model
        .error
        .as_deref()
        .or(model.message.as_deref())
        .unwrap_or(&status_cue);
    let tone_dot = if model.error.is_some() {
        "bg-[var(--portal-archive)]"
    } else if model.message.is_some() {
        "bg-[var(--portal-success)]"
    } else if model.dirty || model.draft_saving {
        "bg-[var(--portal-gold)]"
    } else {
        "bg-black/25"
    };

    let grok_changed = link.callback(|event: InputEvent| {
        Msg::GrokPromptChanged(
            crate::app::exec::input_value(&event),
        )
    });
    let grok_key = link.callback(|event: KeyboardEvent| {
        if event.key() == "Enter" {
            event.prevent_default();
            Msg::GrokGo
        } else {
            Msg::GrokPromptChanged(
                crate::app::exec::input_value(&event),
            )
        }
    });
    let grok_go = link.callback(|_: MouseEvent| Msg::GrokGo);
    let mic = link.callback(|_: MouseEvent| Msg::MicPressed);

    html! {
        <div class="flex min-h-0 flex-col gap-3">
            <div class="grid grid-cols-1 gap-3 lg:grid-cols-[220px_minmax(0,1fr)_minmax(0,1fr)] lg:gap-4">
                <div class="min-w-0 lg:col-span-2">
                    <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-4 py-3 sm:px-5">
                        <div class="flex flex-wrap items-end gap-2">
                            <div class="min-w-0 flex-1">
                                <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
                                    { format!("Grok · {}", template.display_name) }
                                </p>
                                <input
                                    value={model.grok_prompt.clone()}
                                    oninput={grok_changed}
                                    onkeydown={grok_key}
                                    placeholder="Tell Grok what happened — or tap the mic"
                                    class="mt-1.5 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                                />
                            </div>
                            <button
                                type="button"
                                onclick={mic}
                                disabled={model.listening}
                                title={if model.listening { "Listening…" } else { "Speak to Grok" }}
                                class={classes!(
                                    "inline-flex", "h-10", "w-10", "items-center", "justify-center", "rounded-[var(--portal-tab-radius)]", "border", "transition",
                                    if model.listening {
                                        "border-red-400 bg-red-50 text-red-600 animate-pulse"
                                    } else {
                                        "border-[var(--portal-panel-border)] text-[var(--portal-navy)] hover:border-[var(--portal-navy)]"
                                    }
                                )}
                                aria-label="Use microphone to command Grok"
                            >
                                {"◉"}
                            </button>
                            <button
                                type="button"
                                disabled={working || model.grok_working}
                                onclick={grok_go}
                                class="inline-flex h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
                            >
                                {"Go"}
                            </button>
                        </div>
                        <p class="mt-2 text-center text-[11px] font-light text-black/45">
                            {"Say it like you would to Grok. She can fill the fields; you still Save and Send."}
                        </p>
                    </section>
                </div>

                <section
                    aria-label="Status"
                    class="portal-glass-panel portal-glass-panel-lifted flex h-full min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]"
                >
                    <div class="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                        <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">
                            {"Status"}
                        </p>
                        <span aria-hidden="true" class={format!("h-2 w-2 shrink-0 rounded-full {tone_dot}")} />
                    </div>
                    <div
                        aria-live="polite"
                        class="min-h-0 flex-1 overflow-hidden px-4 py-2.5 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] line-clamp-3"
                    >
                        <div>
                            <div>{ status_text }</div>
                            {
                                if is_listing {
                                    html! {
                                        <div class="mt-0.5 text-[10px] font-light text-black/40">
                                            {
                                                format!(
                                                    "v{} {} · {}{}",
                                                    template.version,
                                                    if listing_is_active { "active" } else { "history" },
                                                    if form.person_id.is_some() { "Client linked" } else { "Client not linked" },
                                                    if listing_locked { " · issued snapshot" } else { " · auto-hydrated" },
                                                )
                                            }
                                        </div>
                                    }
                                } else {
                                    Html::default()
                                }
                            }
                        </div>
                    </div>
                </section>
            </div>

            <div class="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-12.5rem)] lg:grid-cols-[220px_minmax(0,1fr)_minmax(0,1fr)]">
                { forms_rail(model, page, form, template, link) }

                <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                    <div class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-4 py-2.5">
                        <div class="min-w-0">
                            <div class="flex min-w-0 items-center gap-2">
                                <h2 class="truncate font-serif text-lg font-light text-[var(--portal-navy)]">
                                    { format!("{}{}", template.display_name, if is_listing { format!(" · v{}", template.version) } else { String::new() }) }
                                </h2>
                                {
                                    if is_listing {
                                        html! {
                                            <span class={format!(
                                                "shrink-0 rounded-full border px-2 py-0.5 text-[8px] font-medium uppercase tracking-[0.13em] {}",
                                                if listing_is_active {
                                                    "border-[var(--portal-gold)]/55 text-[var(--portal-navy)]"
                                                } else {
                                                    "border-black/15 text-black/40"
                                                }
                                            )}>
                                                { if listing_is_active { "Active" } else { "History" } }
                                            </span>
                                        }
                                    } else {
                                        Html::default()
                                    }
                                }
                            </div>
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                            <button
                                type="button"
                                disabled={working || !ctx.can("vault.issue")}
                                onclick={link.callback(|_: MouseEvent| Msg::SavePdf)}
                                class={PRIMARY_BUTTON}
                            >
                                { if working { "Working…" } else { "Save" } }
                            </button>
                            <button
                                type="button"
                                disabled={working || model.preview_uri.is_none()}
                                onclick={link.callback(|_: MouseEvent| Msg::Share)}
                                class={GHOST_BUTTON}
                            >
                                {"Share"}
                            </button>
                            <button
                                type="button"
                                disabled={working || signature_active || !ctx.can("signature.write")}
                                onclick={link.callback(|_: MouseEvent| Msg::SendSignature)}
                                class={GHOST_BUTTON}
                            >
                                {
                                    if signature_active {
                                        "Sent for signature"
                                    } else {
                                        "Send BoldSign"
                                    }
                                }
                            </button>
                            <button
                                type="button"
                                disabled={working || !model.dirty}
                                onclick={link.callback(|_: MouseEvent| Msg::Cancel)}
                                class={GHOST_BUTTON}
                            >
                                {"Cancel"}
                            </button>
                        </div>
                    </div>

                    {
                        page.signature.as_ref().map(|signature| html! {
                            <p class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-2 text-[11px] font-light text-black/45">
                                { format!("Signature · {}", signature.status) }
                            </p>
                        }).unwrap_or_default()
                    }

                    <div class="min-h-0 flex-1 overflow-y-auto p-5">
                        {
                            if is_listing {
                                html! {
                                    <div class="mb-2 flex items-center justify-between gap-2">
                                        <span class="text-[10px] font-light text-black/40">
                                            { if form.person_id.is_some() { "Seller linked to Client" } else { "Select the seller Client" } }
                                        </span>
                                        <button
                                            type="button"
                                            disabled={working || listing_locked}
                                            onclick={link.callback(|_: MouseEvent| Msg::FillClient)}
                                            class="inline-flex min-h-7 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-35"
                                            title="Fill this Listing from the seller Client"
                                        >
                                            { if working { "Filling…" } else { "Fill Client" } }
                                        </button>
                                    </div>
                                }
                            } else {
                                Html::default()
                            }
                        }

                        <div class="grid grid-cols-6 items-end gap-x-3 gap-y-3.5">
                            {
                                for template
                                    .fields
                                    .iter()
                                    .filter(|field| when_visible(field.when.as_ref(), &model.values))
                                    .map(|field| field_control(field, &model.values, link))
                            }
                        </div>

                        <div class="mt-6">
                            <h2 class="font-serif text-base font-bold text-[var(--portal-navy)]">{"Document"}</h2>
                            <p class="mt-1 text-xs font-light text-black/40">
                                {"Template text from the form. Edit it like a Word document."}
                            </p>
                            <textarea
                                id="deal-details"
                                rows={14}
                                value={model.details_text.clone()}
                                placeholder="Document text…"
                                oninput={link.callback(|event: InputEvent| {
                                    Msg::DetailsChanged(
                                        crate::app::exec::textarea_value(&event),
                                    )
                                })}
                                class="mt-2 block min-h-[16rem] w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/80 px-3 py-2.5 font-serif text-[15px] font-light leading-7 text-black/80 outline-none focus:border-[var(--portal-navy-soft)] disabled:opacity-60"
                            />
                        </div>
                    </div>
                </section>

                <section class="portal-glass-panel min-h-0 overflow-hidden rounded-[var(--portal-panel-radius)]">
                    <div class="relative h-full min-h-[34rem] bg-[var(--portal-blue-pale)]/55 p-3 lg:p-4">
                        {
                            if let Some(preview) = model.preview_uri.as_ref() {
                                html! {
                                    <iframe
                                        title="Exact PDF preview"
                                        src={preview.clone()}
                                        class="h-full min-h-[32rem] w-full rounded-sm bg-white shadow-[0_12px_36px_rgba(24,43,64,0.14)] ring-1 ring-black/[0.06]"
                                    />
                                }
                            } else if model.preview_loading {
                                html! {
                                    <div class="flex h-full min-h-[32rem] items-center justify-center bg-white text-sm font-light text-black/45">
                                        {"Building exact PDF preview…"}
                                    </div>
                                }
                            } else {
                                html! {
                                    <div class="flex h-full min-h-[32rem] items-center justify-center bg-white px-8 text-center text-sm font-light text-black/45">
                                        {"PDF preview will appear here."}
                                    </div>
                                }
                            }
                        }
                        {
                            if model.preview_loading && model.preview_uri.is_some() {
                                html! {
                                    <span class="absolute right-6 top-6 rounded-full bg-[var(--portal-navy)]/85 px-3 py-1 text-[10px] uppercase tracking-[0.12em] text-white shadow-sm">
                                        {"Updating PDF…"}
                                    </span>
                                }
                            } else {
                                Html::default()
                            }
                        }
                    </div>
                </section>
            </div>
        </div>
    }
}
