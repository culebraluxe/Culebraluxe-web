//! CORE — Forms list and generic XML-driven form editor.
//!
//! Pure Yew/MVI: this screen knows no URL literals beyond navigation destinations,
//! performs no fetches and has no database access. Every read/write is a typed
//! Endpoint -> Cmd::request; the Axum bridge calls the canonical Rust services.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::app::api::{
    FormItem, FormTemplate, FormTemplateField, FormTemplateSection, FormWhen, FormsAction,
    FormsBridgeResponse, FormsPage, FormsRead, FormsWrite, FormsWriteResponse,
};
use crate::app::cmd::{ApiError, Cmd};
use crate::app::screen::{Link, Screen, ScreenCtx};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormsListModel {
    page: Option<FormsPage>,
    selected_template: String,
    loading: bool,
    busy: bool,
    error: Option<String>,
}

#[derive(Debug)]
pub enum FormsListMsg {
    Loaded(Result<FormsBridgeResponse, ApiError>),
    TemplateSelected(String),
    Create,
    Created(Result<FormsWriteResponse, ApiError>),
    Open(String),
}

pub struct Forms;

impl Screen for Forms {
    type Model = FormsListModel;
    type Msg = FormsListMsg;

    fn init(ctx: &ScreenCtx) -> (Self::Model, Cmd<Self::Msg>) {
        (
            FormsListModel {
                loading: true,
                ..FormsListModel::default()
            },
            Cmd::request(
                FormsRead::list(
                    ctx.query("dealId").map(str::to_owned),
                    ctx.query("personId").map(str::to_owned),
                    ctx.query("propertyId").map(str::to_owned),
                ),
                FormsListMsg::Loaded,
            ),
        )
    }

    fn update(
        model: &mut Self::Model,
        msg: Self::Msg,
        ctx: &ScreenCtx,
    ) -> Cmd<Self::Msg> {
        match msg {
            FormsListMsg::Loaded(result) => {
                model.loading = false;
                match result {
                    Ok(answer) => {
                        if model.selected_template.is_empty() {
                            model.selected_template = answer
                                .forms
                                .template_choices
                                .first()
                                .map(|item| item.id.clone())
                                .unwrap_or_default();
                        }
                        model.page = Some(answer.forms);
                        model.error = None;
                    }
                    Err(error) => model.error = Some(error.message),
                }
                Cmd::none()
            }
            FormsListMsg::TemplateSelected(value) => {
                model.selected_template = value;
                Cmd::none()
            }
            FormsListMsg::Create => {
                if model.busy {
                    return Cmd::none();
                }
                let has_context = ctx.query("dealId").is_some()
                    || ctx.query("personId").is_some()
                    || ctx.query("propertyId").is_some();
                if !has_context {
                    model.error =
                        Some("Open Forms from a deal, client, or property before creating a form.".into());
                    return Cmd::none();
                }
                if model.selected_template.trim().is_empty() {
                    model.error = Some("Choose a form type first.".into());
                    return Cmd::none();
                }
                model.busy = true;
                model.error = None;
                Cmd::request(
                    FormsWrite {
                        action: FormsAction::Create {
                            template_id: model.selected_template.clone(),
                            deal_id: ctx.query("dealId").map(str::to_owned),
                            person_id: ctx.query("personId").map(str::to_owned),
                            property_id: ctx.query("propertyId").map(str::to_owned),
                        },
                    },
                    FormsListMsg::Created,
                )
            }
            FormsListMsg::Created(result) => {
                model.busy = false;
                match result {
                    Ok(answer) => Cmd::navigate(format!("/portal/forms/{}", answer.form_id)),
                    Err(error) => {
                        model.error = Some(error.message);
                        Cmd::none()
                    }
                }
            }
            FormsListMsg::Open(id) => Cmd::navigate(format!("/portal/forms/{id}")),
        }
    }

