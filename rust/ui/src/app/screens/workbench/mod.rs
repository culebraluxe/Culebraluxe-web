//! OPPS — the Data Workbench (`/portal/property-admin`) and a property's record (`/portal/property-admin/:propertyId`,
//! the same workbench opened on that property).
//!
//! One selector/editor over three record types (property, person, project). The screen owns the list (search, paging,
//! selection), the draft form, creating a property, the seller search, and photographs; the relay answers every read
//! and write with the refreshed page, and the draft is rebuilt from it (`crate::ops::ops_form`).
//!
//! RULES THAT PROTECT A DRAFT. With unsaved changes the list cannot be searched, paged or switched — Save or Revert
//! first — because the answer would replace the draft. Every read carries a sequence number and only the newest answer
//! is taken, so a slow answer for a record the operator has left can never overwrite the one they are editing.
//!
//! PHOTOS are sent with the chunked-upload command (`Cmd::upload`): choosing a file IS the upload.

mod view;

use yew::prelude::*;

use crate::app::api::{DealPeopleSearch, OpsCommand, OpsRead, PROPERTY_MEDIA_CHUNKED};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{OpsWorkbenchState, PortalDealPeopleSearch, PortalOpsWorkbenchPage, PortalPage};
use crate::ops::{ops_default_section, ops_form};

/// How long typing must pause before the list is searched.
const SEARCH_PAUSE_MS: u32 = 300;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    pub query: String,
    /// 0-based.
    pub page: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalOpsWorkbenchPage>,
    /// A read or write is in flight; the page stays on screen.
    pub loading: bool,
    pub selected: Option<String>,
    pub ops: OpsWorkbenchState,
    pub controls: Controls,
    pub error: Option<String>,
    /// The newest read's number; an answer with another is stale and dropped.
    seq: u32,
    /// The newest keystroke's number, for the search pause.
    typed: u32,
}

#[derive(Debug)]
pub enum Msg {
    Loaded {
        seq: u32,
        answer: Result<PortalPage, ApiError>,
    },
    PeopleLoaded {
        query: String,
        answer: Result<PortalDealPeopleSearch, ApiError>,
    },
    Uploaded(Result<(), ApiError>),
    SearchPaused(u32),
    /// An event from the video island (`{ "type": "refresh" }` after an upload).
    Video(serde_json::Value),

    QueryChanged(String),
    PageChanged(i64),
    RowSelected(String),
    OpsEntitySelected(String),
    OpsSectionSelected(String),
    OpsRailToggled,
    OpsFieldChanged {
        key: String,
        value: String,
    },
    OpsSaveRequested,
    OpsRevertRequested,
    OpsCreateToggled,
    OpsCreateNameChanged(String),
    OpsCreateTypeChanged(String),
    OpsCreateRequested,
    OpsPersonQueryChanged(String),
    OpsPersonSelected(String),
    OpsMediaSelected(usize),
    OpsMediaUploaderToggled,
    OpsMediaRoleChanged(String),
    OpsMediaAltChanged(String),
    OpsMediaFileChosen(Option<web_sys::File>),
    OpsVideoRefreshRequested,
}

pub struct Workbench;

/// Read the page for the current entity, search, page and selection.
fn read(model: &mut Model) -> Cmd<Msg> {
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
fn write(model: &mut Model, body: serde_json::Value) -> Cmd<Msg> {
    model.seq += 1;
    model.loading = true;
    let seq = model.seq;
    Cmd::request(OpsCommand { body }, move |answer| Msg::Loaded {
        seq,
        answer,
    })
}

fn reset_aux(ops: &mut OpsWorkbenchState) {
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
fn media_title(page: Option<&PortalOpsWorkbenchPage>) -> String {
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
    format!("{stem}_{}", images + 1)
}

/// Refuse a list change while the draft is unsaved, and say which action was refused.
fn guard_draft(model: &mut Model, action: &str) -> bool {
    if model.ops.dirty {
        model.error = Some(format!("Save or Revert changes before {action}."));
        return true;
    }
    false
}

fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
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
                    model.ops.media_uploading = false;
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
            // ONE BUTTON: choosing IS uploading. The role defaults to gallery and the title is derived.
            let Some(file) = file else {
                model.ops.media_file_name = None;
                return Cmd::none();
            };
            if model.ops.entity != "property" || model.ops.media_uploading {
                return Cmd::none();
            }
            let Some(property_id) = model.selected.clone() else {
                model.error = Some("Select a Property before uploading.".into());
                return Cmd::none();
            };
            model.ops.media_file_name = Some(file.name());
            model.ops.media_role = "gallery".into();
            model.ops.media_alt = media_title(model.read.loaded());
            model.ops.media_uploading = true;
            model.error = None;
            let mut init = vec![("role".to_string(), model.ops.media_role.clone())];
            if !model.ops.media_alt.trim().is_empty() {
                init.push((
                    "altText".to_string(),
                    model.ops.media_alt.trim().to_string(),
                ));
            }
            Cmd::upload(
                file,
                PROPERTY_MEDIA_CHUNKED,
                vec![("propertyId".to_string(), property_id)],
                init,
                Msg::Uploaded,
            )
        }
        Msg::Uploaded(result) => {
            model.ops.media_uploading = false;
            model.ops.media_file_name = None;
            match result {
                Ok(()) => {
                    model.ops.media_alt.clear();
                    model.ops.media_uploader_open = false;
                    model.ops.media_index = 0;
                    model.error = None;
                    read(model)
                }
                Err(error) => {
                    model.error = Some(format!("The photo was not added: {}", error.message));
                    Cmd::none()
                }
            }
        }
        Msg::OpsVideoRefreshRequested => read(model),
        Msg::Video(event) => {
            if event.get("type").and_then(|kind| kind.as_str()) == Some("refresh") {
                read(model)
            } else {
                Cmd::none()
            }
        }
    }
}

