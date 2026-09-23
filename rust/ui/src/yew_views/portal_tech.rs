//! TECH / Engineering Cockpit — the Forge assembly line.
//!
//! Pass 1 preserves the screen's proven operating model: raw story supply, a collapsible daily Workbench, Flight staging,
//! the engine queue/running/results lanes, selected-story introspection and the last four story outcomes. SVAR remains a
//! rendering-only React island for drag mechanics; Yew owns the screen, selection and canonical data refresh.

use serde_json::json;
use yew::prelude::*;

use crate::model::{
    Msg, PortalTechEngineRun, PortalTechFlight, PortalTechHistory, PortalTechPage, PortalTechRun,
    PortalTechStory,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct TechCockpitProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct TechCockpit;

impl Component for TechCockpit {
    type Message = ();
    type Properties = TechCockpitProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("tech").expect("tech screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { cockpit(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalTechPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.tech.as_ref())
}

fn cockpit(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(tech) = payload(model) else {
        return html! {
            <section class="min-h-[32rem] rounded-xl bg-[#0b1220] p-6 text-slate-200">
                <p class="text-sm text-slate-400">
                    { if model.loading { "Loading the Forge line…" } else { "The Engineering Cockpit is not available." } }
                </p>
            </section>
        };
    };

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
            { island_bridge(on_msg) }
            { header(tech, model, on_msg) }
            { kpis(tech) }
            { command_notice(model) }
            { sorter(tech) }
            { flight_strip(model, tech, on_msg) }
            { workbench(model, tech, on_msg) }
            { engine_line(tech) }
            { recent_history(tech) }
        </div>
    }
}

fn island_bridge(on_msg: &Callback<Msg>) -> Html {
    let dispatch = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: MouseEvent| {
            let target = event.target_unchecked_into::<web_sys::HtmlElement>();
            let Some(raw) = target.get_attribute("data-intent") else {
                return;
            };
            let Ok(intent) = serde_json::from_str::<serde_json::Value>(&raw) else {
                return;
            };
            if intent.get("kind").and_then(|value| value.as_str()) == Some("refresh") {
                on_msg.emit(Msg::TechRefreshRequested);
            }
        })
    };
    html! {
        <button
            id="tech-cockpit-island-bridge"
            type="button"
            class="hidden"
            data-intent=""
            tabindex="-1"
            aria-hidden="true"
            onclick={dispatch}
        />
    }
}

fn header(tech: &PortalTechPage, model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
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
        ("Stories", tech.total_stories.to_string(), "canonical raw material".to_string()),
        ("Workbench", tech.active_work.len().to_string(), "today's inspection tray".to_string()),
        ("Flight", tech.staging_flight.as_ref().map(|flight| flight.story_count).unwrap_or(0).to_string(), "staged, not dispatched".to_string()),
        ("Queued", queued.to_string(), "handed to Forge".to_string()),
        ("Running", running.to_string(), "machine owns it".to_string()),
        ("Failed", failed.to_string(), format!("{:.1}% board complete", tech.completion_percent)),
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

fn sorter(tech: &PortalTechPage) -> Html {
    let widget = json!({
        "cards": &tech.sorter_cards,
        "columns": &tech.sorter_columns,
    });
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
            <div
                id="tech-sorter-island"
                data-tech-widget={widget.to_string()}
                class="h-[520px] overflow-hidden p-2"
                aria-label="Story sorter"
            />
        </section>
    }
}

fn flight_strip(model: &crate::model::Model, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let busy = model.tech.busy_action.is_some();
    let flight_count = tech.staging_flight.as_ref().map(|flight| flight.story_count).unwrap_or(0);
    let launch = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechLaunchFlightRequested))
    };
    let schedule_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
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
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechCancelFlightRequested(batch_id.clone())))
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