    fn view(model: &Self::Model, ctx: &ScreenCtx, link: &Link<Self::Msg>) -> Html {
        let page = model.page.as_ref();
        let context_label = [
            ctx.query("dealId").map(|_| "Deal"),
            ctx.query("personId").map(|_| "Client"),
            ctx.query("propertyId").map(|_| "Property"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" + ");
        let can_create = ctx.can("form.write")
            && (ctx.query("dealId").is_some()
                || ctx.query("personId").is_some()
                || ctx.query("propertyId").is_some());

        let choose_template = link.callback(|event: Event| {
            FormsListMsg::TemplateSelected(
                event
                    .target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value(),
            )
        });
        let create = link.callback(|_: MouseEvent| FormsListMsg::Create);

        html! {
            <section class="flex min-h-0 flex-col gap-4">
                <header class="flex flex-wrap items-end justify-between gap-3">
                    <div>
                        <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-muted)]">
                            {"CORE / Forms"}
                        </p>
                        <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Forms"}</h1>
                        <p class="mt-1 text-sm text-[var(--portal-muted)]">
                            {
                                if context_label.is_empty() {
                                    "Saved transaction forms".to_owned()
                                } else {
                                    format!("{context_label} forms")
                                }
                            }
                        </p>
                    </div>
                    {
                        page.map(|page| html! {
                            <div class="flex flex-wrap items-end gap-2">
                                <label class="min-w-56 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-muted)]">
                                    {"New from template"}
                                    <select
                                        class="mt-1 block h-10 w-full rounded-md border border-[var(--portal-border)] bg-white px-3 text-sm text-[var(--portal-navy)]"
                                        onchange={choose_template}
                                        disabled={model.busy}
                                    >
                                        { for page.template_choices.iter().map(|choice| html! {
                                            <option
                                                value={choice.id.clone()}
                                                selected={choice.id == model.selected_template}
                                            >
                                                { format!("{} · v{}", choice.display_name, choice.active_version) }
                                            </option>
                                        }) }
                                    </select>
                                </label>
                                <button
                                    class="h-10 rounded-md bg-[var(--portal-navy)] px-4 text-xs font-semibold uppercase tracking-[0.12em] text-white disabled:cursor-not-allowed disabled:opacity-40"
                                    disabled={!can_create || model.busy || model.selected_template.is_empty()}
                                    onclick={create}
                                >
                                    { if model.busy { "Creating…" } else { "Create Form" } }
                                </button>
                            </div>
                        }).unwrap_or_default()
                    }
                </header>

                { error_band(model.error.as_deref()) }

                {
                    if model.loading {
                        html! { <p class="py-10 text-sm text-[var(--portal-muted)]">{"Loading forms…"}</p> }
                    } else if let Some(page) = page {
                        if page.items.is_empty() {
                            html! {
                                <div class="rounded-xl border border-[var(--portal-border)] bg-white/60 p-8">
                                    <h2 class="font-serif text-xl text-[var(--portal-navy)]">{"No saved forms yet"}</h2>
                                    <p class="mt-2 text-sm text-[var(--portal-muted)]">
                                        {
                                            if can_create {
                                                "Choose a template above to start this deal or client form."
                                            } else {
                                                "Open Forms from a deal, client, or property to create the first form."
                                            }
                                        }
                                    </p>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="overflow-hidden rounded-xl border border-[var(--portal-border)] bg-white/70">
                                    <div class="grid grid-cols-[minmax(14rem,2fr)_minmax(10rem,1.3fr)_8rem_8rem_2rem] gap-3 border-b border-[var(--portal-border)] px-4 py-2 text-[10px] font-medium uppercase tracking-[0.13em] text-[var(--portal-muted)]">
                                        <span>{"Form"}</span>
                                        <span>{"Context"}</span>
                                        <span>{"Updated"}</span>
                                        <span>{"Status"}</span>
                                        <span></span>
                                    </div>
                                    { for page.items.iter().map(|item| form_row(item, link)) }
                                </div>
                            }
                        }
                    } else {
                        Html::default()
                    }
                }
            </section>
        }
    }
}