impl Screen for Workbench {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        // The record route opens the workbench on that property.
        let mut model = Model {
            read: Remote::Loading,
            selected: ctx
                .id
                .clone()
                .or_else(|| ctx.query("selected").map(str::to_owned)),
            ..Model::default()
        };
        let read = read(&mut model);
        (model, read)
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        template::remote(&model.read, "the Workbench", |data| {
            view::workbench(
                &Vm {
                    data,
                    ops: &model.ops,
                    controls: &model.controls,
                    loading: model.loading,
                    error: model.error.clone(),
                    ctx,
                },
                &on_msg,
            )
        })
    }
}

/// What the view reads. Read-only.
pub struct Vm<'a> {
    data: &'a PortalOpsWorkbenchPage,
    pub ops: &'a OpsWorkbenchState,
    pub controls: &'a Controls,
    pub loading: bool,
    pub error: Option<String>,
    ctx: &'a ScreenCtx,
}

impl<'a> Vm<'a> {
    pub fn can(&self, action: &str) -> bool {
        self.ctx.can(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(selected: &str, name: &str) -> serde_json::Value {
        json!({ "ops": {
            "entity": "property", "total": 120, "page": 1, "pageSize": 50,
            "rows": [{ "id": "p1", "title": "Villa" }, { "id": "p2", "title": "Casa" }],
            "selectedId": selected,
            "property": { "id": selected, "name": name, "status": "active" },
            "media": [{ "id": "m1", "mediaType": "image" }, { "id": "m2", "mediaType": "image" }]
        } })
    }

    fn opened(ctx: &ScreenCtx) -> Model {
        let (mut model, cmd) = Workbench::init(ctx);
        let request = cmd.into_requests().remove(0);
        Workbench::update(
            &mut model,
            request.respond(Ok(page("p1", "Villa del Mar"))),
            ctx,
        );
        model
    }

    #[test]
    fn the_record_route_opens_the_workbench_on_that_property() {
        let ctx = ScreenCtx {
            id: Some("p1".into()),
            ..ScreenCtx::default()
        };
        let (_, cmd) = Workbench::init(&ctx);
        assert_eq!(
            cmd.into_requests().remove(0).path,
            "/api/portal/rust-ui/opps?entity=property&page=0&search=&selected=p1"
        );
        let model = opened(&ctx);
        assert_eq!(
            model.ops.form.get("name").map(String::as_str),
            Some("Villa del Mar")
        );
        assert_eq!(
            model.ops.form.get("status").map(String::as_str),
            Some("active")
        );
    }

    #[test]
    fn a_draft_blocks_leaving_it_and_saves_once() {
        let ctx = ScreenCtx::default();
        let mut model = opened(&ctx);
        Workbench::update(
            &mut model,
            Msg::OpsFieldChanged {
                key: "name".into(),
                value: "Villa Mar".into(),
            },
            &ctx,
        );
        assert!(
            Workbench::update(&mut model, Msg::RowSelected("p2".into()), &ctx)
                .into_requests()
                .is_empty()
        );
        assert_eq!(
            model.error.as_deref(),
            Some("Save or Revert changes before switching records.")
        );
        assert!(Workbench::update(&mut model, Msg::PageChanged(1), &ctx)
            .into_requests()
            .is_empty());

        let save = Workbench::update(&mut model, Msg::OpsSaveRequested, &ctx)
            .into_requests()
            .remove(0);
        let body = save.body.clone().unwrap();
        assert_eq!(
            (
                body["action"].as_str(),
                body["id"].as_str(),
                body["fields"]["name"].as_str()
            ),
            (Some("save"), Some("p1"), Some("Villa Mar"))
        );
        assert!(
            Workbench::update(&mut model, Msg::OpsSaveRequested, &ctx)
                .into_requests()
                .is_empty(),
            "one save at a time"
        );
        Workbench::update(
            &mut model,
            save.respond(Err(ApiError::network("Slug is taken."))),
            &ctx,
        );
        assert_eq!(model.error.as_deref(), Some("Slug is taken."));
        assert!(
            model.ops.dirty && !model.ops.saving,
            "a refused save keeps the draft"
        );
    }

    #[test]
    fn only_the_newest_read_is_taken() {
        let ctx = ScreenCtx::default();
        let mut model = opened(&ctx);
        let slow = Workbench::update(&mut model, Msg::RowSelected("p2".into()), &ctx)
            .into_requests()
            .remove(0);
        let fast = Workbench::update(&mut model, Msg::RowSelected("p1".into()), &ctx)
            .into_requests()
            .remove(0);
        Workbench::update(
            &mut model,
            fast.respond(Ok(page("p1", "Villa del Mar"))),
            &ctx,
        );
        Workbench::update(&mut model, slow.respond(Ok(page("p2", "Casa Luna"))), &ctx);
        assert_eq!(
            model.selected.as_deref(),
            Some("p1"),
            "the late answer for the record the operator left is dropped"
        );
    }

    #[test]
    fn a_photo_title_is_the_property_and_the_next_number() {
        let ctx = ScreenCtx::default();
        let model = opened(&ctx);
        assert_eq!(media_title(model.read.loaded()), "VilladelMar_3");
    }
}