fn workbench(model: &crate::model::Model, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let open = !model.controls.toggled;
    let busy = model.tech.busy_action.is_some();
    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::Toggled(open)))
    };
    let clear_count = tech.active_work.len();
    let clear = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            let prompt = format!(
                "Take all {} stories off the Workbench?\n\nThis clears today's list only. Story status and run history stay exactly as they are.",
                clear_count
            );
            let confirmed = web_sys::window()
                .and_then(|window| window.confirm_with_message(&prompt).ok())
                .unwrap_or(false);
            if confirmed {
                on_msg.emit(Msg::TechClearWorkbenchRequested);
            }
        })
    };
    html! {
        <section class="mb-4 overflow-hidden rounded-lg border border-white/10 bg-white/[0.02]">
            <div class="flex flex-wrap items-center justify-between gap-2 border-b border-white/10 px-4 py-3">
                <button type="button" onclick={toggle} class="flex items-baseline gap-2 text-left">
                    <span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-white">
                        {"WORKBENCH "}
                        <span class="font-normal text-[#c6a15b]">{ format!("({})", tech.active_work.len()) }</span>
                    </span>
                    <span class="text-[10px] font-normal text-slate-400">{"today's inspection tray"}</span>
                    <span class="text-[10px] text-slate-500">{ if open { "▲" } else { "▼" } }</span>
                </button>
                <div class="flex flex-wrap items-center gap-3">
                    <p class="text-[10px] text-slate-500">
                        {"Orthogonal to status · scoped investigation returns here for review"}
                    </p>
                    if !tech.active_work.is_empty() {
                        <button
                            type="button"
                            onclick={clear}
                            disabled={busy}
                            class="rounded border border-white/15 px-2 py-1 text-[9px] uppercase tracking-[0.1em] text-slate-400 transition hover:border-[#c6a15b]/40 hover:text-[#e0c489] disabled:opacity-35"
                        >
                            { if model.tech.busy_action.as_deref() == Some("clearWorkbench") { "Clearing…" } else { "Clear Workbench" } }
                        </button>
                    }
                </div>
            </div>
            if open {
                <div class="grid gap-3 p-3 lg:grid-cols-[minmax(0,0.82fr)_minmax(0,1.18fr)]">
                    { workbench_queue(tech, on_msg) }
                    <div class="space-y-3">
                        { selected_story(model, tech, on_msg) }
                        { selected_runs(tech) }
                    </div>
                </div>
            }
        </section>
    }
}

fn workbench_queue(tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    html! {
        <article class="overflow-hidden rounded-md border border-white/10 bg-white/[0.025]">
            <div class="flex items-center justify-between border-b border-white/10 px-3 py-2.5">
                <div>
                    <p class="text-[9px] uppercase tracking-[0.14em] text-slate-500">{"Selected today"}</p>
                    <h3 class="font-serif text-base font-semibold text-white">{"Active Work Queue"}</h3>
                </div>
                <span class="rounded-full border border-white/10 px-2 py-0.5 text-xs text-[#e0c489]">{ tech.active_work.len() }</span>
            </div>
            <div class="max-h-[28rem] space-y-1 overflow-y-auto p-2">
                if tech.active_work.is_empty() {
                    <p class="px-3 py-8 text-center text-xs italic text-slate-500">{"Nothing on today's Workbench."}</p>
                } else {
                    { for tech.active_work.iter().map(|story| workbench_row(story, tech, on_msg)) }
                }
            </div>
        </article>
    }
}

fn workbench_row(story: &PortalTechStory, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let selected = tech.selected_story.as_ref().is_some_and(|candidate| candidate.id == story.id);
    let id = story.id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechStorySelected(id.clone())))
    };
    html! {
        <button
            type="button"
            {onclick}
            class={classes!(
                "grid","w-full","grid-cols-[8rem_minmax(0,1fr)_3rem]","items-center","gap-2","rounded-md","border","px-2.5","py-2","text-left","transition",
                if selected { "border-[#c6a15b]/60 bg-[#c6a15b]/10" } else { "border-white/10 bg-white/[0.025] hover:border-[#c6a15b]/30" }
            )}
        >
            <span class="truncate font-mono text-[10px] text-slate-300">{ story.id.clone() }</span>
            <span class="truncate text-[11px] font-light text-white/75">{ story.title.clone() }</span>
            <span class="text-right text-[9px] tabular-nums text-slate-500">{ format!("{}%", story.completion.round()) }</span>
        </button>
    }
}

