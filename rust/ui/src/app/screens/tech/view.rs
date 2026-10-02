//! TECH / Forge Live Operations Cockpit.
//!
//! Presentation-only V2 concept. All values come from the screen's local MVI model; this view
//! deliberately knows nothing about APIs, Neon, OpenCode, or Forge DAOs.

use yew::prelude::*;

use super::{
    find_agent, ActivityEvent, AgentNode, AgentState, EngineResult, EventKind, Model, Msg,
    ResultState,
};

pub(super) fn cockpit(model: &Model, on_msg: &Callback<Msg>) -> Html {
    html! {
        <div class="min-h-screen rounded-xl bg-[#07101d] px-4 py-4 text-slate-200 sm:px-5 sm:py-5">
            { header(model) }
            { notice(model, on_msg) }
            { run_strip(model, on_msg) }

            <div class="mt-4 grid gap-4 xl:grid-cols-[minmax(18rem,0.72fr)_minmax(0,1.28fr)]">
                { agent_panel(model, on_msg) }
                <div class="space-y-4">
                    { activity_panel(model) }
                    { budget_panel(model) }
                </div>
            </div>

            <div class="mt-4">
                { results_queue(model, on_msg) }
            </div>
        </div>
    }
}

fn header(model: &Model) -> Html {
    html! {
        <header class="flex flex-wrap items-end justify-between gap-3">
            <div>
                <div class="flex flex-wrap items-center gap-2">
                    <p class="text-[10px] font-semibold uppercase tracking-[0.22em] text-[#c6a15b]">
                        {"TECH / FORGE LIVE OPERATIONS"}
                    </p>
                    <span class="rounded-full border border-[#c6a15b]/30 bg-[#c6a15b]/10 px-2 py-0.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[#e0c489]">
                        {"MVI DEMO"}
                    </span>
                    <span class="rounded-full border border-sky-400/20 bg-sky-400/10 px-2 py-0.5 text-[9px] uppercase tracking-[0.1em] text-sky-200">
                        {"backend disconnected"}
                    </span>
                </div>
                <h1 class="mt-1 font-serif text-2xl font-semibold text-white">{"Forge Cockpit"}</h1>
                <p class="mt-1 max-w-3xl text-sm font-light text-slate-400">
                    {"The instrument panel while Forge is in the air: live workflow, agents, execution facts, spend, turns, and engine output."}
                </p>
            </div>
            <div class="flex items-center gap-2 text-[10px] text-slate-500">
                <span>{"Run"}</span>
                <span class="font-mono text-slate-300">{ model.run_id }</span>
            </div>
        </header>
    }
}

fn notice(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let Some(message) = model.notice.as_deref() else {
        return Html::default();
    };
    let close = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ClearNotice))
    };
    html! {
        <div class="mt-4 flex items-start justify-between gap-4 rounded-lg border border-sky-400/20 bg-sky-400/[0.07] px-4 py-3">
            <p class="text-xs leading-5 text-sky-100/80">{ message.to_string() }</p>
            <button type="button" onclick={close} class="shrink-0 text-sm text-sky-200/60 hover:text-sky-100" aria-label="Dismiss">
                {"×"}
            </button>
        </div>
    }
}