fn form_row(item: &FormItem, link: &Link<FormsListMsg>) -> Html {
    let id = item.id.clone();
    let open = link.callback(move |_: MouseEvent| FormsListMsg::Open(id.clone()));
    let context = item
        .client_name
        .as_deref()
        .or(item.deal_label.as_deref())
        .or(item.property_label.as_deref())
        .unwrap_or("—");
    let updated = item.updated_at.get(0..10).unwrap_or(&item.updated_at);
    html! {
        <button
            class="grid w-full grid-cols-[minmax(14rem,2fr)_minmax(10rem,1.3fr)_8rem_8rem_2rem] items-center gap-3 border-b border-[var(--portal-border)] px-4 py-3 text-left last:border-b-0 hover:bg-white"
            onclick={open}
        >
            <span>
                <span class="block font-serif text-base text-[var(--portal-navy)]">{ &item.template_name }</span>
                <span class="text-[11px] text-[var(--portal-muted)]">
                    { format!("{} · v{}", item.template_id, item.template_version) }
                </span>
            </span>
            <span class="truncate text-sm text-[var(--portal-text)]">{ context }</span>
            <span class="text-xs text-[var(--portal-muted)]">{ updated }</span>
            <span class="text-xs font-semibold uppercase tracking-[0.1em] text-[var(--portal-navy)]">
                { &item.status }
            </span>
            <span class="text-lg text-[var(--portal-gold)]">{"›"}</span>
        </button>
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormRecordModel {
    page: Option<FormsPage>,
    values: BTreeMap<String, String>,
    sections: BTreeMap<String, String>,
    loading: bool,
    busy: bool,
    dirty: bool,
    error: Option<String>,
    message: Option<String>,
}

#[derive(Debug)]
pub enum FormRecordMsg {
    Loaded(Result<FormsBridgeResponse, ApiError>),
    FieldChanged { name: String, value: String },
    SectionChanged { name: String, value: String },
    Save,
    Saved(Result<FormsWriteResponse, ApiError>),
    Issue,
    Issued(Result<FormsWriteResponse, ApiError>),
    Back,
}

pub struct FormRecord;

impl Screen for FormRecord {
    type Model = FormRecordModel;
    type Msg = FormRecordMsg;

    fn init(ctx: &ScreenCtx) -> (Self::Model, Cmd<Self::Msg>) {
        let Some(id) = ctx.id.clone() else {
            return (
                FormRecordModel {
                    error: Some("Form id is missing.".into()),
                    ..FormRecordModel::default()
                },
                Cmd::none(),
            );
        };
        (
            FormRecordModel {
                loading: true,
                ..FormRecordModel::default()
            },
            Cmd::request(FormsRead::record(id), FormRecordMsg::Loaded),
        )
    }

    fn update(
        model: &mut Self::Model,
        msg: Self::Msg,
        ctx: &ScreenCtx,
    ) -> Cmd<Self::Msg> {
        match msg {
            FormRecordMsg::Loaded(result) => {
                model.loading = false;
                match result {
                    Ok(answer) => install_record(model, answer.forms, None),
                    Err(error) => model.error = Some(error.message),
                }
                Cmd::none()
            }
            FormRecordMsg::FieldChanged { name, value } => {
                model.values.insert(name, value);
                model.dirty = true;
                model.message = None;
                Cmd::none()
            }
            FormRecordMsg::SectionChanged { name, value } => {
                model.sections.insert(name, value);
                model.dirty = true;
                model.message = None;
                Cmd::none()
            }
            FormRecordMsg::Save => {
                if model.busy {
                    return Cmd::none();
                }
                let Some(id) = ctx.id.clone() else {
                    model.error = Some("Form id is missing.".into());
                    return Cmd::none();
                };
                model.busy = true;
                model.error = None;
                model.message = None;
                Cmd::request(
                    FormsWrite {
                        action: FormsAction::Save {
                            form_id: id,
                            field_values: model.values.clone(),
                            sections: model.sections.clone(),
                        },
                    },
                    FormRecordMsg::Saved,
                )
            }
            FormRecordMsg::Saved(result) => {
                model.busy = false;
                match result {
                    Ok(answer) => install_record(model, answer.forms, Some("Saved".into())),
                    Err(error) => model.error = Some(error.message),
                }
                Cmd::none()
            }
            FormRecordMsg::Issue => {
                if model.busy {
                    return Cmd::none();
                }
                let Some(id) = ctx.id.clone() else {
                    model.error = Some("Form id is missing.".into());
                    return Cmd::none();
                };
                model.busy = true;
                model.error = None;
                model.message = None;
                Cmd::request(
                    FormsWrite {
                        action: FormsAction::Issue {
                            form_id: id,
                            field_values: model.values.clone(),
                            sections: model.sections.clone(),
                        },
                    },
                    FormRecordMsg::Issued,
                )
            }
            FormRecordMsg::Issued(result) => {
                model.busy = false;
                match result {
                    Ok(answer) => {
                        let version = answer
                            .forms
                            .issued
                            .as_ref()
                            .map(|issued| issued.issued_version);
                        install_record(
                            model,
                            answer.forms,
                            Some(
                                version
                                    .map(|value| format!("Issued to Cabinet · v{value}"))
                                    .unwrap_or_else(|| "Issued to Cabinet".into()),
                            ),
                        );
                    }
                    Err(error) => model.error = Some(error.message),
                }
                Cmd::none()
            }
            FormRecordMsg::Back => Cmd::navigate("/portal/forms"),
        }
    }

    fn view(model: &Self::Model, ctx: &ScreenCtx, link: &Link<Self::Msg>) -> Html {
        if model.loading {
            return html! { <p class="py-10 text-sm text-[var(--portal-muted)]">{"Loading form…"}</p> };
        }
        let Some(page) = model.page.as_ref() else {
            return html! {
                <section>
                    { error_band(model.error.as_deref()) }
                </section>
            };
        };
        let (Some(form), Some(template)) = (page.selected.as_ref(), page.template.as_ref()) else {
            return html! {
                <section>
                    { error_band(Some("The form record did not include its saved template.")) }
                </section>
            };
        };

        let back = link.callback(|_: MouseEvent| FormRecordMsg::Back);
        let save = link.callback(|_: MouseEvent| FormRecordMsg::Save);
        let issue = link.callback(|_: MouseEvent| FormRecordMsg::Issue);
        let can_save = ctx.can("form.write");
        let can_issue = ctx.can("vault.issue");
        let status = if model.busy {
            "Working…".to_owned()
        } else if model.dirty {
            "Unsaved".to_owned()
        } else if let Some(issued) = &page.issued {
            format!("Cabinet v{}", issued.issued_version)
        } else {
            form.status.clone()
        };

        html! {
            <section class="flex min-h-0 flex-col gap-4">
                <header class="flex flex-wrap items-end justify-between gap-3">
                    <div class="flex items-start gap-3">
                        <button
                            class="mt-1 rounded-md border border-[var(--portal-border)] px-3 py-2 text-xs text-[var(--portal-navy)] hover:bg-white"
                            onclick={back}
                        >
                            {"← Forms"}
                        </button>
                        <div>
                            <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-muted)]">
                                { format!("{} · v{}", template.id, template.version) }
                            </p>
                            <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">
                                { &template.display_name }
                            </h1>
                            <p class="mt-1 text-xs text-[var(--portal-muted)]">
                                { format!("{} · {}", template.document_type_label, status) }
                            </p>
                        </div>
                    </div>
                    <div class="flex gap-2">
                        <button
                            class="h-10 rounded-md border border-[var(--portal-navy)] px-4 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-40"
                            disabled={!can_save || model.busy || !model.dirty}
                            onclick={save}
                        >
                            {"Save Draft"}
                        </button>
                        <button
                            class="h-10 rounded-md bg-[var(--portal-navy)] px-4 text-xs font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
                            disabled={!can_issue || model.busy}
                            onclick={issue}
                        >
                            {"Issue PDF"}
                        </button>
                    </div>
                </header>

                { error_band(model.error.as_deref()) }
                {
                    model.message.as_ref().map(|message| html! {
                        <div class="rounded-md border border-[var(--portal-border)] bg-white/80 px-3 py-2 text-sm text-[var(--portal-navy)]">
                            { message }
                        </div>
                    }).unwrap_or_default()
                }

                <div class="grid min-h-0 gap-4 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
                    <div class="space-y-4">
                        <section class="rounded-xl border border-[var(--portal-border)] bg-white/70 p-4">
                            <div class="mb-3 flex items-center justify-between">
                                <h2 class="font-serif text-xl text-[var(--portal-navy)]">{"Fields"}</h2>
                                <span class="text-[10px] uppercase tracking-[0.12em] text-[var(--portal-muted)]">
                                    {"Required fields are guidance; drafts remain savable"}
                                </span>
                            </div>
                            <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                                {
                                    for template.fields.iter()
                                        .filter(|field| when_visible(field.when.as_ref(), &model.values))
                                        .map(|field| field_control(field, &model.values, link))
                                }
                            </div>
                        </section>

                        { signature_slots(template, page, &model.values) }
                    </div>

                    <section class="space-y-3 rounded-xl border border-[var(--portal-border)] bg-white/70 p-4">
                        <div>
                            <p class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-muted)]">
                                { &template.presentation }
                            </p>
                            <h2 class="font-serif text-xl text-[var(--portal-navy)]">
                                { &template.rendering_title }
                            </h2>
                        </div>
                        {
                            for template.sections.iter()
                                .filter(|section| when_visible(section.when.as_ref(), &model.values))
                                .map(|section| section_control(section, &model.values, &model.sections, link))
                        }
                    </section>
                </div>
            </section>
        }
    }
}