fn selected_story(model: &crate::model::Model, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
    let Some(story) = tech.selected_story.as_ref() else {
        return html! {
            <article class="rounded-md border border-white/10 bg-white/[0.025] p-5 text-sm text-slate-500">
                {"Select a Workbench story to inspect it."}
            </article>
        };
    };
    let on_bench = tech.active_work.iter().any(|candidate| candidate.id == story.id);
    let engine_owned = selected_engine_owned(tech, &story.id);
    let engine_live = selected_engine_live(tech, &story.id);
    let busy = model.tech.busy_action.is_some();

    let scoped = |target: &'static str| {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechScopedRunRequested(target.to_string())))
    };
    let move_to = |target: &'static str| {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechMoveWorkbenchRequested(target.to_string())))
    };
    let good_to_go = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::TechGoodToGoRequested))
    };

    let specs = [
        ("Goal", story.goal.as_deref()),
        ("Architecture Brief", story.architect_brief.as_deref()),
        ("Scope", story.scope.as_deref()),
        ("Preconditions", story.preconditions.as_deref()),
        ("Acceptance Criteria", story.acceptance_criteria.as_deref()),
        ("Postconditions", story.postconditions.as_deref()),
        ("Context / References", story.context_refs.as_deref()),
        ("Notes", story.notes.as_deref()),
    ];
    html! {
        <article class="overflow-hidden rounded-md border border-white/10 bg-white/[0.025]">
            <div class="border-b border-white/10 px-4 py-3">
                <div class="flex flex-wrap items-start justify-between gap-2">
                    <div class="min-w-0">
                        <p class="font-mono text-[9px] uppercase tracking-[0.14em] text-slate-500">{"Current Story / Architecture"}</p>
                        <h3 class="mt-1 font-serif text-lg font-semibold text-white">
                            { format!("{} — {}", story.id, story.title) }
                        </h3>
                    </div>
                    if let Some(instance) = tech.recorder_instance_id.as_deref().filter(|value| !value.is_empty()) {
                        <a
                            href={format!("/portal/tech/flight-recorder/{instance}")}
                            class="rounded-md border border-[#c6a15b]/40 px-2.5 py-1.5 text-[9px] font-medium uppercase tracking-[0.11em] text-[#e0c489] hover:bg-[#c6a15b]/10"
                        >
                            {"Flight Recorder →"}
                        </a>
                    }
                </div>
                <div class="mt-2 flex flex-wrap gap-2 text-[9px] uppercase tracking-[0.1em] text-slate-400">
                    <span class="rounded-full border border-white/10 px-2 py-0.5">{ story.status.clone() }</span>
                    <span>{ story.priority.clone() }</span>
                    <span>{ format!("{:.0}% complete", story.completion) }</span>
                    <span>{ story.workstream.clone() }</span>
                </div>

                <div class="mt-3 rounded-md border border-[#c6a15b]/25 bg-[#c6a15b]/[0.045] p-2.5">
                    <div class="flex flex-wrap items-center justify-between gap-2">
                        <div>
                            <p class="text-[9px] font-semibold uppercase tracking-[0.14em] text-[#e0c489]">{"INVESTIGATE"}</p>
                            <p class="mt-0.5 text-[9px] text-slate-500">{"Run only far enough to answer the question; keep the story on the Workbench."}</p>
                        </div>
                        <div class="flex flex-wrap gap-1.5">
                            { for [("scout", "Scout"), ("architect", "Architect"), ("lead", "Lead")].into_iter().map(|(key, label)| {
                                let action = scoped(key);
                                let active = model.tech.busy_action.as_deref() == Some(&format!("scoped:{key}"));
                                html! {
                                    <button
                                        type="button"
                                        onclick={action}
                                        disabled={busy || !on_bench || engine_live}
                                        class="rounded border border-white/15 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-slate-300 transition hover:border-[#c6a15b]/40 hover:text-[#e0c489] disabled:cursor-not-allowed disabled:opacity-35"
                                    >
                                        { if active { "Queueing…" } else { label } }
                                    </button>
                                }
                            }) }
                        </div>
                    </div>
                    <div class="mt-2 flex flex-wrap items-center justify-between gap-2 border-t border-white/10 pt-2">
                        <div class="flex flex-wrap items-center gap-1.5">
                            <span class="mr-1 text-[9px] font-semibold uppercase tracking-[0.12em] text-slate-500">{"MOVE TO"}</span>
                            <button type="button" onclick={move_to("backlog")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-[#c6a15b]/30 hover:text-white disabled:opacity-35">{"Backlog"}</button>
                            <button type="button" onclick={move_to("closed")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-[#c6a15b]/30 hover:text-white disabled:opacity-35">{"Closed"}</button>
                            <button type="button" onclick={move_to("next")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-[#c6a15b]/30 hover:text-white disabled:opacity-35">{"Next Version"}</button>
                        </div>
                        <button
                            type="button"
                            onclick={good_to_go}
                            disabled={busy || !on_bench || engine_owned}
                            title="Remove from the Workbench and hand the full story to Forge. Ready queues real work."
                            class="rounded border border-[#c6a15b]/50 bg-[#c6a15b]/15 px-3 py-1 text-[9px] font-semibold uppercase tracking-[0.12em] text-[#e0c489] transition hover:bg-[#c6a15b]/25 disabled:cursor-not-allowed disabled:opacity-35"
                        >
                            { if engine_owned { "Already with Forge" } else if model.tech.busy_action.as_deref() == Some("goodToGo") { "Queueing…" } else { "Good to Go →" } }
                        </button>
                    </div>
                    if !on_bench {
                        <p class="mt-2 text-[9px] text-slate-600">{"Inspection only — add this story to the Workbench before using operator controls."}</p>
                    } else if engine_live {
                        <p class="mt-2 text-[9px] text-amber-300/70">{"Forge is already executing this story. Scoped controls cannot rewrite a live run."}</p>
                    }
                </div>

                if let Some(hold) = tech.hold.as_ref() {
                    <div class="mt-3 rounded-md border border-amber-400/30 bg-amber-400/[0.07] px-3 py-2">
                        <p class="text-[9px] font-semibold uppercase tracking-[0.12em] text-amber-300">{"Forge HOLD"}</p>
                        <p class="mt-1 text-xs text-amber-100/80">{ hold.reason.clone().unwrap_or_else(|| "Engine parked this story.".into()) }</p>
                    </div>
                }
            </div>
            <div class="max-h-80 space-y-2 overflow-y-auto p-4">
                { for specs.into_iter().map(|(label, value)| html! {
                    <details open={matches!(label, "Goal" | "Architecture Brief" | "Acceptance Criteria")}>
                        <summary class="cursor-pointer list-none text-[9px] font-semibold uppercase tracking-[0.14em] text-[#c6a15b]/80">{ label }</summary>
                        <p class="mt-1 whitespace-pre-wrap text-[12px] font-light leading-5 text-white/70">
                            { value.unwrap_or("Not specified.") }
                        </p>
                    </details>
                }) }
            </div>
        </article>
    }
}

fn selected_runs(tech: &PortalTechPage) -> Html {
    html! {
        <article class="overflow-hidden rounded-md border border-white/10 bg-white/[0.025]">
            <div class="flex items-center justify-between border-b border-white/10 px-4 py-2.5">
                <div>
                    <p class="text-[9px] uppercase tracking-[0.14em] text-slate-500">{"Child execution / evidence"}</p>
                    <h3 class="font-serif text-base font-semibold text-white">{"Engineering / Run History"}</h3>
                </div>
                <span class="rounded-full border border-white/10 px-2 py-0.5 text-xs text-slate-300">{ tech.selected_runs.len() }</span>
            </div>
            if tech.selected_runs.is_empty() {
                <p class="px-4 py-6 text-center text-xs italic text-slate-500">{"No execution history recorded for this story."}</p>
            } else {
                <div class="max-h-52 divide-y divide-white/5 overflow-y-auto">
                    { for tech.selected_runs.iter().map(run_row) }
                </div>
            }
        </article>
    }
}

fn run_row(run: &PortalTechRun) -> Html {
    html! {
        <div class="grid grid-cols-[6.5rem_minmax(0,1fr)_5rem] gap-3 px-4 py-2.5 text-[10px]">
            <div>
                <p class="text-slate-300">{ run.result_status.clone().unwrap_or_else(|| "Running".into()) }</p>
                <p class="mt-0.5 text-[9px] text-slate-600">{ short_time(&run.started_at) }</p>
            </div>
            <div class="min-w-0">
                <p class="truncate text-slate-400">
                    { run.run_phase.clone().or(run.run_type.clone()).or(run.agent_runtime.clone()).unwrap_or_else(|| "Forge run".into()) }
                </p>
                if let Some(note) = run.notes.as_deref() {
                    <p class="mt-0.5 truncate text-[9px] text-slate-600">{ note }</p>
                }
            </div>
            <div class="text-right text-slate-500">
                { run.completion.map(|value| format!("{value:.0}%")).unwrap_or_else(|| "—".into()) }
            </div>
        </div>
    }
}

fn engine_line(tech: &PortalTechPage) -> Html {
    let running = running_runs(tech);
    let results = result_runs(tech);
    html! {
        <section class="mb-4 overflow-hidden rounded-lg border border-white/10 bg-white/[0.02]">
            <div class="border-b border-white/10 px-4 py-3">
                <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-[#c6a15b]">{"FORGE ENGINE"}</p>
                <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"What is happening now"}</h2>
            </div>
            <div class="grid divide-y divide-white/10 lg:grid-cols-3 lg:divide-x lg:divide-y-0">
                { engine_queue(tech) }
                { engine_runs_panel("RUNNING", &running, false) }
                { engine_runs_panel("RESULTS", &results, true) }
            </div>
            if let Some(ledger) = tech.ledger.as_ref() {
                <div class="flex flex-wrap gap-x-5 gap-y-1 border-t border-white/10 px-4 py-2 text-[9px] text-slate-500">
                    <span>{ format!("{} attempts", ledger.total_attempts) }</span>
                    <span>{ format!("{} stories touched", ledger.stories) }</span>
                    <span>{ format!("{} complete", ledger.completed) }</span>
                    <span>{ format!("{} failed", ledger.failed) }</span>
                    <span>{ format!("{} interrupted", ledger.interrupted) }</span>
                </div>
            }
        </section>
    }
}

fn engine_queue(tech: &PortalTechPage) -> Html {
    html! {
        <article class="min-h-44 p-3">
            <div class="mb-2 flex items-center justify-between">
                <h3 class="text-[10px] font-semibold uppercase tracking-[0.13em] text-sky-300">{"ENGINE QUEUED"}</h3>
                <span class="text-[10px] text-slate-500">{ tech.queued_cards.len() }</span>
            </div>
            if !tech.queue_read_ok {
                <p class="text-[10px] text-amber-300/80">{"Queue read failed; nothing is guessed."}</p>
            } else if tech.queued_cards.is_empty() {
                <p class="py-5 text-center text-[10px] italic text-slate-600">{"Nothing waiting."}</p>
            } else {
                <div class="space-y-1.5">
                    { for tech.queued_cards.iter().take(8).map(|card| html! {
                        <div class="rounded-md border border-sky-400/15 bg-sky-400/[0.04] px-2.5 py-2">
                            <p class="truncate text-[10px] text-slate-200">{ card.title.clone() }</p>
                            <p class="mt-1 font-mono text-[8px] text-slate-600">{ format!("{} · {}", compact_id(&card.story_id), card.state) }</p>
                        </div>
                    }) }
                </div>
            }
        </article>
    }
}

fn engine_runs_panel(title: &'static str, runs: &[&PortalTechEngineRun], results: bool) -> Html {
    html! {
        <article class="min-h-44 p-3">
            <div class="mb-2 flex items-center justify-between">
                <h3 class={classes!("text-[10px]","font-semibold","uppercase","tracking-[0.13em]", if results { "text-slate-300" } else { "text-emerald-300" })}>{ title }</h3>
                <span class="text-[10px] text-slate-500">{ runs.len() }</span>
            </div>
            if runs.is_empty() {
                <p class="py-5 text-center text-[10px] italic text-slate-600">{ if results { "No recent results." } else { "The engine is idle." } }</p>
            } else {
                <div class="space-y-1.5">
                    { for runs.iter().take(8).map(|run| engine_run_card(run, results)) }
                </div>
            }
        </article>
    }
}

fn engine_run_card(run: &PortalTechEngineRun, results: bool) -> Html {
    let status = if run.stale { "INTERRUPTED" } else { run.status.as_str() };
    html! {
        <div class="rounded-md border border-white/10 bg-white/[0.025] px-2.5 py-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="truncate text-[10px] text-slate-200">{ run.title.clone() }</p>
                    <p class="mt-1 font-mono text-[8px] text-slate-600">
                        { format!("{} · attempt {}", compact_id(&run.story_id), run.attempts) }
                    </p>
                </div>
                <span class={classes!(
                    "shrink-0","rounded-full","border","px-1.5","py-0.5","text-[8px]","uppercase",
                    if run.status == "failed" { "border-rose-400/30 text-rose-300" }
                    else if run.stale { "border-amber-400/30 text-amber-300" }
                    else if results { "border-white/15 text-slate-400" }
                    else { "border-emerald-400/30 text-emerald-300" }
                )}>{ status }</span>
            </div>
            if results && !run.instance_id.is_empty() {
                <a
                    href={format!("/portal/tech/flight-recorder/{}", run.instance_id)}
                    class="mt-2 inline-block text-[8px] uppercase tracking-[0.1em] text-[#c6a15b] hover:text-[#e0c489]"
                >
                    {"inspect run →"}
                </a>
            }
        </div>
    }
}

fn recent_history(tech: &PortalTechPage) -> Html {
    html! {
        <section class="rounded-lg border border-white/10 bg-white/[0.02] p-4">
            <div class="mb-3">
                <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-[#c6a15b]">{"WHAT JUST WENT THROUGH"}</p>
                <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"Recent story history"}</h2>
            </div>
            if tech.recent_history.is_empty() {
                <p class="py-5 text-center text-xs italic text-slate-600">{"No story run history yet."}</p>
            } else {
                <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                    { for tech.recent_history.iter().map(history_card) }
                </div>
            }
        </section>
    }
}

fn history_card(item: &PortalTechHistory) -> Html {
    let result = item.latest_run_result.as_deref().unwrap_or("Unknown");
    html! {
        <article class="rounded-md border border-white/10 bg-white/[0.03] p-3">
            <div class="flex items-start justify-between gap-2">
                <span class="font-mono text-[9px] text-slate-500">{ compact_id(&item.id) }</span>
                <span class={classes!(
                    "rounded-full","border","px-1.5","py-0.5","text-[8px]","uppercase",
                    if matches!(result, "Complete" | "Passed") { "border-emerald-400/30 text-emerald-300" }
                    else if matches!(result, "Failed" | "Blocked" | "Cancelled") { "border-rose-400/30 text-rose-300" }
                    else { "border-white/15 text-slate-400" }
                )}>{ result }</span>
            </div>
            <p class="mt-2 text-[11px] leading-4 text-white/75">{ item.title.clone() }</p>
            <p class="mt-2 text-[9px] text-slate-600">
                { item.latest_run_at.as_deref().map(short_time).unwrap_or_else(|| "time not recorded".into()) }
            </p>
        </article>
    }
}

fn command_notice(model: &crate::model::Model) -> Html {
    let Some(notice) = model.tech.notice.as_ref() else {
        return Html::default();
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
    tech.queued_cards.iter().any(|item| item.story_id == story_id)
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
        .filter(|run| {
            !run.stale
                && matches!(run.status.as_str(), "running" | "claimed" | "queued")
        })
        .collect()
}

fn result_runs(tech: &PortalTechPage) -> Vec<&PortalTechEngineRun> {
    tech.engine_runs
        .iter()
        .filter(|run| {
            run.stale
                || matches!(run.status.as_str(), "completed" | "failed" | "interrupted")
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
