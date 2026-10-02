//! TECH Cockpit — one shell, three operating views.
//!
//! Assembly Line owns story preparation and dispatch controls.
//! Live Ops is the real-time Forge instrument panel (fake MVI until V2 runtime wiring lands).
//! Flight Recorder mounts the existing forensic console for the selected story's latest run.

use yew::prelude::*;

use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::screens::flight_recorder::{FlightRecorder, Msg as FlightMsg};
use crate::app::template;

use super::{live_ops, Model, Msg, TechTab, Vm};

mod assembly;
mod engine;
mod workbench;

pub(super) fn cockpit(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
    html! {
        <div class="min-h-screen rounded-xl bg-[#07101d] text-slate-200">
            { tab_bar(model, link) }

            <div class="px-4 pb-4 sm:px-5 sm:pb-5">
                {
                    match model.tab {
                        TechTab::AssemblyLine => assembly_tab(model, link),
                        TechTab::LiveOps => live_tab(model, link),
                        TechTab::FlightRecorder => flight_tab(model, ctx, link),
                    }
                }
            </div>
        </div>
    }
}

fn tab_bar(model: &Model, link: &Link<Msg>) -> Html {
    html! {
        <div class="sticky top-0 z-40 mb-1 border-b border-white/10 bg-[#07101d]/95 px-4 pt-4 backdrop-blur sm:px-5 sm:pt-5">
            <div class="flex flex-wrap items-end justify-between gap-3">
                <div>
                    <p class="text-[10px] font-semibold uppercase tracking-[0.22em] text-[#c6a15b]">
                        {"TECH / FORGE"}
                    </p>
                    <h1 class="mt-0.5 font-serif text-2xl font-semibold text-white">{"Engineering Cockpit"}</h1>
                    <p class="mt-1 text-xs font-light text-slate-500">
                        {"Prepare the line · watch the engine · inspect the black box"}
                    </p>
                </div>
                <div class="flex rounded-lg border border-white/10 bg-black/20 p-1" role="tablist" aria-label="Cockpit views">
                    { for [TechTab::AssemblyLine, TechTab::LiveOps, TechTab::FlightRecorder].into_iter().map(|tab| {
                        let selected = model.tab == tab;
                        let onclick = link.callback(move |_: MouseEvent| Msg::TabSelected(tab));
                        html! {
                            <button
                                type="button"
                                role="tab"
                                aria-selected={selected.to_string()}
                                {onclick}
                                class={classes!(
                                    "rounded-md","px-3","py-2","text-[10px]","font-semibold","uppercase","tracking-[0.11em]","transition",
                                    if selected {
                                        "bg-[#c6a15b]/15 text-[#e0c489] shadow-sm"
                                    } else {
                                        "text-slate-500 hover:bg-white/[0.04] hover:text-slate-200"
                                    }
                                )}
                            >
                                { tab.label() }
                            </button>
                        }
                    }) }
                </div>
            </div>
            <div class="mt-4 h-px bg-gradient-to-r from-[#c6a15b]/40 via-white/10 to-transparent" />
        </div>
    }
}

fn assembly_tab(model: &Model, link: &Link<Msg>) -> Html {
    let on_msg = link.callback(|msg: Msg| msg);
    template::remote(&model.read, "the Forge assembly line", |tech| {
        assembly::assembly(
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

fn live_tab(model: &Model, link: &Link<Msg>) -> Html {
    let on_live = link.callback(|msg: live_ops::Msg| Msg::Live(msg));
    live_ops::view(&model.live, &on_live)
}

fn flight_tab(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
    let Some(instance_id) = model.flight_instance_id.as_deref() else {
        return html! {
            <section class="grid min-h-[34rem] place-items-center rounded-xl border border-white/10 bg-[#0b1220] p-8 text-center">
                <div class="max-w-lg">
                    <div class="mx-auto grid h-12 w-12 place-items-center rounded-xl border border-[#c6a15b]/25 bg-[#c6a15b]/10 font-serif text-lg text-[#e0c489]">
                        {"FR"}
                    </div>
                    <h2 class="mt-4 font-serif text-xl font-semibold text-white">{"Select a recorded story first"}</h2>
                    <p class="mt-2 text-sm leading-6 text-slate-400">
                        {"The Flight Recorder tab opens the latest process instance for the story selected in Assembly Line. Choose a story with execution history, then return here."}
                    </p>
                    <button
                        type="button"
                        onclick={link.callback(|_: MouseEvent| Msg::TabSelected(TechTab::AssemblyLine))}
                        class="mt-5 rounded-md border border-[#c6a15b]/35 bg-[#c6a15b]/10 px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#e0c489] hover:bg-[#c6a15b]/15"
                    >
                        {"Open Assembly Line"}
                    </button>
                </div>
            </section>
        };
    };

    let Some(flight) = model.flight.as_ref() else {
        return html! {
            <section class="grid min-h-[34rem] place-items-center rounded-xl border border-white/10 bg-[#0b1220] text-sm text-slate-400">
                {"Opening Flight Recorder…"}
            </section>
        };
    };

    let mut child_ctx = ctx.clone();
    child_ctx.id = Some(instance_id.to_string());
    let child_link = Link::new(link.callback(|msg: FlightMsg| Msg::Flight(msg)));
    FlightRecorder::view(flight, &child_ctx, &child_link)
}
