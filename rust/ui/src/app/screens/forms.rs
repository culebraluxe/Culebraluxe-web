//! CORE — the mature Forms workspace, ported from the legacy FormEditor to Yew/MVI.
//!
//! The screen owns interaction state only. Reads and writes are typed Endpoint -> Cmd::request
//! effects; all persistence and business operations stay in the Rust Forms, Person, Client,
//! Vault and Signature services behind the portal transport.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{
    FormItem, FormPreview, FormPreviewResponse, FormTemplate, FormTemplateField, FormWhen,
    FormsAction, FormsBridgeResponse, FormsPage, FormsRead, FormsWrite, FormsWriteResponse,
};
use crate::app::cmd::{ApiError, Cmd};
use crate::app::screen::{Link, Screen, ScreenCtx};

const PRIMARY_BUTTON: &str =
    "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40";
const GHOST_BUTTON: &str =
    "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:cursor-not-allowed disabled:opacity-40";
const INPUT_CLASS: &str =
    "mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light leading-9 text-black/70 outline-none focus:border-[var(--portal-navy-soft)]";
const LABEL_CLASS: &str =
    "text-[9px] font-light uppercase tracking-[0.14em] text-black/40";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    page: Option<FormsPage>,
    values: BTreeMap<String, String>,
    sections: BTreeMap<String, String>,
    saved_values: BTreeMap<String, String>,
    saved_sections: BTreeMap<String, String>,
    details_text: String,
    saved_details_text: String,
    body_edited: bool,
    selected_template: String,
    session_query: String,
    grok_prompt: String,
    loading: bool,
    busy: bool,
    draft_saving: bool,
    dirty: bool,
    error: Option<String>,
    message: Option<String>,
    preview_uri: Option<String>,
    preview_filename: String,
    preview_loading: bool,
    generation: u32,
}

#[derive(Debug)]
pub enum Msg {
    ListLoaded(Result<FormsBridgeResponse, ApiError>),
    RecordLoaded(Result<FormsBridgeResponse, ApiError>),
    OpenForm(String),
    SessionQueryChanged(String),
    TemplateSelected(String),
    NewForm,
    Created(Result<FormsWriteResponse, ApiError>),
    FieldChanged { name: String, value: String },
    DetailsChanged(String),
    AutosaveDue(u32),
    DraftSaved {
        generation: u32,
        result: Result<FormsWriteResponse, ApiError>,
    },
    PreviewDue(u32),
    Previewed(Result<FormPreviewResponse, ApiError>),
    SavePdf,
    Issued(Result<FormsWriteResponse, ApiError>),
    FillClient,
    ClientFilled(Result<FormsWriteResponse, ApiError>),
    SendSignature,
    SignatureSent(Result<FormsWriteResponse, ApiError>),
    Share,
    Shared(Result<(), ApiError>),
    Cancel,
    GrokPromptChanged(String),
    GrokGo,
}

pub struct Forms;
pub struct FormRecord;

impl Screen for Forms {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                loading: true,
                ..Model::default()
            },
            Cmd::request(FormsRead::list(None, None, None), Msg::ListLoaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        view(model, ctx, link)
    }
}

impl Screen for FormRecord {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let Some(id) = ctx.id.clone() else {
            return (
                Model {
                    error: Some("Form id is missing.".into()),
                    ..Model::default()
                },
                Cmd::none(),
            );
        };
        (
            Model {
                loading: true,
                ..Model::default()
            },
            Cmd::request(FormsRead::record(id), Msg::RecordLoaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg, ctx)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        view(model, ctx, link)
    }
}

fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
    match msg {
        Msg::ListLoaded(result) => {
            model.loading = false;
            match result {
                Ok(answer) => {
                    let page = answer.forms;
                    let preferred = preferred_form_id(&page);
                    model.page = Some(page);
                    model.error = None;
                    if let Some(id) = preferred {
                        model.loading = true;
                        return Cmd::request(FormsRead::record(id), Msg::RecordLoaded);
                    }
                }
                Err(error) => model.error = Some(error.message),
            }
            Cmd::none()
        }
        Msg::RecordLoaded(result) => {
            model.loading = false;
            match result {
                Ok(answer) => install_record(model, answer.forms, None),
                Err(error) => {
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::OpenForm(id) => {
            if model.dirty || model.draft_saving {
                model.error =
                    Some("Save is still settling. Wait a moment before changing forms.".into());
                Cmd::none()
            } else {
                model.error = None;
                Cmd::request(FormsRead::record(id), Msg::RecordLoaded)
            }
        }
        Msg::SessionQueryChanged(value) => {
            model.session_query = value;
            Cmd::none()
        }
        Msg::TemplateSelected(template_id) => {
            if template_id == model.selected_template {
                return Cmd::none();
            }
            model.selected_template = template_id.clone();
            let Some(page) = model.page.as_ref() else {
                return Cmd::none();
            };
            if let Some(existing) = page
                .items
                .iter()
                .find(|item| item.template_id == template_id)
            {
                if model.dirty || model.draft_saving {
                    model.error =
                        Some("Save is still settling. Wait a moment before changing form type.".into());
                    return Cmd::none();
                }
                model.error = None;
                return Cmd::request(
                    FormsRead::record(existing.id.clone()),
                    Msg::RecordLoaded,
                );
            }
            create_form(model, &template_id)
        }
        Msg::NewForm => {
            let template_id = current_template_id(model);
            create_form(model, &template_id)
        }
        Msg::Created(result) => {
            model.busy = false;
            match result {
                Ok(answer) => install_record(model, answer.forms, Some("New form".into())),
                Err(error) => {
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::FieldChanged { name, value } => {
            model.values.insert(name, value);
            if !model.body_edited {
                if let Some(template) = model.page.as_ref().and_then(|page| page.template.as_ref()) {
                    model.details_text = document_body_text(template, &model.values, &model.sections);
                }
            }
            local_edit(model)
        }
        Msg::DetailsChanged(value) => {
            model.details_text = value;
            model.body_edited = true;
            local_edit(model)
        }
        Msg::AutosaveDue(generation) => {
            if generation != model.generation || !model.dirty || model.busy || model.draft_saving {
                return Cmd::none();
            }
            let Some(form_id) = current_form_id(model) else {
                return Cmd::none();
            };
            model.draft_saving = true;
            Cmd::request(
                FormsWrite {
                    action: FormsAction::Save {
                        form_id,
                        field_values: model.values.clone(),
                        sections: composed_sections(model),
                    },
                },
                move |result| Msg::DraftSaved { generation, result },
            )
        }
        Msg::DraftSaved { generation, result } => {
            model.draft_saving = false;
            match result {
                Ok(answer) => {
                    if generation == model.generation {
                        model.saved_values = model.values.clone();
                        model.saved_sections = composed_sections(model);
                        model.saved_details_text = model.details_text.clone();
                        model.dirty = false;
                    }
                    model.page = Some(answer.forms);
                    model.error = None;
                }
                Err(error) => model.error = Some(error.message),
            }
            Cmd::none()
        }
        Msg::PreviewDue(generation) => {
            if generation != model.generation {
                return Cmd::none();
            }
            request_preview(model)
        }
        Msg::Previewed(result) => {
            model.preview_loading = false;
            match result {
                Ok(answer) => {
                    model.preview_uri = Some(answer.data_uri);
                    model.preview_filename = answer.filename;
                }
                Err(error) => model.error = Some(error.message),
            }
            Cmd::none()
        }
        Msg::SavePdf => {
            let Some(form_id) = current_form_id(model) else {
                return Cmd::none();
            };
            model.busy = true;
            model.error = None;
            model.message = None;
            Cmd::request(
                FormsWrite {
                    action: FormsAction::Issue {
                        form_id,
                        field_values: model.values.clone(),
                        sections: composed_sections(model),
                    },
                },
                Msg::Issued,
            )
        }
        Msg::Issued(result) => {
            model.busy = false;
            match result {
                Ok(answer) => {
                    let fallback = answer
                        .forms
                        .issued
                        .as_ref()
                        .map(|issued| format!("Saved to vault v{}", issued.issued_version))
                        .unwrap_or_else(|| "Saved to vault".into());
                    let message = answer.message.clone().unwrap_or(fallback);
                    install_record(model, answer.forms, Some(message))
                }
                Err(error) => {
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::FillClient => {
            let Some(form_id) = current_form_id(model) else {
                return Cmd::none();
            };
            let seller_name = model
                .values
                .get("sellerName")
                .cloned()
                .unwrap_or_default();
            if seller_name.trim().is_empty() {
                model.error = Some("Enter the seller name first.".into());
                return Cmd::none();
            }
            model.busy = true;
            model.error = None;
            Cmd::request(
                FormsWrite {
                    action: FormsAction::FillClient {
                        form_id,
                        seller_name,
                    },
                },
                Msg::ClientFilled,
            )
        }
        Msg::ClientFilled(result) => {
            model.busy = false;
            match result {
                Ok(answer) => {
                    let message = answer
                        .message
                        .clone()
                        .unwrap_or_else(|| "Client linked".into());
                    install_record(model, answer.forms, Some(message))
                }
                Err(error) => {
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::SendSignature => {
            let Some(form_id) = current_form_id(model) else {
                return Cmd::none();
            };
            model.busy = true;
            model.error = None;
            model.message = None;
            Cmd::request(
                FormsWrite {
                    action: FormsAction::SendSignature {
                        form_id,
                        field_values: model.values.clone(),
                        sections: composed_sections(model),
                    },
                },
                Msg::SignatureSent,
            )
        }
        Msg::SignatureSent(result) => {
            model.busy = false;
            match result {
                Ok(answer) => {
                    let message = answer
                        .message
                        .clone()
                        .unwrap_or_else(|| "Sent for signature".into());
                    install_record(model, answer.forms, Some(message))
                }
                Err(error) => {
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::Share => {
            let Some(uri) = model.preview_uri.clone() else {
                model.error = Some("The PDF preview is not ready yet.".into());
                return Cmd::none();
            };
            let filename = if model.preview_filename.trim().is_empty() {
                "CulebraLuxe-Document.pdf".to_owned()
            } else {
                model.preview_filename.clone()
            };
            model.error = None;
            Cmd::share_pdf(uri, filename, Msg::Shared)
        }
        Msg::Shared(result) => {
            match result {
                Ok(()) => {
                    model.message = Some("Shared".into());
                    model.error = None;
                }
                Err(error) => {
                    model.message = Some(
                        "This browser could not attach the PDF to native Share. Save the PDF and attach it in Mail or Messages."
                            .into(),
                    );
                    model.error = Some(error.message);
                }
            }
            Cmd::none()
        }
        Msg::Cancel => {
            model.values = model.saved_values.clone();
            model.sections = model.saved_sections.clone();
            model.details_text = model.saved_details_text.clone();
            model.body_edited = model
                .saved_sections
                .get("bodyEdited")
                .is_some_and(|value| value == "true");
            model.dirty = false;
            model.error = None;
            model.message = Some("Changes discarded".into());
            model.generation = model.generation.wrapping_add(1);
            Cmd::after(0, Msg::PreviewDue(model.generation))
        }
        Msg::GrokPromptChanged(value) => {
            model.grok_prompt = value;
            Cmd::none()
        }
        Msg::GrokGo => {
            if model.grok_prompt.trim().is_empty() {
                model.message = Some("Tell Grok what happened on the deal, then tap Go.".into());
            } else {
                model.error = Some(
                    "Grok form-fill is not exposed by the Rust service catalog yet; no form data was changed."
                        .into(),
                );
            }
            Cmd::none()
        }
    }
}

fn local_edit(model: &mut Model) -> Cmd<Msg> {
    model.dirty = true;
    model.error = None;
    model.message = None;
    model.generation = model.generation.wrapping_add(1);
    let generation = model.generation;
    Cmd::batch([
        Cmd::after(250, Msg::PreviewDue(generation)),
        Cmd::after(900, Msg::AutosaveDue(generation)),
    ])
}

fn install_record(model: &mut Model, page: FormsPage, message: Option<String>) -> Cmd<Msg> {
    let Some(form) = page.selected.as_ref() else {
        model.page = Some(page);
        model.error = Some("The Forms response did not include the selected form.".into());
        return Cmd::none();
    };
    let Some(template) = page.template.as_ref() else {
        model.page = Some(page);
        model.error = Some("The saved form did not include its template.".into());
        return Cmd::none();
    };

    model.values = form.field_values.clone();
    model.sections = form.sections.clone();
    model.details_text = resolve_document_body(template, &model.values, &model.sections);
    model.body_edited = model
        .sections
        .get("bodyEdited")
        .is_some_and(|value| value == "true");
    model.saved_values = model.values.clone();
    model.saved_sections = model.sections.clone();
    model.saved_details_text = model.details_text.clone();
    let path = format!("/portal/forms/{}", form.id);
    model.selected_template = form.template_id.clone();
    model.page = Some(page);
    model.dirty = false;
    model.error = None;
    model.message = message;
    model.loading = false;
    model.generation = model.generation.wrapping_add(1);
    Cmd::batch([request_preview(model), Cmd::replace_path(path)])
}

fn request_preview(model: &mut Model) -> Cmd<Msg> {
    let Some(form_id) = current_form_id(model) else {
        return Cmd::none();
    };
    model.preview_loading = true;
    Cmd::request(
        FormPreview {
            form_id,
            field_values: model.values.clone(),
            sections: composed_sections(model),
        },
        Msg::Previewed,
    )
}

fn create_form(model: &mut Model, template_id: &str) -> Cmd<Msg> {
    if model.busy {
        return Cmd::none();
    }
    let Some(form) = model.page.as_ref().and_then(|page| page.selected.as_ref()) else {
        model.error = Some("Open an existing form before starting another form.".into());
        return Cmd::none();
    };
    model.busy = true;
    model.error = None;
    Cmd::request(
        FormsWrite {
            action: FormsAction::Create {
                template_id: template_id.to_owned(),
                deal_id: form.deal_id.clone(),
                person_id: form.person_id.clone(),
                property_id: form.property_id.clone(),
            },
        },
        Msg::Created,
    )
}

fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
    if model.loading {
        return html! {
            <p class="py-10 text-sm font-light text-black/45">{"Loading Forms…"}</p>
        };
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
            event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value(),
        )
    });
    let grok_key = link.callback(|event: KeyboardEvent| {
        if event.key() == "Enter" {
            event.prevent_default();
            Msg::GrokGo
        } else {
            Msg::GrokPromptChanged(
                event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            )
        }
    });
    let grok_go = link.callback(|_: MouseEvent| Msg::GrokGo);

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
                                disabled={true}
                                title="Voice input stays disabled until the Rust voice service is exposed."
                                class="inline-flex h-10 w-10 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] text-[var(--portal-navy-soft)] opacity-45"
                                aria-label="Use microphone to command Grok"
                            >
                                {"◉"}
                            </button>
                            <button
                                type="button"
                                disabled={working}
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
                                        event
                                            .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                                            .value(),
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

fn forms_rail(
    model: &Model,
    page: &FormsPage,
    form: &FormItem,
    template: &FormTemplate,
    link: &Link<Msg>,
) -> Html {
    let needle = model.session_query.trim().to_lowercase();
    let visible = page
        .items
        .iter()
        .filter(|item| item.template_id == form.template_id)
        .filter(|item| {
            if needle.is_empty() {
                return true;
            }
            session_label(item).to_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    let query_changed = link.callback(|event: InputEvent| {
        Msg::SessionQueryChanged(
            event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value(),
        )
    });
    let template_changed = link.callback(|event: Event| {
        Msg::TemplateSelected(
            event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value(),
        )
    });

    html! {
        <aside class="portal-glass-panel flex max-h-72 min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)] lg:max-h-none">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                <div class="mb-2 flex items-center justify-between gap-2">
                    <span class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Forms · {}", visible.len()) }
                    </span>
                    <button
                        type="button"
                        disabled={model.busy}
                        onclick={link.callback(|_: MouseEvent| Msg::NewForm)}
                        class="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
                    >
                        {"New"}
                    </button>
                </div>
                <input
                    type="search"
                    value={model.session_query.clone()}
                    oninput={query_changed}
                    placeholder="Search…"
                    class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                />
                <select
                    value={model.selected_template.clone()}
                    disabled={model.busy}
                    onchange={template_changed}
                    class="mt-2 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none focus:border-[var(--portal-navy)] disabled:opacity-40"
                >
                    {
                        for page.template_choices.iter().map(|item| html! {
                            <option value={item.id.clone()}>
                                { &item.display_name }
                            </option>
                        })
                    }
                </select>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto">
                {
                    if visible.is_empty() {
                        html! {
                            <p class="px-3 py-6 text-sm font-light text-black/40">
                                { format!("No matching {} forms{}",
                                    template.display_name.to_lowercase(),
                                    if needle.is_empty() { " yet." } else { "." }
                                ) }
                            </p>
                        }
                    } else {
                        html! {
                            <>
                                {
                                    for visible.into_iter().map(|item| {
                                        let selected = item.id == form.id;
                                        let id = item.id.clone();
                                        let open = link.callback(move |_: MouseEvent| Msg::OpenForm(id.clone()));
                                        html! {
                                            <button
                                                type="button"
                                                disabled={model.busy}
                                                onclick={open}
                                                class={format!(
                                                    "flex w-full items-center gap-2 border-b border-[var(--portal-panel-border)] px-2.5 py-2 text-left transition {}",
                                                    if selected {
                                                        "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                                                    } else {
                                                        "border-l-2 border-l-transparent hover:bg-white/25"
                                                    }
                                                )}
                                            >
                                                <span
                                                    class={format!(
                                                        "h-1.5 w-1.5 shrink-0 rounded-full {}",
                                                        if selected && model.dirty {
                                                            "bg-[var(--portal-gold)]"
                                                        } else {
                                                            status_dot_class(&item.status)
                                                        }
                                                    )}
                                                />
                                                <div class="min-w-0 flex-1">
                                                    <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">
                                                        { party_name(item) }
                                                    </div>
                                                    <div class="truncate text-[11px] font-light text-black/45">
                                                        { session_secondary(item, template) }
                                                    </div>
                                                </div>
                                            </button>
                                        }
                                    })
                                }
                            </>
                        }
                    }
                }
            </div>
        </aside>
    }
}

fn empty_forms_view(page: &FormsPage, model: &Model, _link: &Link<Msg>) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
            { error_band(model.error.as_deref()) }
            <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Forms"}</h1>
            {
                if page.items.is_empty() {
                    html! {
                        <p class="mt-3 text-sm font-light text-black/45">{"No saved forms yet."}</p>
                    }
                } else {
                    html! {
                        <p class="mt-3 text-sm font-light text-black/45">
                            {"Select a saved form to open the working surface."}
                        </p>
                    }
                }
            }
        </section>
    }
}

fn field_control(
    field: &FormTemplateField,
    values: &BTreeMap<String, String>,
    link: &Link<Msg>,
) -> Html {
    let value = values.get(&field.name).cloned().unwrap_or_default();
    let label = if field.required {
        format!("{} *", field.label)
    } else {
        field.label.clone()
    };
    let span = field_span_class(field);

    let control = match field.field_type.as_str() {
        "textarea" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                    .value(),
            });
            html! {
                <textarea
                    rows={2}
                    value={value}
                    oninput={changed}
                    class={format!("{INPUT_CLASS} h-auto min-h-12 resize-y py-1.5 leading-6")}
                />
            }
        }
        "select" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value(),
            });
            html! {
                <select value={value} oninput={changed} class={INPUT_CLASS}>
                    <option value="">{"—"}</option>
                    {
                        for field.options.iter().map(|option| html! {
                            <option value={option.clone()}>{ option }</option>
                        })
                    }
                </select>
            }
        }
        "money" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value()
                    .replace('$', "").replace(',', ""),
            });
            html! {
                <input
                    inputmode="decimal"
                    value={format_money(&value)}
                    oninput={changed}
                    class={INPUT_CLASS}
                />
            }
        }
        "date" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            });
            html! {
                <input
                    type="date"
                    value={value}
                    oninput={changed}
                    class={format!("{INPUT_CLASS} appearance-auto [color-scheme:light]")}
                />
            }
        }
        _ => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| Msg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            });
            html! {
                <input type="text" value={value} oninput={changed} class={INPUT_CLASS} />
            }
        }
    };

    html! {
        <label class={format!("{span} min-w-0")}>
            <span class={format!("{LABEL_CLASS} block min-h-[1rem]")}>{ label }</span>
            { control }
        </label>
    }
}