fn run_strip(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let hold = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DemoAction("HOLD")))
    };
    let stop = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DemoAction("STOP RUN")))
    };
    let spend_percent = if model.story_budget_usd > 0.0 {
        ((model.story_spend_usd / model.story_budget_usd) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let turn_percent = if model.turn_cap > 0 {
        ((model.turns_used as f64 / model.turn_cap as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    html! {
        <section class="mt-4 overflow-hidden rounded-xl border border-white/10 bg-white/[0.035]">
            <div class="grid gap-px bg-white/10 lg:grid-cols-[minmax(18rem,1.5fr)_repeat(4,minmax(8rem,0.65fr))_auto]">
                <div class="bg-[#0a1422] px-4 py-3">
                    <div class="flex items-center gap-2">
                        <span class="inline-flex items-center gap-1.5 rounded-full border border-emerald-400/25 bg-emerald-400/10 px-2 py-0.5 text-[9px] font-semibold uppercase tracking-[0.12em] text-emerald-300">
                            <span class="h-1.5 w-1.5 rounded-full bg-emerald-300" />
                            { model.status }
                        </span>
                        <span class="font-mono text-[10px] text-[#e0c489]">{ model.story_id }</span>
                    </div>
                    <p class="mt-1.5 truncate font-serif text-lg font-semibold text-white">{ model.story_title }</p>
                    <p class="mt-1 truncate text-[10px] text-slate-500">
                        { format!("{} · base {} · candidate {}", model.work_item, model.base_sha, model.candidate_sha) }
                    </p>
                </div>

                { metric("Current role", model.role, "Forge workflow") }
                { metric("Model", model.model_name, "Forge selected") }
                { metric("Runtime", model.runtime, "active role") }

                <div class="bg-[#0a1422] px-3 py-3">
                    <p class="text-[9px] font-semibold uppercase tracking-[0.12em] text-slate-500">{"Story budget"}</p>
                    <div class="mt-1 flex items-baseline justify-between gap-2">
                        <span class="font-mono text-sm text-white">{ format!("{}{:.3}", "$", model.story_spend_usd) }</span>
                        <span class="text-[9px] text-slate-500">{ format!("of {}{:.2}", "$", model.story_budget_usd) }</span>
                    </div>
                    <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-white/10">
                        <div class="h-full rounded-full bg-[#c6a15b]" style={format!("width:{spend_percent:.1}%")} />
                    </div>
                </div>

                <div class="bg-[#0a1422] px-3 py-3">
                    <p class="text-[9px] font-semibold uppercase tracking-[0.12em] text-slate-500">{"Model turns"}</p>
                    <div class="mt-1 flex items-baseline justify-between gap-2">
                        <span class="font-mono text-sm text-white">{ format!("{} / {}", model.turns_used, model.turn_cap) }</span>
                        <span class="text-[9px] text-slate-500">{ format!("{} left", model.turn_cap.saturating_sub(model.turns_used)) }</span>
                    </div>
                    <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-white/10">
                        <div class="h-full rounded-full bg-sky-400" style={format!("width:{turn_percent:.1}%")} />
                    </div>
                </div>

                <div class="flex items-center gap-2 bg-[#0a1422] px-3 py-3">
                    <button type="button" onclick={hold} class="rounded-md border border-amber-300/30 bg-amber-300/10 px-3 py-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-amber-200 hover:bg-amber-300/15">
                        {"Hold"}
                    </button>
                    <button type="button" onclick={stop} class="rounded-md border border-rose-400/30 bg-rose-400/10 px-3 py-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-rose-200 hover:bg-rose-400/15">
                        {"Stop"}
                    </button>
                </div>
            </div>

            <div class="flex flex-wrap items-center gap-x-5 gap-y-1 border-t border-white/10 bg-black/10 px-4 py-2 text-[9px] text-slate-500">
                <span>
                    {"Worktree "}
                    <span class="font-mono text-slate-300">{ model.worktree }</span>
                </span>
                <span>{"Controls are local MVI preview actions only."}</span>
            </div>
        </section>
    }
}

fn metric(label: &'static str, value: &'static str, hint: &'static str) -> Html {
    html! {
        <div class="bg-[#0a1422] px-3 py-3">
            <p class="text-[9px] font-semibold uppercase tracking-[0.12em] text-slate-500">{ label }</p>
            <p class="mt-1 truncate text-sm font-medium text-white" title={value}>{ value }</p>
            <p class="mt-1 text-[9px] text-slate-600">{ hint }</p>
        </div>
    }
}

fn agent_panel(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let selected = find_agent(&model.agents, &model.selected_agent);
    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="flex items-center justify-between border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] font-semibold uppercase tracking-[0.15em] text-[#c6a15b]">{"WORKFLOW / AGENTS"}</p>
                    <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"Live execution tree"}</h2>
                </div>
                <span class="rounded-full border border-white/10 px-2 py-0.5 text-[9px] uppercase tracking-[0.1em] text-slate-400">
                    {"Forge owns routing"}
                </span>
            </div>

            <div class="p-2">
                { for model.agents.iter().map(|agent| agent_row(agent, 0, &model.selected_agent, on_msg)) }
            </div>

            if let Some(agent) = selected {
                { agent_detail(agent) }
            }
        </section>
    }
}

fn agent_row(agent: &AgentNode, depth: usize, selected_id: &str, on_msg: &Callback<Msg>) -> Html {
    let selected = agent.id == selected_id;
    let id = agent.id.to_string();
    let select = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::AgentSelected(id.clone())))
    };
    let indent = depth * 18;

    html! {
        <>
            <button
                type="button"
                onclick={select}
                class={classes!(
                    "mb-1", "grid", "w-full", "grid-cols-[1.2rem_minmax(0,1fr)_auto]", "items-center", "gap-2",
                    "rounded-md", "border", "px-2.5", "py-2", "text-left", "transition",
                    if selected { "border-[#c6a15b]/55 bg-[#c6a15b]/10" } else { "border-transparent hover:border-white/10 hover:bg-white/[0.025]" }
                )}
                style={format!("padding-left:{}px", 10 + indent)}
            >
                <span class={classes!("text-xs", agent_state_tone(agent.state))}>{ agent.state.glyph() }</span>
                <span class="min-w-0">
                    <span class="block truncate text-xs font-medium text-white">{ agent.label }</span>
                    <span class="mt-0.5 block truncate text-[9px] text-slate-500">{ agent.detail }</span>
                </span>
                <span class={classes!("rounded-full", "border", "px-2", "py-0.5", "text-[8px]", "uppercase", "tracking-[0.1em]", agent_state_badge(agent.state))}>
                    { agent.state.label() }
                </span>
            </button>
            { for agent.children.iter().map(|child| agent_row(child, depth + 1, selected_id, on_msg)) }
        </>
    }
}

