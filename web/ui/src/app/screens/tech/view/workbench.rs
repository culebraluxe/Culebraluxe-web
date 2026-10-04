//! The Tech workbench: today's queue, its rows, the selected story and its runs.

#[allow(unused_imports)]
use super::*;

pub(super) fn workbench(model: &Vm<'_>, tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
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
            let confirmed = crate::app::exec::confirm(&prompt);
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
                        <span class="font-normal text-brand-gold">{ format!("({})", tech.active_work.len()) }</span>
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
                            class="rounded border border-white/15 px-2 py-1 text-[9px] uppercase tracking-[0.1em] text-slate-400 transition hover:border-brand-gold/40 hover:text-[#e0c489] disabled:opacity-35"
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

pub(super) fn workbench_queue(tech: &PortalTechPage, on_msg: &Callback<Msg>) -> Html {
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

pub(super) fn workbench_row(
    story: &PortalTechStory,
    tech: &PortalTechPage,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = tech
        .selected_story
        .as_ref()
        .is_some_and(|candidate| candidate.id == story.id);
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
                if selected { "border-brand-gold/60 bg-brand-gold/10" } else { "border-white/10 bg-white/[0.025] hover:border-brand-gold/30" }
            )}
        >
            <span class="truncate font-mono text-[10px] text-slate-300">{ story.id.clone() }</span>
            <span class="truncate text-[11px] font-light text-white/75">{ story.title.clone() }</span>
            <span class="text-right text-[9px] tabular-nums text-slate-500">{ format!("{}%", story.completion.round()) }</span>
        </button>
    }
}

pub(super) fn selected_story(
    model: &Vm<'_>,
    tech: &PortalTechPage,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(story) = tech.selected_story.as_ref() else {
        return html! {
            <article class="rounded-md border border-white/10 bg-white/[0.025] p-5 text-sm text-slate-500">
                {"Select a Workbench story to inspect it."}
            </article>
        };
    };
    let on_bench = tech
        .active_work
        .iter()
        .any(|candidate| candidate.id == story.id);
    let engine_owned = selected_engine_owned(tech, &story.id);
    let engine_live = selected_engine_live(tech, &story.id);
    let busy = model.tech.busy_action.is_some();

    let scoped = |target: &'static str| {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::TechScopedRunRequested(target.to_string()))
        })
    };
    let move_to = |target: &'static str| {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::TechMoveWorkbenchRequested(target.to_string()))
        })
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
                    if tech.recorder_instance_id.as_deref().is_some_and(|value| !value.is_empty()) {
                        <button
                            type="button"
                            onclick={on_msg.reform(|_: MouseEvent| Msg::TabSelected(TechTab::FlightRecorder))}
                            class="rounded-md border border-brand-gold/40 px-2.5 py-1.5 text-[9px] font-medium uppercase tracking-[0.11em] text-[#e0c489] hover:bg-brand-gold/10"
                        >
                            {"Flight Recorder →"}
                        </button>
                    }
                </div>
                <div class="mt-2 flex flex-wrap gap-2 text-[9px] uppercase tracking-[0.1em] text-slate-400">
                    <span class="rounded-full border border-white/10 px-2 py-0.5">{ story.status.clone() }</span>
                    <span>{ story.priority.clone() }</span>
                    <span>{ format!("{:.0}% complete", story.completion) }</span>
                    <span>{ story.workstream.clone() }</span>
                </div>

                <div class="mt-3 rounded-md border border-brand-gold/25 bg-brand-gold/[0.045] p-2.5">
                    <div class="flex flex-wrap items-center justify-between gap-2">
                        <div>
                            <p class="text-[9px] font-semibold uppercase tracking-[0.14em] text-[#e0c489]">{"INVESTIGATE"}</p>
                            <p class="mt-0.5 text-[9px] text-slate-500">{"Run only far enough to answer the question; keep the story on the Workbench."}</p>
                        </div>
                        <div class="flex flex-wrap gap-1.5">
                            { for [("scout", "Scout"), ("architect", "Architect"), ("lead", "Lead")].into_iter().map(|(key, label)| {
                                let action = scoped(key);
                                let busy_key = format!("scoped:{key}");
                                let active = model.tech.busy_action.as_deref() == Some(busy_key.as_str());
                                html! {
                                    <button
                                        type="button"
                                        onclick={action}
                                        disabled={busy || !on_bench || engine_live}
                                        class="rounded border border-white/15 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-slate-300 transition hover:border-brand-gold/40 hover:text-[#e0c489] disabled:cursor-not-allowed disabled:opacity-35"
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
                            <button type="button" onclick={move_to("backlog")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-brand-gold/30 hover:text-white disabled:opacity-35">{"Backlog"}</button>
                            <button type="button" onclick={move_to("closed")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-brand-gold/30 hover:text-white disabled:opacity-35">{"Closed"}</button>
                            <button type="button" onclick={move_to("next")} disabled={busy || !on_bench} class="rounded border border-white/10 px-2 py-0.5 text-[9px] text-slate-400 hover:border-brand-gold/30 hover:text-white disabled:opacity-35">{"Next Version"}</button>
                        </div>
                        <button
                            type="button"
                            onclick={good_to_go}
                            disabled={busy || !on_bench || engine_owned}
                            title="Remove from the Workbench and hand the full story to Forge. Ready queues real work."
                            class="rounded border border-brand-gold/50 bg-brand-gold/15 px-3 py-1 text-[9px] font-semibold uppercase tracking-[0.12em] text-[#e0c489] transition hover:bg-brand-gold/25 disabled:cursor-not-allowed disabled:opacity-35"
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
                        <summary class="cursor-pointer list-none text-[9px] font-semibold uppercase tracking-[0.14em] text-brand-gold/80">{ label }</summary>
                        <p class="mt-1 whitespace-pre-wrap text-[12px] font-light leading-5 text-white/70">
                            { value.unwrap_or("Not specified.") }
                        </p>
                    </details>
                }) }
            </div>
        </article>
    }
}

pub(super) fn selected_runs(tech: &PortalTechPage) -> Html {
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

pub(super) fn run_row(run: &PortalTechRun) -> Html {
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