fn install_record(model: &mut FormRecordModel, page: FormsPage, message: Option<String>) {
    if let Some(selected) = &page.selected {
        model.values = selected.field_values.clone();
        model.sections = selected.sections.clone();
    }
    model.page = Some(page);
    model.dirty = false;
    model.error = None;
    model.message = message;
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

fn field_control(
    field: &FormTemplateField,
    values: &BTreeMap<String, String>,
    link: &Link<FormRecordMsg>,
) -> Html {
    let value = values.get(&field.name).cloned().unwrap_or_default();
    let label = if field.required {
        format!("{} *", field.label)
    } else {
        field.label.clone()
    };
    let common =
        "mt-1 block min-h-10 w-full rounded-md border border-[var(--portal-border)] bg-white px-3 py-2 text-sm text-[var(--portal-text)] outline-none focus:border-[var(--portal-gold)]";

    let control = match field.field_type.as_str() {
        "select" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: Event| FormRecordMsg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value(),
            });
            html! {
                <select class={common} onchange={changed}>
                    <option value="" selected={value.is_empty()}>{"—"}</option>
                    { for field.options.iter().map(|option| html! {
                        <option value={option.clone()} selected={option == &value}>{ option }</option>
                    }) }
                </select>
            }
        }
        "textarea" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| FormRecordMsg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                    .value(),
            });
            html! {
                <textarea class={format!("{common} min-h-24")} value={value} oninput={changed} />
            }
        }
        "date" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| FormRecordMsg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            });
            html! { <input class={common} type="date" value={value} oninput={changed} /> }
        }
        "money" => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| FormRecordMsg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            });
            html! { <input class={common} type="text" inputmode="decimal" value={value} oninput={changed} /> }
        }
        _ => {
            let name = field.name.clone();
            let changed = link.callback(move |event: InputEvent| FormRecordMsg::FieldChanged {
                name: name.clone(),
                value: event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            });
            html! { <input class={common} type="text" value={value} oninput={changed} /> }
        }
    };

    html! {
        <label class={if field.field_type == "textarea" { "md:col-span-2" } else { "" }}>
            <span class="text-[10px] font-medium uppercase tracking-[0.13em] text-[var(--portal-muted)]">
                { label }
            </span>
            { control }
        </label>
    }
}