fn agent_detail(agent: &AgentNode) -> Html {
    let steps = format!("{} / {}", agent.steps_used, agent.step_cap);
    let spend = format!("{}{:.3}", "$", agent.cost_usd);
    html! {
        <div class="border-t border-white/10 bg-black/10 p-4">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <p class="text-[9px] uppercase tracking-[0.12em] text-slate-500">{"Selected agent"}</p>
                    <p class="mt-0.5 font-serif text-base font-semibold text-white">{ agent.label }</p>
                </div>
                <span class={classes!("text-xs", agent_state_tone(agent.state))}>{ agent.state.label() }</span>
            </div>
            <p class="mt-2 text-xs leading-5 text-slate-400">{ agent.detail }</p>
            <div class="mt-3 grid grid-cols-2 gap-2 text-[9px] sm:grid-cols-4">
                { detail_cell("Model", agent.model) }
                { detail_cell("Session", agent.session) }
                { detail_cell("Steps", &steps) }
                { detail_cell("Spend", &spend) }
            </div>
            <div class="mt-2 text-[9px] text-slate-600">
                { format!("{} tokens measured in this preview lane", agent.tokens) }
            </div>
        </div>
    }
}

fn detail_cell(label: &'static str, value: &str) -> Html {
    let value = value.to_string();
    html! {
        <div class="rounded-md border border-white/10 bg-white/[0.02] px-2.5 py-2">
            <p class="uppercase tracking-[0.1em] text-slate-600">{ label }</p>
            <p class="mt-1 truncate font-mono text-slate-300" title={value.clone()}>{ value }</p>
        </div>
    }
}

fn activity_panel(model: &Model) -> Html {
    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="flex items-center justify-between border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] font-semibold uppercase tracking-[0.15em] text-[#c6a15b]">{"CURRENT EXECUTION"}</p>
                    <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"Structured activity"}</h2>
                </div>
                <span class="text-[9px] text-slate-500">{"facts, not chain-of-thought"}</span>
            </div>
            <div class="max-h-[25rem] divide-y divide-white/5 overflow-y-auto">
                { for model.activity.iter().map(activity_row) }
            </div>
        </section>
    }
}

fn activity_row(event: &ActivityEvent) -> Html {
    html! {
        <div class="grid grid-cols-[4.5rem_5.5rem_minmax(0,1fr)] gap-3 px-4 py-2.5">
            <span class="font-mono text-[9px] text-slate-600">{ event.at }</span>
            <span class={classes!("text-[9px]", "font-semibold", "uppercase", "tracking-[0.1em]", event_tone(event.kind))}>
                { event.kind.label() }
            </span>
            <div class="min-w-0">
                <p class="truncate text-[11px] font-medium text-slate-200">{ event.title }</p>
                <p class="mt-0.5 truncate text-[9px] text-slate-500">{ event.detail }</p>
            </div>
        </div>
    }
}

fn budget_panel(model: &Model) -> Html {
    let remaining = (model.story_budget_usd - model.story_spend_usd).max(0.0);
    html! {
        <section class="grid gap-3 sm:grid-cols-3">
            <article class="rounded-xl border border-white/10 bg-white/[0.025] p-4">
                <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Spend remaining"}</p>
                <p class="mt-2 font-serif text-2xl text-white">{ format!("{}{:.3}", "$", remaining) }</p>
                <p class="mt-1 text-[9px] text-slate-600">{"hard stop will live in Forge"}</p>
            </article>
            <article class="rounded-xl border border-white/10 bg-white/[0.025] p-4">
                <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Turns remaining"}</p>
                <p class="mt-2 font-serif text-2xl text-white">{ model.turn_cap.saturating_sub(model.turns_used) }</p>
                <p class="mt-1 text-[9px] text-slate-600">{"generation cap"}</p>
            </article>
            <article class="rounded-xl border border-white/10 bg-white/[0.025] p-4">
                <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Candidate"}</p>
                <p class="mt-2 truncate font-mono text-sm text-[#e0c489]">{ model.candidate_sha }</p>
                <p class="mt-1 text-[9px] text-slate-600">{"QA gate pending"}</p>
            </article>
        </section>
    }
}

