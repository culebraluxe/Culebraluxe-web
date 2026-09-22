//! CORE / Forms — saved sessions and the working editor on Yew.
//!
//! The template metadata is static XML-derived application metadata; every persisted value comes through the Rust API.
//! Yew owns no draft state: inputs emit Msg and the reducer mutates PortalFormsPage.selected.

use yew::prelude::*;

use crate::model::{
    Msg, PortalFormField, PortalFormRecord, PortalFormSection, PortalFormTemplate, PortalFormsPage,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct FormsProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Forms;
pub struct FormRecord;

impl Component for Forms {
    type Message = ();
    type Properties = FormsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("forms").expect("forms screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { forms_index(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

impl Component for FormRecord {
    type Message = ();
    type Properties = FormsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("form-record").expect("form record exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { form_editor(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn forms_payload(model: &crate::model::Model) -> Option<&PortalFormsPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.forms.as_ref())
}

fn forms_index(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let payload = forms_payload(model);
    let query = model.controls.query.trim().to_lowercase();
    let items = payload
        .map(|forms| {
            forms
                .items
                .iter()
                .filter(|item| {
                    query.is_empty()
                        || item.template_name.to_lowercase().contains(&query)
                        || item
                            .client_name
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || item
                            .property_label
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let oninput = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };

    html! {
        <div>
            <header class="mb-6 flex flex-wrap items-end justify-between gap-4">
                <div>
                    <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Forms"}</h1>
                    <p class="mt-1 text-sm font-light text-black/55">
                        {"Working transaction forms, versioned against their immutable template."}
                    </p>
                </div>
                <input type="search" {oninput} value={model.controls.query.clone()} placeholder="Search forms…"
                    class="h-10 w-full max-w-xs rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-3 text-sm font-light outline-none focus:border-[var(--portal-navy)]" />
            </header>

            if model.loading {
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
                    <p class="text-sm font-light text-black/45">{"Loading forms…"}</p>
                </section>
            } else if items.is_empty() {
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-8 py-14 text-center">
                    <h2 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"No matching forms"}</h2>
                    <p class="mt-2 text-sm font-light text-black/50">
                        {"Forms are created from a transaction, client, or property context."}
                    </p>
                </section>
            } else {
                <div class="grid gap-3 lg:grid-cols-2 2xl:grid-cols-3">
                    { for items.into_iter().map(form_card) }
                </div>
            }
        </div>
    }
}

fn form_card(item: &crate::model::PortalFormSummary) -> Html {
    let who = party_label(
        item.field_values.get("buyerName"),
        item.field_values.get("sellerName"),
        item.client_name.as_ref(),
    );
    html! {
        <a href={format!("/portal/forms/{}", item.id)}
            class="portal-glass-panel block rounded-[var(--portal-panel-radius)] p-5 transition hover:border-[var(--portal-blue-gray)]/50">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                        { format!("{} · v{}", item.template_name, item.template_version) }
                    </div>
                    <h2 class="mt-2 truncate font-serif text-xl font-light text-[var(--portal-navy)]">{ who }</h2>
                    <p class="mt-1 truncate text-xs font-light text-black/50">
                        { item.property_label.clone().or_else(|| item.deal_label.clone()).unwrap_or_else(|| "No property context".into()) }
                    </p>
                </div>
                { status_pill(&item.status) }
            </div>
            <div class="mt-4 flex items-center justify-between text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                <span>{ format!("Updated {}", item.updated_at) }</span>
                if item.template_version != item.active_version {
                    <span>{"History"}</span>
                }
            </div>
        </a>
    }
}

fn form_editor(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(payload) = forms_payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
                <p class="text-sm font-light text-black/45">
                    { if model.loading { "Loading form…" } else { "Form not found." } }
                </p>
            </section>
        };
    };
    let (Some(form), Some(template)) = (payload.selected.as_ref(), payload.template.as_ref()) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-8">
                <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">{"Form not found"}</h1>
            </section>
        };
    };

    html! {
        <div class="flex min-h-0 flex-col gap-3">
            { editor_status(model, payload, form, template, on_msg) }
            <div class="grid min-h-0 flex-1 gap-4 lg:h-[calc(100dvh-12.5rem)] lg:grid-cols-[220px_minmax(0,1fr)_minmax(300px,0.8fr)]">
                { sessions(payload, form, model, on_msg) }
                { editor_panel(payload, form, template, on_msg) }
                { document_panel(payload, form, template) }
            </div>
        </div>
    }
}

fn editor_status(
    model: &crate::model::Model,
    payload: &PortalFormsPage,
    form: &PortalFormRecord,
    template: &PortalFormTemplate,
    on_msg: &Callback<Msg>,
) -> Html {
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::FormSaveRequested))
    };
    let state = if payload.saving {
        "Saving…".to_string()
    } else if payload.dirty {
        "Unsaved changes".to_string()
    } else if let Some(issued) = payload.issued.as_ref() {
        format!("Vault v{}", issued.issued_version)
    } else {
        form.status.clone()
    };

    html! {
        <section class="portal-glass-panel portal-glass-panel-lifted flex flex-wrap items-center justify-between gap-3 rounded-[var(--portal-panel-radius)] p-3">
            <div class="min-w-0">
                <div class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Forms"}</div>
                <div class="mt-1 truncate font-serif text-lg font-light text-[var(--portal-navy)]">
                    { format!("{} · v{}", template.display_name, template.version) }
                </div>
                if let Some(error) = model.error.as_ref() {
                    <p class="mt-1 text-xs font-light text-red-700">{ error.clone() }</p>
                } else {
                    <p class="mt-1 text-xs font-light text-black/45">{ state }</p>
                }
            </div>
            <div class="flex items-center gap-2">
                <button type="button" onclick={save} disabled={!payload.dirty || payload.saving}
                    class="inline-flex min-h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-35">
                    { if payload.saving { "Saving…" } else { "Save Draft" } }
                </button>
                <span class="rounded-full bg-[var(--portal-blue-pale)] px-3 py-1 text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]">
                    { form.status.clone() }
                </span>
            </div>
        </section>
    }
}