fn section_control(
    section: &FormTemplateSection,
    values: &BTreeMap<String, String>,
    sections: &BTreeMap<String, String>,
    link: &Link<FormRecordMsg>,
) -> Html {
    if section.editable {
        let value = sections.get(&section.name).cloned().unwrap_or_default();
        let name = section.name.clone();
        let changed = link.callback(move |event: InputEvent| FormRecordMsg::SectionChanged {
            name: name.clone(),
            value: event
                .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                .value(),
        });
        return html! {
            <div class="rounded-lg border border-[var(--portal-border)] bg-white/80 p-3">
                <h3 class="font-serif text-base font-semibold text-[var(--portal-navy)]">{ &section.label }</h3>
                <textarea
                    class="mt-2 min-h-28 w-full rounded-md border border-[var(--portal-border)] bg-white px-3 py-2 text-sm leading-6 text-[var(--portal-text)] outline-none focus:border-[var(--portal-gold)]"
                    value={value}
                    oninput={changed}
                />
            </div>
        };
    }

    let text = section
        .segments
        .iter()
        .map(|segment| match segment.kind.as_str() {
            "value" => segment
                .field
                .as_deref()
                .and_then(|field| values.get(field))
                .cloned()
                .unwrap_or_default(),
            _ => segment.text.clone().unwrap_or_default(),
        })
        .collect::<String>();

    html! {
        <article class="rounded-lg border border-[var(--portal-border)] bg-white/50 p-3">
            <h3 class="font-serif text-base font-semibold text-[var(--portal-navy)]">{ &section.label }</h3>
            <p class="mt-2 whitespace-pre-wrap text-sm leading-6 text-[var(--portal-text)]">{ text }</p>
        </article>
    }
}

