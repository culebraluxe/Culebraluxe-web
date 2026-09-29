//! TECH / Engineering Cockpit — the Forge assembly line.
//!
//! Pass 1 preserves the screen's proven operating model: raw story supply, a collapsible daily Workbench, Flight staging,
//! the engine queue/running/results lanes, selected-story introspection and the last four story outcomes. SVAR remains a
//! rendering-only React island for drag mechanics; Yew owns the screen, selection and canonical data refresh.

use yew::prelude::*;

use crate::model::{
    PortalTechEngineRun, PortalTechFlight, PortalTechHistory, PortalTechPage, PortalTechRun,
    PortalTechStory,
};

use super::{Msg, Vm};
mod engine;
mod workbench;
#[allow(unused_imports)]
pub(super) use engine::*;
#[allow(unused_imports)]
pub(super) use workbench::*;

pub(super) fn cockpit(model: &Vm<'_>, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    if !tech.ready {
        return html! {
            <section class="rounded-xl bg-[#0b1220] p-8 text-center text-slate-200">
                <h1 class="font-serif text-2xl font-semibold text-white">{"Engineering Cockpit"}</h1>
                <p class="mt-2 text-sm text-slate-400">{"The Story Board tables are not ready."}</p>
            </section>
        };
    }

    html! {
        <div class="min-h-screen rounded-xl bg-[#0b1220] px-5 py-5 text-slate-200">
            { header(tech, model, on_msg) }
            { kpis(tech) }
            { command_notice(model) }
            { sorter(model, tech, on_msg) }
            { flight_strip(model, tech, on_msg) }
            { workbench(model, tech, on_msg) }
            { engine_line(tech) }
            { recent_history(tech) }
        </div>
    }
}

fn header(tech: &PortalTechPage, model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let refresh = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechRefreshRequested))
    };
    html! {
        <header class="mb-4 flex flex-wrap items-end justify-between gap-3">
            <div>
                <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-[#c6a15b]">{"TECH / ENGINEERING"}</p>
                <h1 class="mt-1 font-serif text-2xl font-semibold text-white">{"Forge Cockpit"}</h1>
                <p class="mt-1 max-w-3xl text-sm font-light text-slate-400">
                    {"One assembly line: raw stories, today's Workbench, the next Flight, what Forge is running, and what just came out."}
                </p>
                <p class="mt-1 text-[10px] text-slate-500">{ format!("Story data as of {}", short_time(&tech.freshness)) }</p>
            </div>
            <div class="flex items-center gap-2">
                if model.loading {
                    <span class="text-[10px] uppercase tracking-[0.12em] text-[#c6a15b]">{"refreshing…"}</span>
                }
                <button
                    type="button"
                    onclick={refresh}
                    class="rounded-md border border-white/15 bg-white/[0.04] px-3 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-slate-300 hover:border-[#c6a15b]/50 hover:text-[#e0c489]"
                >
                    {"Refresh"}
                </button>
            </div>
        </header>
    }
}

fn kpis(tech: &PortalTechPage) -> Html {
    let queued = tech.queued_cards.len();
    let running = running_runs(tech).len();
    let failed = tech
        .engine_runs
        .iter()
        .filter(|run| run.status == "failed")
        .count();
    let values = [
        (
            "Stories",
            tech.total_stories.to_string(),
            "canonical raw material".to_string(),
        ),
        (
            "Workbench",
            tech.active_work.len().to_string(),
            "today's inspection tray".to_string(),
        ),
        (
            "Flight",
            tech.staging_flight
                .as_ref()
                .map(|flight| flight.story_count)
                .unwrap_or(0)
                .to_string(),
            "staged, not dispatched".to_string(),
        ),
        ("Queued", queued.to_string(), "handed to Forge".to_string()),
        (
            "Running",
            running.to_string(),
            "machine owns it".to_string(),
        ),
        (
            "Failed",
            failed.to_string(),
            format!("{:.1}% board complete", tech.completion_percent),
        ),
    ];
    html! {
        <section class="mb-4 grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-6">
            { for values.into_iter().map(|(label, value, hint)| html! {
                <article class="rounded-lg border border-white/10 bg-white/[0.035] px-3 py-2.5">
                    <p class="text-[9px] font-semibold uppercase tracking-[0.14em] text-slate-500">{ label }</p>
                    <p class="mt-1 font-serif text-2xl font-light text-white">{ value }</p>
                    <p class="mt-1 truncate text-[9px] font-light text-slate-500">{ hint }</p>
                </article>
            }) }
        </section>
    }
}

