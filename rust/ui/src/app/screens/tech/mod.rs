//! TECH — the Forge Cockpit (`/portal/tech`): story supply, the Workbench, Flight staging, the engine lanes and the last
//! outcomes, with one selected story's introspection.
//!
//! Yew owns the read, the selection, every command and the 30-second refresh the Cockpit has always promised. The story
//! sorter is the `tech-sorter` island (drag mechanics only); it asks for a refresh through its events.
//!
//! One command at a time (`busy_action`). Every answer, success or refusal, is said in the notice and followed by a
//! re-read, because the engine may have moved either way.

mod view;

use yew::prelude::*;

use crate::app::api::{TechCommand, TechRead};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{CommandNotice, PortalPage, PortalTechPage, TechCockpitState};

const REFRESH_MS: u32 = 30_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controls {
    /// The Workbench is collapsed.
    pub toggled: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalTechPage>,
    pub loading: bool,
    pub selected: Option<String>,
    pub tech: TechCockpitState,
    pub controls: Controls,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    Tick,
    /// An event from the sorter island (`{ "type": "refresh" }`).
    Sorter(serde_json::Value),
    TechStorySelected(String),
    TechRefreshRequested,
    TechScheduleChanged(String),
    TechClearWorkbenchRequested,
    TechGoodToGoRequested,
    TechScopedRunRequested(String),
    TechMoveWorkbenchRequested(String),
    TechLaunchFlightRequested,
    TechScheduleFlightRequested {
        scheduled_for: String,
    },
    TechCancelFlightRequested(String),
    /// The Workbench's open state, as it was when the toggle was pressed.
    Toggled(bool),
    CommandAnswered(Result<serde_json::Value, ApiError>),
}

pub struct TechCockpit;

fn read(model: &mut Model) -> Cmd<Msg> {
    model.loading = true;
    Cmd::request(
        TechRead {
            selected: model.selected.clone(),
        },
        Msg::Loaded,
    )
}

fn command(model: &mut Model, busy: impl Into<String>, body: serde_json::Value) -> Cmd<Msg> {
    if model.tech.busy_action.is_some() {
        return Cmd::none();
    }
    model.tech.busy_action = Some(busy.into());
    model.tech.notice = None;
    Cmd::request(TechCommand { body }, Msg::CommandAnswered)
}

