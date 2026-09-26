//! TECH — the Flight Recorder for one process instance (`/portal/tech/flight-recorder/:instanceId`).
//!
//! Yew owns the instance, the read, its refresh (every 30 seconds while the screen is open, as the console promises)
//! and the failure states. The console itself — virtualized events, SVG graphs, swimlanes, the inspector — is the
//! `flight-recorder` island: it gets one immutable transaction snapshot and never fetches.
//!
//! A FAILED REFRESH KEEPS THE LAST GOOD TRACE on screen and says so above it, with a retry.

use yew::prelude::*;

use crate::app::api::FlightRecorderRead;
use crate::app::cmd::{ApiError, Cmd};
use crate::app::island::Island;
use crate::app::screen::{Link, Screen, ScreenCtx};

/// How often the open console re-reads its trace.
const REFRESH_MS: u32 = 30_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    /// The last good transaction, as the JSON the console is handed.
    pub transaction: Option<AttrValue>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<serde_json::Value, ApiError>),
    RefreshRequested,
    /// The 30-second timer. It re-arms itself, and does nothing while a read is in flight.
    Tick,
}

pub struct FlightRecorder;

fn read(ctx: &ScreenCtx, model: &mut Model) -> Cmd<Msg> {
    let Some(instance_id) = ctx.id.clone().filter(|id| !id.trim().is_empty()) else {
        model.error = Some("Flight Recorder requires a process-instance id.".into());
        return Cmd::none();
    };
    model.loading = true;
    Cmd::request(FlightRecorderRead { instance_id }, Msg::Loaded)
}

impl Screen for FlightRecorder {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let mut model = Model::default();
        let read = read(ctx, &mut model);
        (model, Cmd::batch([read, Cmd::after(REFRESH_MS, Msg::Tick)]))
    }

    fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.loading = false;
                match answer {
                    Ok(transaction) => {
                        model.transaction = Some(AttrValue::from(transaction.to_string()));
                        model.error = None;
                    }
                    Err(error) => model.error = Some(error.message),
                }
                Cmd::none()
            }
            Msg::RefreshRequested if !model.loading => {
                model.error = None;
                read(ctx, model)
            }
            Msg::RefreshRequested => Cmd::none(),
            Msg::Tick => {
                let refresh = if model.loading || model.transaction.is_none() {
                    Cmd::none()
                } else {
                    read(ctx, model)
                };
                Cmd::batch([refresh, Cmd::after(REFRESH_MS, Msg::Tick)])
            }
        }
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let refresh = link.callback(|_: MouseEvent| Msg::RefreshRequested);
        html! {
            <div class="relative min-w-0 overflow-hidden rounded-xl border border-white/10 bg-[#0b1220]">
                if let Some(transaction) = model.transaction.clone() {
                    <Island kind="flight-recorder" props={transaction}
                        class="h-[calc(100vh-6.5rem)] min-h-[44rem] w-full overflow-hidden" />
                    if model.loading {
                        <div class="pointer-events-none absolute right-4 top-4 z-[80] rounded-full border border-[#c6a15b]/30 bg-[#0b1220]/90 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[#e0c489] shadow-lg">
                            {"Refreshing"}
                        </div>
                    }
                    if let Some(error) = model.error.as_deref() {
                        <div class="absolute inset-x-4 top-16 z-[80] flex items-center justify-between gap-3 rounded-md border border-amber-400/25 bg-[#111827]/95 px-3 py-2 text-[10px] text-amber-100 shadow-xl" role="alert">
                            <span>{ format!("Refresh failed — showing the last good trace. {error}") }</span>
                            <button type="button" onclick={refresh}
                                class="shrink-0 rounded border border-amber-300/25 px-2 py-1 uppercase tracking-[0.1em] text-amber-200 hover:border-amber-300/50">
                                {"Retry"}
                            </button>
                        </div>
                    }
                } else if let Some(error) = model.error.as_deref() {
                    <div class="grid min-h-[42rem] place-items-center bg-[#0b1220] p-6 text-slate-300">
                        <div class="max-w-xl rounded-lg border border-rose-400/25 bg-rose-400/[0.05] px-6 py-5" role="alert">
                            <p class="text-[10px] font-semibold uppercase tracking-[0.16em] text-rose-300">
                                {"Flight Recorder could not load"}
                            </p>
                            <p class="mt-2 text-sm leading-6 text-slate-300">{ error }</p>
                            <button type="button" onclick={refresh}
                                class="mt-4 rounded-md border border-white/15 px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white hover:border-[#c6a15b]/50 hover:text-[#e0c489]">
                                {"Retry"}
                            </button>
                        </div>
                    </div>
                } else {
                    <div class="grid min-h-[42rem] place-items-center bg-[#0b1220] text-sm text-slate-400" data-screen-state="loading">
                        <div class="text-center">
                            <div class="mx-auto h-7 w-7 animate-spin rounded-full border-2 border-white/10 border-t-[#c6a15b]" />
                            <p class="mt-3">{"Loading trace…"}</p>
                            if let Some(id) = ctx.id.clone() {
                                <p class="mt-1 font-mono text-[9px] text-slate-600">{ id }</p>
                            }
                        </div>
                    </div>
                }
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> ScreenCtx {
        ScreenCtx {
            id: Some("pi-1".into()),
            ..ScreenCtx::default()
        }
    }

    #[test]
    fn it_reads_its_instance_refreshes_on_a_timer_and_keeps_the_last_good_trace() {
        let ctx = ctx();
        let (mut model, cmd) = FlightRecorder::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/flight-recorder/pi-1");
        FlightRecorder::update(
            &mut model,
            request.respond(Ok(json!({ "events": [] }))),
            &ctx,
        );
        assert!(model.transaction.is_some());

        let refresh = FlightRecorder::update(&mut model, Msg::Tick, &ctx);
        assert_eq!(refresh.into_requests().len(), 1, "the tick re-reads");
        assert!(
            FlightRecorder::update(&mut model, Msg::RefreshRequested, &ctx)
                .into_requests()
                .is_empty(),
            "one read at a time"
        );
        FlightRecorder::update(
            &mut model,
            Msg::Loaded(Err(ApiError::network("down"))),
            &ctx,
        );
        assert!(model.transaction.is_some(), "the last good trace stays");
        assert_eq!(model.error.as_deref(), Some("down"));
    }

    #[test]
    fn no_instance_is_said_not_read() {
        let (model, cmd) = FlightRecorder::init(&ScreenCtx::default());
        assert!(cmd.into_requests().is_empty());
        assert!(model.error.is_some());
    }
}
