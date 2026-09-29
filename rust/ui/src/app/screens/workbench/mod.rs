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

use crate::app::api::{DealPeopleSearch, OpsCommand, OpsRead, PropertyHero, PropertyMediaRemove, PropertyMergeParcel, PropertyMediaChunked};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{OpsWorkbenchState, PortalDealPeopleSearch, PortalOpsWorkbenchPage, PortalPage};
use crate::ops::{ops_default_section, ops_form};
mod update;
#[allow(unused_imports)]
pub(super) use update::*;


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
    /// Photos chosen together, waiting their turn (uploaded one at a time).
    upload_queue: Vec<crate::app::exec::File>,
    /// The photo uploading now, kept so a failure can put it back in the queue.
    upload_current: Option<crate::app::exec::File>,
    /// Photos already put back once: a second failure is reported instead of retried again.
    upload_retried: Vec<String>,
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
    /// Open the property whose catastro number is the one typed on this record (the property table is the source).
    FindByCatastro,
    ParcelMerged(Result<serde_json::Value, ApiError>),
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
    OpsMediaFileChosen(Option<crate::app::exec::File>),
    /// Several photos, or a whole folder: uploaded one after another.
    OpsMediaFilesChosen(Vec<crate::app::exec::File>),
    /// Make this photograph the hero.
    MakeHero(String),
    /// Delete pressed on a photo: the first press asks, the second deletes.
    DeletePhoto(String),
    PhotoDeleted(Result<serde_json::Value, ApiError>),
    HeroSet(Result<serde_json::Value, ApiError>),
    OpsVideoRefreshRequested,
    VideoChosen(crate::app::exec::File),
    VideoRoleChanged(String),
    VideoCaptionChanged(String),
    VideoProgressed(crate::app::cmd::VideoProgress),
    VideoUploaded(Result<(), ApiError>),
}

pub struct Workbench;

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
    fn prices_read_as_us_dollars() {
        assert_eq!(view::usd("2350000"), "$2,350,000");
        assert_eq!(view::usd("950000.5"), "$950,000.5");
        assert_eq!(view::usd(""), "");
        assert_eq!(view::usd("0"), "$0");
    }

    #[test]
    fn find_by_catastro_merges_the_parcel_record_into_the_open_property() {
        let ctx = ScreenCtx::default();
        let mut model = opened(&ctx);
        Workbench::update(
            &mut model,
            Msg::OpsFieldChanged { key: "catastroNumber".into(), value: " 476-000-005-19-000 ".into() },
            &ctx,
        );
        let request = Workbench::update(&mut model, Msg::FindByCatastro, &ctx).into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/property/merge-parcel");
        let body = request.body.expect("a body");
        assert_eq!(body["catastro"], "476-000-005-19-000");
        assert_eq!(body["propertyId"].as_str(), model.selected.as_deref());
    }

    #[test]
    fn a_photo_title_is_the_property_and_the_next_number() {
        let ctx = ScreenCtx::default();
        let model = opened(&ctx);
        assert_eq!(media_title(model.read.loaded()), "VilladelMar_3");
    }

    /// The hand-fix hold is part of the person record: seeded from what is stored, editable, and it travels in the
    /// save body the bridge turns into `manualOverride` (migration 256, `person.manual_override`).
    #[test]
    fn the_person_hand_fix_hold_is_seeded_edited_and_saved() {
        let ctx = ScreenCtx::default();
        let mut model = opened(&ctx);
        let read = Workbench::update(&mut model, Msg::OpsEntitySelected("person".into()), &ctx)
            .into_requests()
            .remove(0);
        Workbench::update(
            &mut model,
            read.respond(Ok(json!({ "ops": {
                "entity": "person", "total": 1, "page": 1, "pageSize": 50,
                "rows": [{ "id": "u1", "title": "Juan" }],
                "selectedId": "u1",
                "person": { "id": "u1", "displayName": "Juan", "role": "unclassified",
                            "status": "new", "manualOverride": true },
            } }))),
            &ctx,
        );
        assert_eq!(
            model.ops.form.get("manualOverride").map(String::as_str),
            Some("true"),
            "the record says it is fixed, so the toggle starts ticked"
        );

        Workbench::update(
            &mut model,
            Msg::OpsFieldChanged {
                key: "manualOverride".into(),
                value: "false".into(),
            },
            &ctx,
        );
        let save = Workbench::update(&mut model, Msg::OpsSaveRequested, &ctx)
            .into_requests()
            .remove(0);
        let body = save.body.expect("a body");
        assert_eq!(body["entity"], "person");
        assert_eq!(
            body["fields"]["manualOverride"], "false",
            "releasing the hold is what the save says"
        );
    }
}