impl Screen for TechCockpit {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let mut model = Model {
            read: Remote::Loading,
            ..Model::default()
        };
        let read = read(&mut model);
        (model, Cmd::batch([read, Cmd::after(REFRESH_MS, Msg::Tick)]))
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.loading = false;
                match answer.and_then(|page| {
                    page.tech
                        .ok_or_else(|| ApiError::decode("The answer had no Cockpit in it."))
                }) {
                    Ok(tech) => model.read = Remote::Loaded(tech),
                    // A failed refresh keeps the line on screen and says why.
                    Err(error) if model.read.loaded().is_some() => {
                        model.tech.notice = Some(CommandNotice::failure(error.message))
                    }
                    Err(error) => model.read = Remote::Failed(error),
                }
                Cmd::none()
            }
            Msg::Tick => {
                let refresh = if model.loading || model.read.loaded().is_none() {
                    Cmd::none()
                } else {
                    read(model)
                };
                Cmd::batch([refresh, Cmd::after(REFRESH_MS, Msg::Tick)])
            }
            Msg::Sorter(event) => {
                if event.get("type").and_then(|kind| kind.as_str()) == Some("refresh")
                    && !model.loading
                {
                    read(model)
                } else {
                    Cmd::none()
                }
            }
            Msg::TechStorySelected(id) => {
                model.selected = Some(id);
                read(model)
            }
            Msg::TechRefreshRequested => read(model),
            Msg::TechScheduleChanged(value) => {
                model.tech.schedule_at = value;
                model.tech.notice = None;
                Cmd::none()
            }
            Msg::Toggled(open) => {
                model.controls.toggled = open;
                Cmd::none()
            }
            Msg::TechClearWorkbenchRequested => command(
                model,
                "clearWorkbench",
                serde_json::json!({ "action": "clearWorkbench" }),
            ),
            Msg::TechGoodToGoRequested => match model.selected.clone() {
                Some(story_id) => command(
                    model,
                    "goodToGo",
                    serde_json::json!({ "action": "goodToGo", "storyId": story_id }),
                ),
                None => Cmd::none(),
            },
            Msg::TechScopedRunRequested(stop_after) => {
                match (
                    model.selected.clone(),
                    matches!(stop_after.as_str(), "scout" | "architect" | "lead"),
                ) {
                    (Some(story_id), true) => command(
                        model,
                        format!("scoped:{stop_after}"),
                        serde_json::json!({ "action": "scopedRun", "storyId": story_id, "stopAfter": stop_after }),
                    ),
                    _ => Cmd::none(),
                }
            }
            Msg::TechMoveWorkbenchRequested(target) => {
                match (
                    model.selected.clone(),
                    matches!(target.as_str(), "backlog" | "closed" | "next"),
                ) {
                    (Some(story_id), true) => command(
                        model,
                        format!("move:{target}"),
                        serde_json::json!({ "action": "moveWorkbench", "storyId": story_id, "target": target }),
                    ),
                    _ => Cmd::none(),
                }
            }
            Msg::TechLaunchFlightRequested => command(
                model,
                "launchFlight",
                serde_json::json!({ "action": "launchFlight" }),
            ),
            Msg::TechScheduleFlightRequested { scheduled_for }
                if !scheduled_for.trim().is_empty() =>
            {
                command(
                    model,
                    "scheduleFlight",
                    serde_json::json!({ "action": "scheduleFlight", "scheduledFor": scheduled_for }),
                )
            }
            Msg::TechCancelFlightRequested(batch_id) if !batch_id.trim().is_empty() => command(
                model,
                "cancelFlight",
                serde_json::json!({ "action": "cancelFlight", "batchId": batch_id }),
            ),
            Msg::TechScheduleFlightRequested { .. } | Msg::TechCancelFlightRequested(_) => {
                Cmd::none()
            }
            Msg::CommandAnswered(answer) => {
                model.tech.busy_action = None;
                model.tech.notice = Some(match answer {
                    Ok(value) => {
                        let message = value
                            .get("message")
                            .and_then(|message| message.as_str())
                            .unwrap_or("TECH command completed.");
                        // `{ ok: false, message }` is a refusal whichever way it arrived.
                        if value.get("ok").and_then(|ok| ok.as_bool()) == Some(false) {
                            CommandNotice::failure(message)
                        } else {
                            CommandNotice::success(message)
                        }
                    }
                    Err(error) => CommandNotice::failure(error.message),
                });
                read(model)
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        template::remote(&model.read, "the Forge line", |tech| {
            view::cockpit(
                &Vm {
                    loading: model.loading,
                    tech: &model.tech,
                    controls: &model.controls,
                },
                tech,
                &on_msg,
            )
        })
    }
}

/// What the view reads besides the Cockpit itself.
pub struct Vm<'a> {
    pub loading: bool,
    pub tech: &'a TechCockpitState,
    pub controls: &'a Controls,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selection_rereads_commands_run_alone_and_every_answer_rereads() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = TechCockpit::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/tech");
        TechCockpit::update(
            &mut model,
            request.respond(Ok(json!({ "tech": { "ready": true } }))),
            &ctx,
        );
        assert!(model.read.loaded().is_some());

        let reread = TechCockpit::update(&mut model, Msg::TechStorySelected("S-1".into()), &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(reread.path, "/api/portal/rust-ui/tech?selected=S-1");
        TechCockpit::update(&mut model, reread.respond(Ok(json!({ "tech": {} }))), &ctx);

        let go = TechCockpit::update(&mut model, Msg::TechGoodToGoRequested, &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(
            go.body,
            Some(json!({ "action": "goodToGo", "storyId": "S-1" }))
        );
        assert!(
            TechCockpit::update(&mut model, Msg::TechLaunchFlightRequested, &ctx)
                .into_requests()
                .is_empty()
        );
        let after = TechCockpit::update(
            &mut model,
            go.respond(Ok(json!({ "ok": false, "message": "Engine busy." }))),
            &ctx,
        );
        assert_eq!(
            model.tech.notice,
            Some(CommandNotice::failure("Engine busy."))
        );
        assert_eq!(after.into_requests().len(), 1, "a refusal re-reads too");
        assert!(model.tech.busy_action.is_none());
    }

    #[test]
    fn the_sorter_asks_for_a_refresh_through_its_events() {
        let ctx = ScreenCtx::default();
        let mut model = Model {
            read: Remote::Loaded(PortalTechPage::default()),
            ..Model::default()
        };
        assert_eq!(
            TechCockpit::update(&mut model, Msg::Sorter(json!({ "type": "refresh" })), &ctx)
                .into_requests()
                .len(),
            1
        );
    }
}