fn sessions(
    payload: &PortalFormsPage,
    form: &PortalFormRecord,
    model: &crate::model::Model,
    on_msg: &Callback<Msg>,
) -> Html {
    let query = model.controls.query.trim().to_lowercase();
    let visible = payload
        .items
        .iter()
        .filter(|item| {
            query.is_empty()
                || party_label(
                    item.field_values.get("buyerName"),
                    item.field_values.get("sellerName"),
                    item.client_name.as_ref(),
                )
                .to_lowercase()
                .contains(&query)
                || item
                    .property_label
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
        })
        .collect::<Vec<_>>();

    let oninput = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };

    html! {
        <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                <div class="mb-2 flex items-center justify-between gap-2">
                    <span class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Forms · {}", visible.len()) }
                    </span>
                    { new_form_control(payload, form, on_msg) }
                </div>
                <input type="search" {oninput} value={model.controls.query.clone()} placeholder="Search…"
                    class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none" />
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto">
                { for visible.into_iter().map(|item| session_row(item, form, payload.dirty)) }
            </div>
        </aside>
    }
}

fn new_form_control(
    payload: &PortalFormsPage,
    _form: &PortalFormRecord,
    on_msg: &Callback<Msg>,
) -> Html {
    let onchange = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            if !value.is_empty() {
                on_msg.emit(Msg::FormCreateRequested { template_id: value });
            }
        })
    };
    html! {
        <select {onchange} value="" disabled={payload.saving || payload.dirty}
            aria-label="Start a new form in this context"
            class="max-w-[92px] rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-2 py-1 text-[10px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy)] disabled:opacity-35">
            <option value="">{"New"}</option>
            { for payload.template_choices.iter().map(|choice| html! {
                <option value={choice.id.clone()}>{ choice.display_name.clone() }</option>
            }) }
        </select>
    }
}

