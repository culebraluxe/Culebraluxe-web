//! Flight Recorder record screen — Yew owns the application state; React renders the specialized console.
//!
//! The canonical recorder transaction is fetched by the Yew effect layer and stored in MVI state. The existing
//! FlightRecorderPage remains a bounded renderer for virtualization, SVG graphs, swimlanes and event inspection.
//! It receives one immutable transaction snapshot from this slot and never performs its own network request.

use yew::prelude::*;

use crate::model::Msg;

#[derive(Properties, PartialEq)]
pub struct FlightRecorderProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct FlightRecorder;

impl Component for FlightRecorder {
    type Message = ();
    type Properties = FlightRecorderProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let model = &ctx.props().model;
        let on_msg = &ctx.props().on_msg;
        let recorder = &model.flight_recorder;

        let refresh = {
            let on_msg = on_msg.clone();
            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::FlightRecorderRefreshRequested))
        };
        let bridge = {
            let on_msg = on_msg.clone();
            Callback::from(move |event: MouseEvent| {
                let target = event.target_unchecked_into::<web_sys::HtmlElement>();
                if target.get_attribute("data-intent").as_deref() == Some("refresh") {
                    on_msg.emit(Msg::FlightRecorderRefreshRequested);
                }
            })
        };

        html! {
            <div
                class="relative min-w-0 overflow-hidden rounded-xl border border-white/10 bg-[#0b1220]"
                data-rust-screen="trace-record"
            >
                <button
                    id="flight-recorder-island-bridge"
                    type="button"
                    class="hidden"
                    data-intent=""
                    tabindex="-1"
                    aria-hidden="true"
                    onclick={bridge}
                />

                if let Some(transaction) = recorder.transaction.as_ref() {
                    <script id="flight-recorder-payload" type="application/json">
                        { transaction.to_string() }
                    </script>
                    <div
                        id="flight-recorder-island"
                        class="h-[calc(100vh-6.5rem)] min-h-[44rem] w-full overflow-hidden"
                        data-instance-id={recorder.instance_id.clone()}
                        aria-label="Flight Recorder console"
                    />
                    if model.loading {
                        <div class="pointer-events-none absolute right-4 top-4 z-[80] rounded-full border border-[#c6a15b]/30 bg-[#0b1220]/90 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[#e0c489] shadow-lg">
                            {"Refreshing"}
                        </div>
                    }
                    if let Some(error) = model.error.as_deref() {
                        <div class="absolute inset-x-4 top-16 z-[80] flex items-center justify-between gap-3 rounded-md border border-amber-400/25 bg-[#111827]/95 px-3 py-2 text-[10px] text-amber-100 shadow-xl">
                            <span>{ format!("Refresh failed — showing the last good trace. {error}") }</span>
                            <button
                                type="button"
                                onclick={refresh.clone()}
                                class="shrink-0 rounded border border-amber-300/25 px-2 py-1 uppercase tracking-[0.1em] text-amber-200 hover:border-amber-300/50"
                            >
                                {"Retry"}
                            </button>
                        </div>
                    }
                } else if let Some(error) = model.error.as_deref() {
                    <div class="grid min-h-[42rem] place-items-center bg-[#0b1220] p-6 text-slate-300">
                        <div class="max-w-xl rounded-lg border border-rose-400/25 bg-rose-400/[0.05] px-6 py-5">
                            <p class="text-[10px] font-semibold uppercase tracking-[0.16em] text-rose-300">
                                {"Flight Recorder could not load"}
                            </p>
                            <p class="mt-2 text-sm leading-6 text-slate-300">{ error }</p>
                            <button
                                type="button"
                                onclick={refresh}
                                class="mt-4 rounded-md border border-white/15 px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white hover:border-[#c6a15b]/50 hover:text-[#e0c489]"
                            >
                                {"Retry"}
                            </button>
                        </div>
                    </div>
                } else {
                    <div class="grid min-h-[42rem] place-items-center bg-[#0b1220] text-sm text-slate-400">
                        <div class="text-center">
                            <div class="mx-auto h-7 w-7 animate-spin rounded-full border-2 border-white/10 border-t-[#c6a15b]" />
                            <p class="mt-3">{"Loading trace…"}</p>
                            if !recorder.instance_id.is_empty() {
                                <p class="mt-1 font-mono text-[9px] text-slate-600">{ recorder.instance_id.clone() }</p>
                            }
                        </div>
                    </div>
                }
            </div>
        }
    }
}