fn signature_slots(
    template: &FormTemplate,
    page: &FormsPage,
    values: &BTreeMap<String, String>,
) -> Html {
    if template.signature_groups.is_empty() {
        return Html::default();
    }
    html! {
        <section class="rounded-xl border border-[var(--portal-border)] bg-white/70 p-4">
            <h2 class="font-serif text-xl text-[var(--portal-navy)]">{"Signature slots"}</h2>
            <div class="mt-3 grid gap-2 md:grid-cols-2">
                {
                    for template.signature_groups.iter().map(|group| {
                        let field_name = group.field.as_deref().unwrap_or_default();
                        let value = values.get(field_name).cloned().unwrap_or_default();
                        let signers = page
                            .signers
                            .iter()
                            .filter(|signer| signer.role == group.role)
                            .map(|signer| signer.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ");
                        html! {
                            <div class="rounded-lg border border-[var(--portal-border)] bg-white/80 p-3">
                                <div class="flex items-center justify-between gap-2">
                                    <span class="text-sm font-semibold text-[var(--portal-navy)]">{ &group.label }</span>
                                    <span class="text-[10px] uppercase tracking-[0.12em] text-[var(--portal-muted)]">
                                        { &group.role }
                                    </span>
                                </div>
                                <p class="mt-1 text-sm text-[var(--portal-text)]">
                                    {
                                        if !value.trim().is_empty() {
                                            value
                                        } else if !signers.is_empty() {
                                            signers
                                        } else {
                                            "Unassigned".into()
                                        }
                                    }
                                </p>
                                {
                                    if group.initials {
                                        html! { <p class="mt-1 text-[10px] uppercase tracking-[0.12em] text-[var(--portal-muted)]">{"Initials required"}</p> }
                                    } else {
                                        Html::default()
                                    }
                                }
                            </div>
                        }
                    })
                }
            </div>
        </section>
    }
}

fn error_band(error: Option<&str>) -> Html {
    error
        .map(|message| {
            html! {
                <div class="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-800">
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
    fn forms_list_initializes_through_the_typed_endpoint() {
        let ctx = ScreenCtx {
            query: BTreeMap::from([("personId".into(), "person-1".into())]),
            ..ScreenCtx::default()
        };
        let (_, cmd) = Forms::init(&ctx);
        let requests = cmd.into_requests();
        assert_eq!(requests.len(), 1);
        assert!(format!("{:?}", requests[0]).contains("/api/portal/rust-ui/forms?screen=forms"));
        assert!(format!("{:?}", requests[0]).contains("personId=person-1"));
    }

    #[test]
    fn editing_a_field_only_changes_the_screen_model() {
        let mut model = FormRecordModel::default();
        let command = FormRecord::update(
            &mut model,
            FormRecordMsg::FieldChanged {
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
        assert!(matches!(command, Cmd::None));
    }

    #[test]
    fn template_visibility_matches_the_rust_template_rule() {
        let when = FormWhen {
            field: "kind".into(),
            values: vec!["House".into(), "Land".into()],
        };
        let mut values = BTreeMap::new();
        assert!(!when_visible(Some(&when), &values));
        values.insert("kind".into(), "land".into());
        assert!(when_visible(Some(&when), &values));
        values.insert("kind".into(), "Show All".into());
        assert!(when_visible(Some(&when), &values));
    }
}
