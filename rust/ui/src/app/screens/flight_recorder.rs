//! TECH — the Flight Recorder for one process instance (`/portal/tech/flight-recorder/:instanceId`).
//!
//! The trace console was a JavaScript widget and was deleted with the rest of the TypeScript (owner decision,
//! 2026-09-26). The route stays so links into it keep landing somewhere honest; the console returns as a Rust port.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;

pub struct FlightRecorder;

impl Screen for FlightRecorder {
    type Model = ();
    type Msg = ();

    fn init(_ctx: &ScreenCtx) -> ((), Cmd<()>) {
        ((), Cmd::none())
    }

    fn update(_model: &mut (), _msg: (), _ctx: &ScreenCtx) -> Cmd<()> {
        Cmd::none()
    }

    fn view(_model: &(), ctx: &ScreenCtx, _link: &Link<()>) -> Html {
        html! {
            <div class="space-y-3">
                { template::widget_removed("The flight recorder trace view") }
                if let Some(id) = ctx.id.clone() {
                    <p class="text-center font-mono text-[10px] text-black/40">{ id }</p>
                }
            </div>
        }
    }
}
