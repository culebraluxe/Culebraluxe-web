//! The Workbench's update: reads and writes, the draft guard, media numbering and the upload queue, and every message.

#[allow(unused_imports)]
use super::*;

/// Read the page for the current entity, search, page and selection.
pub(super) fn read(model: &mut Model) -> Cmd<Msg> {
    model.seq += 1;
    model.loading = true;
    let seq = model.seq;
    Cmd::request(
        OpsRead {
            entity: model.ops.entity.clone(),
            selected: model.selected.clone(),
            search: model.controls.query.clone(),
            page: model.controls.page,
        },
        move |answer| Msg::Loaded { seq, answer },
    )
}

/// A write; its answer is the refreshed page, so it is a read too and takes the sequence.
pub(super) fn write(model: &mut Model, body: serde_json::Value) -> Cmd<Msg> {
    model.seq += 1;
    model.loading = true;
    let seq = model.seq;
    Cmd::request(OpsCommand { body }, move |answer| Msg::Loaded {
        seq,
        answer,
    })
}

pub(super) fn reset_aux(ops: &mut OpsWorkbenchState) {
    ops.person_query.clear();
    ops.person_people.clear();
    ops.person_searching = false;
    ops.selected_person = None;
    ops.media_index = 0;
    ops.media_alt.clear();
    ops.media_file_name = None;
    ops.media_uploading = false;
    ops.media_uploader_open = false;
}

/// `VillaDelMar_7` — the title a photograph gets when nobody typed one: the property's name and the next photo number.
pub(super) fn media_title(page: Option<&PortalOpsWorkbenchPage>) -> String {
    media_title_at(page, 0)
}

/// The title of the photo `offset` places after the next one (a batch numbers its photos in order).
pub(super) fn media_title_at(page: Option<&PortalOpsWorkbenchPage>, offset: usize) -> String {
    let Some(page) = page else {
        return String::new();
    };
    let Some(property) = page.property.as_ref() else {
        return String::new();
    };
    let stem: String = property
        .name
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if stem.is_empty() {
        return String::new();
    }
    let images = page
        .media
        .iter()
        .filter(|media| media.media_type == "image")
        .count();
    format!("{stem}_{}", images + 1 + offset)
}