fn results_queue(model: &Model, on_msg: &Callback<Msg>) -> Html {
    let selected = model
        .results
        .iter()
        .find(|result| result.id == model.selected_result)
        .or_else(|| model.results.first());

    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="flex flex-wrap items-center justify-between gap-2 border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] font-semibold uppercase tracking-[0.15em] text-[#c6a15b]">{"ENGINE RESULTS QUEUE"}</p>
                    <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"What the factory produced"}</h2>
                </div>
                <p class="text-[9px] text-slate-500">{"replaces Workbench / Next Version presentation"}</p>
            </div>

            <div class="grid xl:grid-cols-[minmax(0,1.45fr)_minmax(18rem,0.55fr)]">
                <div class="divide-y divide-white/5">
                    { for model.results.iter().map(|result| result_row(result, &model.selected_result, on_msg)) }
                </div>

                if let Some(result) = selected {
                    { result_detail(result) }
                }
            </div>
        </section>
    }
}

fn result_row(result: &EngineResult, selected_id: &str, on_msg: &Callback<Msg>) -> Html {
    let id = result.id.to_string();
    let selected = result.id == selected_id;
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ResultSelected(id.clone())))
    };

    html! {
        <button
            type="button"
            {onclick}
            class={classes!(
                "grid", "w-full", "grid-cols-[8.5rem_minmax(0,1fr)_8rem_6rem]", "items-center", "gap-3",
                "px-4", "py-3", "text-left", "transition",
                if selected { "bg-[#c6a15b]/[0.08]" } else { "hover:bg-white/[0.025]" }
            )}
        >
            <span class="font-mono text-[10px] text-slate-400">{ result.id }</span>
            <span class="min-w-0">
                <span class="block truncate text-[11px] font-medium text-slate-200">{ result.title }</span>
                <span class="mt-0.5 block truncate text-[9px] text-slate-600">{ result.detail }</span>
            </span>
            <span class={classes!("rounded-full", "border", "px-2", "py-1", "text-center", "text-[8px]", "font-semibold", "uppercase", "tracking-[0.08em]", result_badge(result.state))}>
                { result.state.label() }
            </span>
            <span class="text-right font-mono text-[9px] text-slate-500">{ result.tests }</span>
        </button>
    }
}

fn result_detail(result: &EngineResult) -> Html {
    html! {
        <aside class="border-t border-white/10 bg-black/10 p-4 xl:border-l xl:border-t-0">
            <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Selected result"}</p>
            <p class="mt-1 font-mono text-[10px] text-[#e0c489]">{ result.id }</p>
            <h3 class="mt-2 font-serif text-lg font-semibold text-white">{ result.title }</h3>
            <p class="mt-2 text-xs leading-5 text-slate-400">{ result.detail }</p>
            <div class="mt-4 grid grid-cols-2 gap-2">
                { detail_cell("State", result.state.label()) }
                { detail_cell("Tests", result.tests) }
                { detail_cell("Candidate", result.candidate.unwrap_or("—")) }
                { detail_cell("History", "Flight Recorder") }
            </div>
            <p class="mt-3 text-[9px] leading-4 text-slate-600">
                {"When live data is connected, completed runs drill into Flight Recorder; active runs stay in Cockpit."}
            </p>
        </aside>
    }
}

fn agent_state_tone(state: AgentState) -> &'static str {
    match state {
        AgentState::Complete => "text-emerald-300",
        AgentState::Running => "text-sky-300",
        AgentState::Waiting => "text-slate-600",
        AgentState::Hold => "text-amber-300",
    }
}

fn agent_state_badge(state: AgentState) -> &'static str {
    match state {
        AgentState::Complete => "border-emerald-400/20 bg-emerald-400/10 text-emerald-300",
        AgentState::Running => "border-sky-400/20 bg-sky-400/10 text-sky-300",
        AgentState::Waiting => "border-white/10 bg-white/[0.02] text-slate-500",
        AgentState::Hold => "border-amber-400/20 bg-amber-400/10 text-amber-300",
    }
}

fn event_tone(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Read => "text-slate-400",
        EventKind::Search => "text-violet-300",
        EventKind::Edit => "text-[#e0c489]",
        EventKind::Test => "text-emerald-300",
        EventKind::Subagent => "text-sky-300",
        EventKind::Review => "text-cyan-300",
        EventKind::Decision => "text-amber-300",
    }
}

fn result_badge(state: ResultState) -> &'static str {
    match state {
        ResultState::ReadyQa => "border-sky-400/20 bg-sky-400/10 text-sky-300",
        ResultState::QaPassed => "border-emerald-400/20 bg-emerald-400/10 text-emerald-300",
        ResultState::Hold => "border-amber-400/20 bg-amber-400/10 text-amber-300",
        ResultState::Failed => "border-rose-400/20 bg-rose-400/10 text-rose-300",
        ResultState::ReadyPublish => "border-[#c6a15b]/25 bg-[#c6a15b]/10 text-[#e0c489]",
    }
}