fn field_span_class(field: &FormTemplateField) -> &'static str {
    if field.field_type == "textarea" {
        return "col-span-6";
    }
    if matches!(field.field_type.as_str(), "date" | "money" | "select") {
        return "col-span-2";
    }
    let haystack = format!("{} {}", field.name, field.label).to_lowercase();
    if ["name", "property", "location", "address"]
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        "col-span-3"
    } else {
        "col-span-2"
    }
}

fn preferred_form_id(page: &FormsPage) -> Option<String> {
    page.items
        .iter()
        .find(|item| {
            item.template_id == "LISTING-01"
                && item.template_version == item.active_version
                && item.status != "issued"
        })
        .or_else(|| {
            page.items.iter().find(|item| {
                item.template_id == "LISTING-01" && item.template_version == item.active_version
            })
        })
        .or_else(|| page.items.iter().find(|item| item.template_id == "LISTING-01"))
        .or_else(|| page.items.first())
        .map(|item| item.id.clone())
}

fn current_form_id(model: &Model) -> Option<String> {
    model
        .page
        .as_ref()
        .and_then(|page| page.selected.as_ref())
        .map(|form| form.id.clone())
}

fn current_template_id(model: &Model) -> String {
    model
        .page
        .as_ref()
        .and_then(|page| page.selected.as_ref())
        .map(|form| form.template_id.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            (!model.selected_template.is_empty()).then(|| model.selected_template.clone())
        })
        .unwrap_or_else(|| "LISTING-01".into())
}