/// `VillaDelMar_7` -> 7.
pub(super) fn first_number(title: &str) -> usize {
    title
        .rsplit('_')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

/// `VillaDelMar_7`, 9 -> `VillaDelMar_9`.
pub(super) fn numbered(title: &str, number: usize) -> String {
    let stem = title
        .rsplit_once('_')
        .map(|(stem, _)| stem)
        .unwrap_or(title);
    format!("{stem}_{number}")
}

/// Start the next photo of the batch, if any.
pub(super) fn next_upload(model: &mut Model) -> Cmd<Msg> {
    if model.upload_queue.is_empty() {
        return Cmd::none();
    }
    let file = model.upload_queue.remove(0);
    model.upload_current = Some(file.clone());
    // The property the batch started on, even if another is selected while it runs.
    let Some(property_id) = model.ops.media_batch_property.clone() else {
        model.upload_queue.clear();
        model.error = Some("Select a Property before uploading.".into());
        return Cmd::none();
    };
    // Numbered from the batch's start, so the gallery refreshing mid-batch does not shift the numbers.
    let offset = model.ops.media_batch_done + model.ops.media_batch_failed.len();
    model.ops.media_file_name = Some(file.name());
    model.ops.media_role = "gallery".into();
    model.ops.media_alt = model
        .ops
        .media_batch_first
        .as_ref()
        .map(|first| numbered(first, first_number(first) + offset))
        .unwrap_or_default();
    model.ops.media_uploading = true;
    model.error = None;
    let mut init = vec![("role".to_string(), "gallery".to_string())];
    if !model.ops.media_alt.trim().is_empty() {
        init.push((
            "altText".to_string(),
            model.ops.media_alt.trim().to_string(),
        ));
    }
    Cmd::upload(
        file,
        PropertyMediaChunked,
        vec![("propertyId".to_string(), property_id)],
        init,
        Msg::Uploaded,
    )
}

/// Refuse a list change while the draft is unsaved, and say which action was refused.
pub(super) fn guard_draft(model: &mut Model, action: &str) -> bool {
    if model.ops.dirty {
        model.error = Some(format!("Save or Revert changes before {action}."));
        return true;
    }
    false
}

pub(super) fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Loaded { seq, answer } => {
            if seq != model.seq {
                return Cmd::none();
            }
            model.loading = false;
            match answer.and_then(|page| {
                page.ops
                    .ok_or_else(|| ApiError::decode("The answer had no Workbench in it."))
            }) {
                Ok(page) => {
                    model.ops.entity = page.entity.clone();
                    model.selected = page.selected_id.clone();
                    model.ops.form = ops_form(&page);
                    model.ops.dirty = false;
                    model.ops.saving = false;
                    model.ops.person_query.clear();
                    model.ops.person_people.clear();
                    model.ops.person_searching = false;
                    model.ops.selected_person = None;
                    model.ops.media_index = 0;
                    // A gallery refresh in the middle of a batch leaves the batch's progress showing.
                    model.ops.media_uploading = model.ops.media_file_name.is_some();
                    if model.ops.creating {
                        model.ops.creating = false;
                        model.ops.new_name.clear();
                        model.ops.new_property_type.clear();
                    }
                    model.error = None;
                    model.read = Remote::Loaded(page);
                }
                // A failed write or refresh keeps the page (and the draft) and says why.
                Err(error) if model.read.loaded().is_some() => {
                    model.ops.saving = false;
                    model.ops.creating = false;
                    model.error = Some(error.message);
                }
                Err(error) => model.read = Remote::Failed(error),
            }
            Cmd::none()
        }

        // ---- the list ---------------------------------------------------------------------------------------------
        Msg::QueryChanged(query) => {
            if guard_draft(model, "searching") {
                return Cmd::none();
            }
            model.controls.query = query;
            model.typed += 1;
            Cmd::after(SEARCH_PAUSE_MS, Msg::SearchPaused(model.typed))
        }
        // FIND: the other record for this parcel (the Regrid load gave every parcel one) is merged into this one —
        // every field this record lacks is filled from it, and it is deleted. Then this record is read again.
        Msg::FindByCatastro => {
            let catastro = model
                .ops
                .form
                .get("catastroNumber")
                .map(|value| value.trim().to_owned())
                .unwrap_or_default();
            let Some(property_id) = model.selected.clone() else {
                return Cmd::none();
            };
            if catastro.is_empty() {
                model.error = Some("Type the catastro number, then FIND.".into());
                return Cmd::none();
            }
            model.error = None;
            model.ops.saving = true;
            Cmd::request(
                PropertyMergeParcel {
                    property_id,
                    catastro,
                },
                Msg::ParcelMerged,
            )
        }
        Msg::ParcelMerged(result) => {
            model.ops.saving = false;
            match result {
                // The fields it filled show on the record as it is read again; the merged record leaves the list.
                Ok(answer)
                    if answer.get("merged").and_then(serde_json::Value::as_bool) == Some(true) =>
                {
                    model.ops.dirty = false;
                    read(model)
                }
                Ok(_) => {
                    model.error = Some("No other record has that catastro number — nothing to merge. Save to keep the number.".into());
                    Cmd::none()
                }
                Err(error) => {
                    model.error = Some(format!("The records were not merged: {}", error.message));
                    Cmd::none()
                }
            }
        }
        Msg::SearchPaused(typed) => {
            if typed != model.typed || model.ops.dirty {
                return Cmd::none();
            }
            model.controls.page = 0;
            model.selected = None;
            model.error = None;
            read(model)
        }
        Msg::PageChanged(delta) => {
            if guard_draft(model, "changing pages") {
                return Cmd::none();
            }
            let pages = model
                .read
                .loaded()
                .map(|page| {
                    let size = page.page_size.max(1);
                    ((page.total + size - 1) / size).max(1)
                })
                .unwrap_or(1);
            let next = (model.controls.page as i64)
                .saturating_add(delta)
                .clamp(0, pages - 1) as usize;
            if next == model.controls.page {
                return Cmd::none();
            }
            model.controls.page = next;
            model.selected = None;
            model.error = None;
            read(model)
        }
        Msg::RowSelected(id) => {
            if guard_draft(model, "switching records") {
                return Cmd::none();
            }
            let listed = model
                .read
                .loaded()
                .is_some_and(|page| page.rows.iter().any(|row| row.id == id));
            if !listed {
                return Cmd::none();
            }
            model.selected = Some(id);
            reset_aux(&mut model.ops);
            model.error = None;
            read(model)
        }
        Msg::OpsEntitySelected(entity) => {
            if !matches!(entity.as_str(), "property" | "person" | "project")
                || model.ops.entity == entity
            {
                return Cmd::none();
            }
            if guard_draft(model, "switching data types") {
                return Cmd::none();
            }
            model.ops.section = ops_default_section(&entity);
            model.ops.entity = entity;
            model.ops.form.clear();
            model.ops.creating = false;
            model.ops.new_name.clear();
            model.ops.new_property_type.clear();
            reset_aux(&mut model.ops);
            model.selected = None;
            model.controls = Controls::default();
            model.error = None;
            read(model)
        }
        Msg::OpsSectionSelected(section) => {
            let valid = match model.ops.entity.as_str() {
                "person" => matches!(section.as_str(), "identity" | "contact" | "relations"),
                "project" => matches!(section.as_str(), "project" | "links"),
                _ => matches!(
                    section.as_str(),
                    "property"
                        | "site"
                        | "legal"
                        | "website"
                        | "mls"
                        | "photos"
                        | "video"
                        | "person"
                        | "sources"
                ),
            };
            if valid {
                model.ops.section = section;
                model.error = None;
            }
            Cmd::none()
        }
        Msg::OpsRailToggled => {
            model.ops.rail_collapsed = !model.ops.rail_collapsed;
            Cmd::none()
        }

        // ---- the draft ------------------------------------------------------------------------------------------------
        Msg::OpsFieldChanged { key, value } => {
            if model.selected.is_some() {
                model.ops.form.insert(key, value);
                model.ops.dirty = true;
                model.error = None;
            }
            Cmd::none()
        }
        Msg::OpsSaveRequested => {
            if model.ops.saving || !model.ops.dirty {
                return Cmd::none();
            }
            let Some(id) = model.selected.clone() else {
                model.error = Some("Select a record before saving.".into());
                return Cmd::none();
            };
            model.ops.saving = true;
            model.error = None;
            let body = serde_json::json!({
                "action": "save",
                "entity": model.ops.entity,
                "id": id,
                "fields": model.ops.form,
                "search": model.controls.query,
                "page": model.controls.page,
            });
            write(model, body)
        }
        Msg::OpsRevertRequested => {
            if let Some(page) = model.read.loaded() {
                model.ops.form = ops_form(page);
                model.ops.dirty = false;
                model.ops.person_query.clear();
                model.ops.person_people.clear();
                model.ops.person_searching = false;
                model.ops.selected_person = None;
                model.error = None;
            }
            Cmd::none()
        }

        // ---- a new property -------------------------------------------------------------------------------------------
        Msg::OpsCreateToggled => {
            if model.ops.entity != "property" || guard_draft(model, "creating another property") {
                return Cmd::none();
            }
            model.ops.creating = !model.ops.creating;
            if !model.ops.creating {
                model.ops.new_name.clear();
                model.ops.new_property_type.clear();
            }
            model.error = None;
            Cmd::none()
        }
        Msg::OpsCreateNameChanged(value) => {
            model.ops.new_name = value;
            model.error = None;
            Cmd::none()
        }
        Msg::OpsCreateTypeChanged(value) => {
            model.ops.new_property_type = value;
            Cmd::none()
        }
        Msg::OpsCreateRequested => {
            if model.ops.entity != "property" || !model.ops.creating || model.loading {
                return Cmd::none();
            }
            let name = model.ops.new_name.trim().to_string();
            if name.is_empty() {
                model.error = Some("Property name is required.".into());
                return Cmd::none();
            }
            // The answer opens the new property: nothing else is selected while it is made.
            model.controls = Controls::default();
            model.selected = None;
            model.error = None;
            let body = serde_json::json!({
                "action": "createProperty",
                "name": name,
                "propertyType": model.ops.new_property_type,
            });
            write(model, body)
        }

        // ---- the property's seller ------------------------------------------------------------------------------------
        Msg::OpsPersonQueryChanged(value) => {
            if model.ops.entity != "property" {
                return Cmd::none();
            }
            model.ops.person_query = value.clone();
            model.ops.person_people.clear();
            model.ops.selected_person = None;
            model.error = None;
            if value.trim().chars().count() < 2 {
                model.ops.person_searching = false;
                return Cmd::none();
            }
            model.ops.person_searching = true;
            let query = value;
            Cmd::request(
                DealPeopleSearch {
                    query: query.clone(),
                },
                move |answer| Msg::PeopleLoaded { query, answer },
            )
        }
        Msg::PeopleLoaded { query, answer } => {
            if model.ops.person_query != query {
                return Cmd::none();
            }
            model.ops.person_searching = false;
            match answer {
                Ok(found) => model.ops.person_people = found.people,
                Err(error) => model.error = Some(error.message),
            }
            Cmd::none()
        }
        Msg::OpsPersonSelected(id) => {
            if model.ops.entity != "property" || model.selected.is_none() {
                return Cmd::none();
            }
            if id.is_empty() {
                model
                    .ops
                    .form
                    .insert("sellerPersonId".into(), String::new());
                model.ops.selected_person = None;
                model.ops.person_query.clear();
            } else {
                let Some(person) = model
                    .ops
                    .person_people
                    .iter()
                    .find(|person| person.id == id)
                    .cloned()
                else {
                    return Cmd::none();
                };
                model
                    .ops
                    .form
                    .insert("sellerPersonId".into(), person.id.clone());
                model.ops.person_query = person.display_name.clone();
                model.ops.selected_person = Some(person);
            }
            model.ops.person_people.clear();
            model.ops.person_searching = false;
            model.ops.dirty = true;
            model.error = None;
            Cmd::none()
        }

        // ---- photographs and video -----------------------------------------------------------------------------------
        Msg::OpsMediaSelected(index) => {
            model.ops.media_confirm_delete = None;
            let images = model
                .read
                .loaded()
                .map(|page| {
                    page.media
                        .iter()
                        .filter(|media| media.media_type == "image")
                        .count()
                })
                .unwrap_or(0);
            if index < images {
                model.ops.media_index = index;
            }
            Cmd::none()
        }
        Msg::OpsMediaUploaderToggled => {
            if model.ops.section == "photos" && !model.ops.media_uploading {
                model.ops.media_uploader_open = !model.ops.media_uploader_open;
                model.error = None;
            }
            Cmd::none()
        }
        Msg::OpsMediaRoleChanged(value) => {
            if matches!(value.as_str(), "hero" | "gallery") {
                model.ops.media_role = value;
                model.error = None;
            }
            Cmd::none()
        }
        Msg::OpsMediaAltChanged(value) => {
            model.ops.media_alt = value;
            model.error = None;
            Cmd::none()
        }
        Msg::OpsMediaFileChosen(file) => {
            // ONE BUTTON: choosing IS uploading — a single photo is a batch of one.
            let Some(file) = file else {
                model.ops.media_file_name = None;
                return Cmd::none();
            };
            update(model, Msg::OpsMediaFilesChosen(vec![file]))
        }
        Msg::OpsMediaFilesChosen(files) => {
            if model.ops.entity != "property" || model.ops.media_uploading {
                return Cmd::none();
            }
            // Photos only, in name order (a folder uploads in the order it is sorted on disk).
            let mut files: Vec<crate::app::exec::File> = files
                .into_iter()
                .filter(|file| file.type_().starts_with("image/"))
                .collect();
            files.sort_by_key(|file| file.name().to_lowercase());
            if files.is_empty() {
                model.error = Some("No photos were chosen.".into());
                return Cmd::none();
            }
            model.ops.media_batch_total = files.len();
            model.ops.media_batch_done = 0;
            model.ops.media_batch_failed.clear();
            model.upload_retried.clear();
            model.ops.media_batch_property = model.selected.clone();
            model.ops.media_batch_first =
                Some(media_title(model.read.loaded())).filter(|title| !title.is_empty());
            model.upload_queue = files;
            next_upload(model)
        }
        Msg::MakeHero(media_id) => {
            let Some(property_id) = model.selected.clone() else {
                return Cmd::none();
            };
            model.error = None;
            Cmd::request(
                PropertyHero {
                    property_id,
                    media_id,
                },
                Msg::HeroSet,
            )
        }
        Msg::DeletePhoto(media_id) => {
            if model.ops.media_confirm_delete.as_deref() != Some(media_id.as_str()) {
                model.ops.media_confirm_delete = Some(media_id);
                return Cmd::none();
            }
            model.ops.media_confirm_delete = None;
            let Some(property_id) = model.selected.clone() else {
                return Cmd::none();
            };
            model.error = None;
            Cmd::request(
                PropertyMediaRemove {
                    property_id,
                    media_id,
                },
                Msg::PhotoDeleted,
            )
        }
        Msg::PhotoDeleted(result) => match result {
            Ok(_) => {
                model.ops.media_index = model.ops.media_index.saturating_sub(1);
                read(model)
            }
            Err(error) => {
                model.error = Some(format!("The photo was not deleted: {}", error.message));
                Cmd::none()
            }
        },
        Msg::HeroSet(result) => match result {
            Ok(_) => {
                model.ops.media_index = 0;
                read(model)
            }
            Err(error) => {
                model.error = Some(format!("The hero was not changed: {}", error.message));
                Cmd::none()
            }
        },
        Msg::Uploaded(result) => {
            model.ops.media_uploading = false;
            match result {
                Ok(()) => model.ops.media_batch_done += 1,
                Err(error) => {
                    let name = model.ops.media_file_name.clone().unwrap_or_default();
                    match model.upload_current.take() {
                        // One more try, at the back of the line: it resumes with the pieces the server kept.
                        Some(file) if !model.upload_retried.contains(&name) => {
                            model.upload_retried.push(name);
                            model.upload_queue.push(file);
                        }
                        _ => model
                            .ops
                            .media_batch_failed
                            .push(format!("{name} ({})", error.message)),
                    }
                }
            }
            model.ops.media_file_name = None;
            if !model.upload_queue.is_empty() {
                // Each photo shows in the gallery as soon as it is saved, while the next one uploads.
                let refresh = read(model);
                return Cmd::batch([refresh, next_upload(model)]);
            }
            // The batch is finished: say what did not make it, then show the gallery as it now is.
            model.ops.media_alt.clear();
            model.ops.media_uploader_open = false;
            model.ops.media_index = 0;
            model.error = (!model.ops.media_batch_failed.is_empty()).then(|| {
                format!(
                    "{} of {} photos added. Not added: {}",
                    model.ops.media_batch_done,
                    model.ops.media_batch_total,
                    model.ops.media_batch_failed.join("; ")
                )
            });
            read(model)
        }
        Msg::OpsVideoRefreshRequested => read(model),
        Msg::VideoRoleChanged(role) => {
            model.ops.video_role = role;
            Cmd::none()
        }
        Msg::VideoCaptionChanged(caption) => {
            model.ops.video_caption = caption;
            Cmd::none()
        }
        Msg::VideoChosen(file) => {
            if model.ops.video_file_name.is_some() {
                return Cmd::none();
            }
            let Some(property_id) = model.selected.clone() else {
                model.error = Some("Select a Property before uploading.".into());
                return Cmd::none();
            };
            if !file.type_().starts_with("video/") {
                model.error = Some(format!("{} is not a video.", file.name()));
                return Cmd::none();
            }
            model.error = None;
            model.ops.video_file_name = Some(file.name());
            model.ops.video_progress = Some((0.0, file.size(), "uploading".into()));
            Cmd::video_upload(
                file,
                property_id,
                model.ops.video_role.clone(),
                model.ops.video_caption.trim().to_owned(),
                Msg::VideoProgressed,
                Msg::VideoUploaded,
            )
        }
        Msg::VideoProgressed(step) => {
            model.ops.video_progress = Some((step.sent, step.total, step.stage.to_owned()));
            Cmd::none()
        }
        Msg::VideoUploaded(result) => {
            let name = model.ops.video_file_name.take().unwrap_or_default();
            model.ops.video_progress = None;
            match result {
                Ok(()) => {
                    model.ops.video_caption.clear();
                    read(model)
                }
                Err(error) => {
                    model.error = Some(format!(
                        "{name} was not added: {} Choose it again to resume where it stopped.",
                        error.message
                    ));
                    Cmd::none()
                }
            }
        }
    }
}