fn session_row(
    item: &crate::model::PortalFormSummary,
    current: &PortalFormRecord,
    dirty: bool,
) -> Html {
    let selected = item.id == current.id;
    let who = party_label(
        item.field_values.get("buyerName"),
        item.field_values.get("sellerName"),
        item.client_name.as_ref(),
    );
    let body = html! {
        <>
            <span class={format!("h-1.5 w-1.5 shrink-0 rounded-full {}", status_dot(&item.status))}></span>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">{ who }</div>
                <div class="truncate text-[11px] font-light text-black/45">
                    { item.property_label.clone().unwrap_or_else(|| item.template_name.clone()) }
                </div>
            </div>
        </>
    };
    let class = classes!(
        "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left",
        if selected { "border-l-2 border-l-[var(--portal-gold)] bg-white/40" } else { "border-l-2 border-l-transparent hover:bg-white/25" }
    );

    if selected {
        html! { <div {class}>{ body }</div> }
    } else if dirty {
        html! {
            <div class={classes!(class, "opacity-45")} title="Save this draft before switching forms">{ body }</div>
        }
    } else {
        html! { <a href={format!("/portal/forms/{}", item.id)} {class}>{ body }</a> }
    }
}

fn editor_panel(
    payload: &PortalFormsPage,
    form: &PortalFormRecord,
    template: &PortalFormTemplate,
    on_msg: &Callback<Msg>,
) -> Html {
    html! {
        <section class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="shrink-0 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{ template.display_name.clone() }</h2>
                        <p class="text-[10px] font-light uppercase tracking-[0.12em] text-black/35">
                            { format!("{} · template v{}", template.document_type_label, template.version) }
                        </p>
                    </div>
                    if template.version != template.active_version {
                        <span class="rounded-full border border-black/15 px-2 py-1 text-[9px] font-light uppercase tracking-[0.12em] text-black/40">{"History"}</span>
                    }
                </div>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-4">
                <div class="grid grid-cols-6 gap-x-3 gap-y-4">
                    { for template.fields.iter().filter(|field| visible_field(field, form)).map(|field| field_control(field, form, on_msg)) }
                </div>

                if !template.sections.is_empty() {
                    <div class="mt-7 space-y-5 border-t border-[var(--portal-panel-border)] pt-5">
                        { for template.sections.iter().filter(|section| visible_section(section, form)).map(|section| section_control(section, form, on_msg)) }
                    </div>
                }
            </div>

            <div class="shrink-0 border-t border-[var(--portal-panel-border)] px-4 py-2 text-[10px] font-light text-black/40">
                {
                    if payload.dirty {
                        "Working draft differs from Rust persistence.".to_string()
                    } else {
                        format!("Saved {}", form.updated_at)
                    }
                }
            </div>
        </section>
    }
}

fn visible_field(field: &PortalFormField, form: &PortalFormRecord) -> bool {
    field
        .when
        .as_ref()
        .is_none_or(|gate| {
            form.field_values
                .get(&gate.field)
                .is_some_and(|value| gate.values.iter().any(|candidate| candidate == value))
        })
}

fn visible_section(section: &PortalFormSection, form: &PortalFormRecord) -> bool {
    section
        .when
        .as_ref()
        .is_none_or(|gate| {
            form.field_values
                .get(&gate.field)
                .is_some_and(|value| gate.values.iter().any(|candidate| candidate == value))
        })
}