/// The Kanban: every story in one column; drag a card to another column to move its story (the tech service's
/// moveStoryBucket). Click a card to open its story in the Cockpit.
fn sorter(model: &Vm<'_>, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let busy = model.tech.busy_action.is_some();
    html! {
        <section class="mb-4 overflow-hidden rounded-lg border border-white/10 bg-white/[0.02]">
            <div class="flex flex-wrap items-baseline justify-between gap-2 border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-[#c6a15b]">{"STORY SUPPLY → FLIGHT → ENGINE"}</p>
                    <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"Story sorter"}</h2>
                </div>
                <p class="text-[10px] text-slate-400">
                    {"Drag freely · WORK BENCH is daily intent · FLIGHT STAGING does not dispatch · ENGINE RUN Q does"}
                </p>
            </div>
            <div class="grid grid-cols-2 gap-2 p-2 md:grid-cols-3 xl:grid-cols-6">
                {for tech.sorter_columns.iter().map(|column| {
                    let cards: Vec<_> = tech.sorter_cards.iter().filter(|card| card.column == column.id).collect();
                    let target = column.id.clone();
                    let drop = on_msg.reform(move |event: DragEvent| { event.prevent_default(); Msg::SorterDropped(target.clone()) });
                    html! {
                        <div ondragover={Callback::from(|event: DragEvent| event.prevent_default())} ondrop={drop}
                            class="flex min-h-[8rem] flex-col rounded-md border border-white/10 bg-black/20">
                            <div class="flex items-center justify-between border-b border-white/10 px-2 py-1.5">
                                <span class="text-[9px] font-semibold uppercase tracking-[0.14em] text-slate-300">{&column.label}</span>
                                <span class="text-[9px] text-slate-500">{cards.len()}</span>
                            </div>
                            <div class="max-h-[420px] min-h-0 flex-1 space-y-1 overflow-y-auto p-1.5">
                                {for cards.into_iter().map(|card| {
                                    let id = card.id.clone();
                                    let story = card.id.split('#').next().unwrap_or(&card.id).to_owned();
                                    let start = on_msg.reform(move |_: DragEvent| Msg::SorterDragStarted(id.clone()));
                                    let open = on_msg.reform(move |_: MouseEvent| Msg::TechStorySelected(story.clone()));
                                    html! {
                                        <div draggable={(!busy).to_string()} ondragstart={start} onclick={open}
                                            class="cursor-grab rounded border border-white/10 bg-white/[0.04] px-2 py-1.5 hover:border-[#c6a15b]/50">
                                            <div class="flex items-center justify-between gap-1">
                                                <span class="truncate font-mono text-[9px] text-[#e0c489]">{card.id.split('#').next().unwrap_or(&card.id)}</span>
                                                <span class="shrink-0 text-[8px] uppercase text-slate-500">{&card.priority}</span>
                                            </div>
                                            <div class="mt-0.5 line-clamp-2 text-[11px] leading-snug text-slate-200">{&card.title}</div>
                                        </div>
                                    }
                                })}
                            </div>
                        </div>
                    }
                })}
            </div>
        </section>
    }
}

fn flight_strip(model: &Vm<'_>, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let busy = model.tech.busy_action.is_some();
    let flight_count = tech
        .staging_flight
        .as_ref()
        .map(|flight| flight.story_count)
        .unwrap_or(0);
    let launch = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechLaunchFlightRequested))
    };
    let schedule_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::TechScheduleChanged(value));
        })
    };
    let schedule_local = model.tech.schedule_at.clone();
    let schedule = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(scheduled_for) = local_datetime_to_iso(&schedule_local) {
                on_msg.emit(Msg::TechScheduleFlightRequested { scheduled_for });
            }
        })
    };

    html! {
        <section class="mb-4 grid gap-3 lg:grid-cols-[1.15fr_1.85fr]">
            <article class="rounded-lg border border-[#c6a15b]/25 bg-[#c6a15b]/[0.06] p-4">
                <div class="flex items-start justify-between gap-3">
                    <div>
                        <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-[#c6a15b]">{"NEXT FLIGHT"}</p>
                        if let Some(flight) = tech.staging_flight.as_ref() {
                            <p class="mt-1 font-serif text-2xl font-light text-white">{ format!("{} stories", flight.story_count) }</p>
                            <p class="mt-1 text-[10px] text-slate-400">
                                { format!("policy {} · staged only", flight.model_policy) }
                            </p>
                        } else {
                            <p class="mt-2 text-sm font-light text-slate-400">{"No Flight is staged yet. Drag stories into FLIGHT STAGING."}</p>
                        }
                    </div>
                    if let Some(flight) = tech.staging_flight.as_ref() {
                        <span class="rounded-full border border-[#c6a15b]/30 px-2 py-1 text-[9px] uppercase tracking-[0.12em] text-[#e0c489]">
                            { flight.status.clone() }
                        </span>
                    }
                </div>
                <div class="mt-4 flex flex-wrap items-center gap-2">
                    <button
                        type="button"
                        onclick={launch}
                        disabled={busy || flight_count == 0}
                        title="Dispatch every persisted story in this Flight to Forge now."
                        class="rounded border border-[#c6a15b]/50 bg-[#c6a15b]/15 px-2.5 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[#e0c489] transition hover:bg-[#c6a15b]/25 disabled:cursor-not-allowed disabled:opacity-35"
                    >
                        { if model.tech.busy_action.as_deref() == Some("launchFlight") { "Launching…" } else { "Launch Flight →" } }
                    </button>
                    <input
                        type="datetime-local"
                        value={model.tech.schedule_at.clone()}
                        onchange={schedule_change}
                        disabled={busy || flight_count == 0}
                        aria-label="Schedule Flight"
                        class="rounded border border-white/15 bg-[#0b1220] px-2 py-1.5 text-[10px] text-slate-300 disabled:opacity-35"
                    />
                    <button
                        type="button"
                        onclick={schedule}
                        disabled={busy || flight_count == 0 || model.tech.schedule_at.trim().is_empty()}
                        title="Persist this same Flight for the chosen browser-local time. Nothing dispatches now."
                        class="rounded border border-white/20 px-2.5 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-slate-300 transition hover:border-[#c6a15b]/50 hover:text-[#e0c489] disabled:cursor-not-allowed disabled:opacity-35"
                    >
                        { if model.tech.busy_action.as_deref() == Some("scheduleFlight") { "Scheduling…" } else { "Schedule" } }
                    </button>
                </div>
                <p class="mt-2 text-[9px] leading-4 text-slate-500">
                    {"Launch dispatches now. Schedule writes only a time on this Flight; the unattended worker fires it later."}
                </p>
            </article>
            <article class="rounded-lg border border-white/10 bg-white/[0.025] p-4">
                <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-slate-500">{"RECENT FLIGHTS"}</p>
                <div class="mt-2 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                    { for tech.recent_flights.iter().map(|flight| flight_card(flight, busy, on_msg)) }
                </div>
            </article>
        </section>
    }
}

