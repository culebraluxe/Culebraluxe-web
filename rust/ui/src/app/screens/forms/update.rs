//! The Forms update: every message, local edits, installing a record, the preview and creating a form.

#[allow(unused_imports)]
use super::*;

pub(super) fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
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
        Msg::NewSellerChanged(value) => {
            model.new_seller = value;
            Cmd::none()
        }
        Msg::NewCatastroChanged(value) => {
            model.new_catastro = value;
            Cmd::none()
        }
        Msg::NewFormCancel => {
            model.new_form_template = None;
            Cmd::none()
        }
        Msg::NewFormCreate => {
            let Some(template_id) = model.new_form_template.clone() else {
                return Cmd::none();
            };
            let seller = model.new_seller.trim().to_owned();
            let catastro = model.new_catastro.trim().to_owned();
            if seller.is_empty() && catastro.is_empty() {
                model.error = Some("Who is this form for? Type the seller as on the contract, or the catastro number.".into());
                return Cmd::none();
            }
            model.busy = true;
            model.error = None;
            Cmd::request(
                FormsWrite {
                    action: FormsAction::Create {
                        template_id,
                        deal_id: None,
                        person_id: None,
                        property_id: None,
                        seller_name: Some(seller).filter(|value| !value.is_empty()),
                        catastro: Some(catastro).filter(|value| !value.is_empty()),
                    },
                },
                Msg::Created,
            )
        }
        Msg::NewForm => {
            let template_id = current_template_id(model);
            create_form(model, &template_id)
        }
        Msg::Created(result) => {
            model.busy = false;
            match result {
                Ok(answer) => {
                    let (seller, catastro) = (model.new_seller.trim().to_owned(), model.new_catastro.trim().to_owned());
                    model.new_form_template = None;
                    let installed = install_record(model, answer.forms, Some("New form".into()));
                    // What was typed to find the seller and the property belongs on the contract too.
                    let mut typed = false;
                    for (field, value) in [("sellerName", seller), ("catastroNumber", catastro)] {
                        if !value.is_empty() && model.values.get(field).map_or(true, |current| current.trim().is_empty()) {
                            model.values.insert(field.to_owned(), value);
                            typed = true;
                        }
                    }
                    if typed { Cmd::batch([installed, local_edit(model)]) } else { installed }
                }
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
                Err(error) if error.code == "CANCELLED" => {
                    model.message = Some("Share cancelled".into());
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
                return Cmd::none();
            }
            let (Some(form_id), Some(template)) =
                (current_form_id(model), model.page.as_ref().and_then(|page| page.template.clone()))
            else {
                model.error = Some("Open a form first.".into());
                return Cmd::none();
            };
            model.grok_working = true;
            model.error = None;
            model.message = Some("Asking Grok…".into());
            Cmd::request(
                FormsGrok {
                    form_id,
                    form_name: template.display_name.clone(),
                    prompt: model.grok_prompt.trim().to_owned(),
                    details_text: model.details_text.clone(),
                    field_values: model.values.clone(),
                    fields: template.fields,
                },
                Msg::GrokFilled,
            )
        }
        // Grok suggests; the editor takes it as if typed, so it autosaves like any edit and the agent still Sends.
        Msg::GrokFilled(result) => {
            model.grok_working = false;
            match result {
                Ok(answer) => {
                    for (name, value) in answer.field_values {
                        model.values.insert(name, value);
                    }
                    if let Some(body) = answer.body {
                        model.details_text = body;
                        model.body_edited = true;
                    }
                    model.grok_prompt.clear();
                    let cmd = local_edit(model);
                    model.message = Some(answer.note);
                    cmd
                }
                Err(error) => {
                    model.message = None;
                    model.error = Some(error.message);
                    Cmd::none()
                }
            }
        }
        Msg::MicPressed => {
            if model.listening {
                return Cmd::none();
            }
            model.listening = true;
            model.error = None;
            model.message = Some("Listening…".into());
            Cmd::listen(Msg::Heard)
        }
        // What was heard goes into the prompt, to read over before Go — the mic never sends by itself.
        Msg::Heard(result) => {
            model.listening = false;
            model.message = None;
            match result {
                Ok(words) => {
                    let prompt = model.grok_prompt.trim();
                    model.grok_prompt = if prompt.is_empty() { words } else { format!("{prompt} {words}") };
                }
                Err(error) => model.error = Some(error.message),
            }
            Cmd::none()
        }
    }
}

pub(super) fn local_edit(model: &mut Model) -> Cmd<Msg> {
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

pub(super) fn install_record(model: &mut Model, page: FormsPage, message: Option<String>) -> Cmd<Msg> {
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

pub(super) fn request_preview(model: &mut Model) -> Cmd<Msg> {
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

/// A NEW FORM IS FOR SOMEONE. It used to take the open form's deal, client and property — so every contract
/// inherited the first one's demo deal ("Sunset Point"). Now it asks: the seller as named on the contract and the
/// property's catastro, and the server finds (or makes) that person and that property. No deal is inherited.
pub(super) fn create_form(model: &mut Model, template_id: &str) -> Cmd<Msg> {
    if model.busy {
        return Cmd::none();
    }
    model.new_form_template = Some(template_id.to_owned());
    model.new_seller.clear();
    model.new_catastro.clear();
    model.error = None;
    Cmd::none()
}