fn field_control(field: &PortalFormField, form: &PortalFormRecord, on_msg: &Callback<Msg>) -> Html {
    let value = form.field_values.get(&field.name).cloned().unwrap_or_default();
    let span = if field.field_type == "textarea" {
        "col-span-6"
    } else if matches!(field.field_type.as_str(), "date" | "money" | "select") {
        "col-span-2"
    } else if field.name.to_lowercase().contains("name")
        || field.name.to_lowercase().contains("property")
        || field.name.to_lowercase().contains("address")
    {
        "col-span-3"
    } else {
        "col-span-2"
    };
    let label = html! {
        <div class="text-[9px] font-light uppercase tracking-[0.14em] text-black/40">
            { field.label.clone() }
            if field.required { <span class="ml-1 text-[var(--portal-gold-muted)]">{"*"}</span> }
        </div>
    };

    let name = field.name.clone();
    let control = match field.field_type.as_str() {
        "textarea" => {
            let on_msg = on_msg.clone();
            let oninput = Callback::from(move |event: InputEvent| {
                let value = event
                    .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
                    .value();
                on_msg.emit(Msg::FormFieldChanged {
                    name: name.clone(),
                    value,
                });
            });
            html! {
                <textarea {oninput} value={value}
                    class="mt-1 min-h-28 w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 py-2 text-[13px] font-light leading-5 text-black/70 outline-none focus:border-[var(--portal-navy-soft)]" />
            }
        }
        "select" => {
            let on_msg = on_msg.clone();
            let onchange = Callback::from(move |event: Event| {
                let value = event
                    .target_unchecked_into::<web_sys::HtmlSelectElement>()
                    .value();
                on_msg.emit(Msg::FormFieldChanged {
                    name: name.clone(),
                    value,
                });
            });
            html! {
                <select {onchange} value={value}
                    class="mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light text-black/70 outline-none">
                    <option value=""></option>
                    { for field.options.iter().map(|option| html! {
                        <option value={option.clone()}>{ option.clone() }</option>
                    }) }
                </select>
            }
        }
        _ => {
            let on_msg = on_msg.clone();
            let input_type = if field.field_type == "date" { "date" } else { "text" };
            let oninput = Callback::from(move |event: InputEvent| {
                let value = event
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value();
                on_msg.emit(Msg::FormFieldChanged {
                    name: name.clone(),
                    value,
                });
            });
            html! {
                <input type={input_type} {oninput} value={value}
                    class="mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]" />
            }
        }
    };

    html! { <label class={span}>{ label }{ control }</label> }
}

fn section_control(
    section: &PortalFormSection,
    form: &PortalFormRecord,
    on_msg: &Callback<Msg>,
) -> Html {
    let value = form.sections.get(&section.name).cloned().unwrap_or_default();
    if !section.editable {
        return html! {
            <section>
                <h3 class="font-serif text-base font-bold text-[var(--portal-navy)]">{ section.label.clone() }</h3>
                <p class="mt-2 whitespace-pre-wrap text-sm font-light leading-6 text-black/60">
                    { if value.is_empty() { "Generated from the template and form fields." } else { value.as_str() } }
                </p>
            </section>
        };
    }

    let name = section.name.clone();
    let on_msg = on_msg.clone();
    let oninput = Callback::from(move |event: InputEvent| {
        let value = event
            .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
            .value();
        on_msg.emit(Msg::FormSectionChanged {
            name: name.clone(),
            value,
        });
    });
    html! {
        <label class="block">
            <span class="font-serif text-base font-bold text-[var(--portal-navy)]">{ section.label.clone() }</span>
            <textarea {oninput} value={value}
                class="mt-2 min-h-32 w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 py-2 font-serif text-[15px] font-light leading-6 text-[var(--portal-navy)] outline-none" />
        </label>
    }
}