fn composed_sections(model: &Model) -> BTreeMap<String, String> {
    let mut sections = model.sections.clone();
    sections.insert("body".into(), model.details_text.clone());
    sections.insert(
        "bodyEdited".into(),
        if model.body_edited { "true" } else { "false" }.into(),
    );
    sections
}

fn resolve_document_body(
    template: &FormTemplate,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
) -> String {
    let edited = sections
        .get("bodyEdited")
        .is_some_and(|value| value == "true");
    let body = sections
        .get("body")
        .map(String::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if edited && !body.is_empty() {
        body.to_owned()
    } else {
        document_body_text(template, values, sections)
    }
}

fn document_body_text(
    template: &FormTemplate,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
) -> String {
    template
        .sections
        .iter()
        .filter(|section| when_visible(section.when.as_ref(), values))
        .map(|section| {
            let generated = section
                .segments
                .iter()
                .map(|segment| match segment.kind.as_str() {
                    "value" => segment
                        .field
                        .as_deref()
                        .and_then(|name| {
                            let value = values.get(name).cloned().unwrap_or_default();
                            template
                                .fields
                                .iter()
                                .find(|field| field.name == name)
                                .map(|field| format_field_value(field, &value))
                                .or(Some(value))
                        })
                        .unwrap_or_default(),
                    _ => segment.text.clone().unwrap_or_default(),
                })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let edited = if section.editable {
                sections
                    .get(&section.name)
                    .map(String::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
            } else {
                ""
            };
            let text = if edited.is_empty() { generated } else { edited.into() };
            if text.trim().is_empty() {
                section.label.clone()
            } else {
                format!("{}\n{}", section.label, text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_field_value(field: &FormTemplateField, value: &str) -> String {
    if field.field_type == "money" {
        format_money(value)
    } else {
        value.to_owned()
    }
}

fn format_money(value: &str) -> String {
    let raw = value
        .trim()
        .trim_start_matches('$')
        .replace(',', "");
    if raw.is_empty() {
        return String::new();
    }
    let (whole, fraction) = raw.split_once('.').unwrap_or((&raw, ""));
    let negative = whole.starts_with('-');
    let digits = whole.trim_start_matches('-');
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let grouped = out.chars().rev().collect::<String>();
    let mut result = if negative {
        format!("-{grouped}")
    } else {
        grouped
    };
    if !fraction.is_empty() {
        result.push('.');
        result.push_str(fraction);
    }
    result
}

fn when_visible(when: Option<&FormWhen>, values: &BTreeMap<String, String>) -> bool {
    let Some(when) = when else {
        return true;
    };
    let actual = values
        .get(&when.field)
        .map(String::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if actual.is_empty() {
        return false;
    }
    actual.eq_ignore_ascii_case("Show All")
        || when
            .values
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(actual))
}

fn party_name(item: &FormItem) -> String {
    let buyer = item
        .field_values
        .get("buyerName")
        .or_else(|| item.field_values.get("visitorName"))
        .or_else(|| item.client_name.as_ref())
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty());
    let seller = item
        .field_values
        .get("sellerName")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty());
    match (buyer, seller) {
        (Some(left), Some(right)) if left != right => format!("{left} / {right}"),
        (Some(left), _) => left.to_owned(),
        (_, Some(right)) => right.to_owned(),
        _ => "Untitled".into(),
    }
}

fn session_label(item: &FormItem) -> String {
    [
        Some(party_name(item)),
        item.property_label.clone(),
        Some(item.updated_at.get(0..10).unwrap_or(&item.updated_at).to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn session_secondary(item: &FormItem, template: &FormTemplate) -> String {
    let updated = item.updated_at.get(0..10).unwrap_or(&item.updated_at);
    [item.property_label.as_deref(), Some(updated)]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
        .trim()
        .to_owned()
        .pipe(|value| {
            if value.is_empty() {
                template.display_name.clone()
            } else {
                value
            }
        })
}

fn status_dot_class(status: &str) -> &'static str {
    match status {
        "issued" => "bg-[var(--portal-success)]",
        "ready" => "bg-[var(--portal-navy-soft)]",
        _ => "bg-black/25",
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

fn error_band(error: Option<&str>) -> Html {
    error
        .map(|message| {
            html! {
                <div class="mb-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-archive)]/25 bg-white/80 px-3 py-2 text-sm text-[var(--portal-archive)]">
                    { message }
                </div>
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_landing_asks_only_for_the_typed_forms_read() {
        let (_, cmd) = Forms::init(&ScreenCtx::default());
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0]
            .path
            .starts_with("/api/portal/rust-ui/forms?screen=forms"));
    }

    #[test]
    fn preferred_form_matches_the_legacy_listing_rule() {
        let mut page = FormsPage::default();
        page.items = vec![
            FormItem {
                id: "issued".into(),
                template_id: "LISTING-01".into(),
                template_version: 4,
                active_version: 4,
                status: "issued".into(),
                ..FormItem::default()
            },
            FormItem {
                id: "draft".into(),
                template_id: "LISTING-01".into(),
                template_version: 4,
                active_version: 4,
                status: "draft".into(),
                ..FormItem::default()
            },
        ];
        assert_eq!(preferred_form_id(&page).as_deref(), Some("draft"));
    }

    #[test]
    fn editing_a_field_is_model_only_and_schedules_preview_and_autosave() {
        let mut model = Model::default();
        let cmd = update(
            &mut model,
            Msg::FieldChanged {
                name: "sellerCivilStatus".into(),
                value: "Married".into(),
            },
            &ScreenCtx::default(),
        );
        assert_eq!(
            model.values.get("sellerCivilStatus").map(String::as_str),
            Some("Married")
        );
        assert!(model.dirty);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn switching_saved_forms_reuses_the_mounted_screen_instead_of_navigating() {
        let mut model = Model::default();
        let cmd = update(
            &mut model,
            Msg::OpenForm("form-next".into()),
            &ScreenCtx::default(),
        );
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0]
            .path
            .contains("screen=form-record&scope=form-next"));
    }

    #[test]
    fn share_is_an_executor_effect_not_a_pdf_navigation() {
        let mut model = Model {
            preview_uri: Some("data:application/pdf;base64,JVBERi0=".into()),
            preview_filename: "Agreement.pdf".into(),
            ..Model::default()
        };
        let cmd = update(&mut model, Msg::Share, &ScreenCtx::default());
        assert!(matches!(cmd, Cmd::SharePdf { .. }));
    }

    #[test]
    fn money_is_shown_with_grouping_without_changing_the_saved_value() {
        assert_eq!(format_money("2100000"), "2,100,000");
        assert_eq!(format_money("425000.50"), "425,000.50");
    }
}