fn flight_card(flight: &PortalTechFlight, busy: bool, on_msg: &Callback<Msg>) -> Html {
    let batch_id = flight.id.clone();
    let cancel = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::TechCancelFlightRequested(batch_id.clone()))
        })
    };
    html! {
        <div class="rounded-md border border-white/10 bg-white/[0.03] p-2.5">
            <div class="flex items-center justify-between gap-2">
                <span class="truncate font-mono text-[9px] text-slate-400">{ compact_id(&flight.id) }</span>
                <span class="text-[9px] uppercase tracking-[0.08em] text-[#c6a15b]">{ flight.status.clone() }</span>
            </div>
            <p class="mt-1 text-sm text-white">{ format!("{} stories", flight.story_count) }</p>
            <p class="mt-1 text-[9px] text-slate-500">
                { format!("{} queued · {} skipped", flight.queued_count, flight.skipped_count) }
            </p>
            if flight.status == "Scheduled" {
                <div class="mt-2 flex items-center justify-between gap-2">
                    <span class="truncate text-[9px] text-[#e0c489]">
                        { flight.scheduled_for.as_deref().map(short_time).unwrap_or_else(|| "time not recorded".into()) }
                    </span>
                    <button
                        type="button"
                        onclick={cancel}
                        disabled={busy}
                        class="text-[9px] uppercase tracking-[0.1em] text-slate-400 underline decoration-dotted underline-offset-2 hover:text-white disabled:opacity-35"
                    >
                        {"Cancel"}
                    </button>
                </div>
            }
        </div>
    }
}

fn command_notice(model: &Vm<'_>) -> Html {
    let Some(notice) = model.tech.notice.as_ref() else {
        return html! {};
    };
    html! {
        <div class={classes!(
            "mb-4","rounded-md","border","px-3","py-2","text-[10px]",
            if notice.ok {
                "border-emerald-400/25 bg-emerald-400/[0.05] text-emerald-200"
            } else {
                "border-rose-400/30 bg-rose-400/[0.06] text-rose-200"
            }
        )}>
            { notice.message.clone() }
        </div>
    }
}

fn local_datetime_to_iso(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(raw));
    if date.get_time().is_nan() {
        return None;
    }
    let value: wasm_bindgen::JsValue = date.to_iso_string().into();
    value.as_string()
}

fn selected_engine_owned(tech: &PortalTechPage, story_id: &str) -> bool {
    tech.queued_cards
        .iter()
        .any(|item| item.story_id == story_id)
        || tech.engine_runs.iter().any(|run| {
            run.story_id == story_id
                && !run.stale
                && matches!(run.status.as_str(), "running" | "claimed" | "queued")
        })
}

fn selected_engine_live(tech: &PortalTechPage, story_id: &str) -> bool {
    tech.engine_runs.iter().any(|run| {
        run.story_id == story_id
            && !run.stale
            && matches!(run.status.as_str(), "running" | "claimed")
    })
}

fn running_runs(tech: &PortalTechPage) -> Vec<&PortalTechEngineRun> {
    tech.engine_runs
        .iter()
        .filter(|run| !run.stale && matches!(run.status.as_str(), "running" | "claimed" | "queued"))
        .collect()
}

fn result_runs(tech: &PortalTechPage) -> Vec<&PortalTechEngineRun> {
    tech.engine_runs
        .iter()
        .filter(|run| {
            run.stale || matches!(run.status.as_str(), "completed" | "failed" | "interrupted")
        })
        .collect()
}

fn compact_id(value: &str) -> String {
    if value.len() <= 18 {
        value.to_string()
    } else {
        format!("{}…", &value[..17])
    }
}

fn short_time(value: &str) -> String {
    value
        .replace('T', " ")
        .trim_end_matches('Z')
        .chars()
        .take(16)
        .collect()
}