fn document_panel(
    payload: &PortalFormsPage,
    form: &PortalFormRecord,
    template: &PortalFormTemplate,
) -> Html {
    html! {
        <aside class="portal-glass-panel min-h-0 overflow-y-auto rounded-[var(--portal-panel-radius)] p-5">
            <div>
                <div class="text-[10px] font-medium uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">{"Document"}</div>
                <h2 class="mt-1 font-serif text-xl font-light text-[var(--portal-navy)]">{ template.rendering_title.clone() }</h2>
                <p class="mt-1 text-xs font-light text-black/45">{ template.presentation.clone() }</p>
            </div>

            <dl class="mt-5 space-y-3 text-sm font-light">
                { fact("Form", &form.id) }
                { fact("Template", &format!("{} v{}", template.id, template.version)) }
                { fact("Status", &form.status) }
                { fact("Deal", form.deal_id.as_deref().unwrap_or("—")) }
                { fact("Client", form.person_id.as_deref().unwrap_or("—")) }
                { fact("Property", form.property_id.as_deref().unwrap_or("—")) }
            </dl>

            <div class="mt-6 border-t border-[var(--portal-panel-border)] pt-5">
                <h3 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Vault"}</h3>
                if let Some(issued) = payload.issued.as_ref() {
                    <div class="mt-3 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 p-3">
                        <div class="text-sm font-medium text-[var(--portal-navy)]">{ format!("Issued v{}", issued.issued_version) }</div>
                        <div class="mt-1 break-all text-[10px] font-light text-black/40">{ issued.checksum.clone() }</div>
                        <a href={format!("/portal/documents/{}", issued.document_id)}
                            class="mt-3 inline-flex text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]">
                            {"Open in Cabinet →"}
                        </a>
                    </div>
                } else {
                    <p class="mt-2 text-sm font-light text-black/45">{"No issued Vault document yet."}</p>
                }

                <button type="button" disabled=true
                    title="The Rust HTTP transport does not have the Vault artifact renderer attached yet."
                    class="mt-4 inline-flex min-h-9 w-full items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white opacity-35">
                    {"Save PDF · renderer pending"}
                </button>
            </div>

            <div class="mt-6 border-t border-[var(--portal-panel-border)] pt-5">
                <h3 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Signers"}</h3>
                if payload.signers.is_empty() {
                    <p class="mt-2 text-sm font-light text-black/45">{"No signer people resolved."}</p>
                } else {
                    <ul class="mt-3 space-y-2">
                        { for payload.signers.iter().map(|signer| html! {
                            <li class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 p-3">
                                <div class="text-sm font-medium text-[var(--portal-navy)]">{ signer.name.clone() }</div>
                                <div class="text-xs font-light text-black/45">
                                    { format!("{} · {}", signer.role, signer.email.clone().unwrap_or_else(|| "no email".into())) }
                                </div>
                            </li>
                        }) }
                    </ul>
                }
                <button type="button" disabled=true
                    title="Signature send requires a Rust-issued document first."
                    class="mt-4 inline-flex min-h-9 w-full items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-4 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] opacity-35">
                    {"Send for signature · issuance pending"}
                </button>
            </div>
        </aside>
    }
}

fn fact(label: &str, value: &str) -> Html {
    html! {
        <div class="flex items-start justify-between gap-4">
            <dt class="text-black/40">{ label }</dt>
            <dd class="max-w-[70%] break-all text-right text-[var(--portal-navy)]">{ value }</dd>
        </div>
    }
}

fn party_label(
    buyer: Option<&String>,
    seller: Option<&String>,
    client: Option<&String>,
) -> String {
    let left = buyer
        .filter(|value| !value.trim().is_empty())
        .or_else(|| client.filter(|value| !value.trim().is_empty()));
    let right = seller.filter(|value| !value.trim().is_empty());
    match (left, right) {
        (Some(left), Some(right)) => format!("{left} / {right}"),
        (Some(value), None) | (None, Some(value)) => value.clone(),
        (None, None) => "Untitled".into(),
    }
}

fn status_pill(status: &str) -> Html {
    html! {
        <span class={classes!(
            "rounded-full","px-3","py-1","text-[10px]","font-light","uppercase","tracking-[0.12em]",
            if status == "issued" { "bg-emerald-50 text-emerald-700" } else { "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]" }
        )}>{ status }</span>
    }
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "issued" => "bg-[var(--portal-success)]",
        "ready" => "bg-[var(--portal-navy-soft)]",
        _ => "bg-black/25",
    }
}
