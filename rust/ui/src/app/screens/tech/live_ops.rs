//! TECH Cockpit / Work in Flight.
//!
//! Pure MVI presentation over the parent ForgeService read model. The Yew screen
//! never reads Neon, Workflow, OpenCode, or JobService directly.

use domain::{ForgeLiveRun, ForgeLiveSnapshot, ForgeLiveWorkItem};
use yew::prelude::*;

use crate::app::cmd::Cmd;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub(super) selected_story: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    WorkSelected(String),
}

pub(super) fn init() -> Model {
    Model::default()
}

pub(super) fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::WorkSelected(story_id) => model.selected_story = Some(story_id),
    }
    Cmd::none()
}

fn selected<'a>(model: &Model, live: &'a ForgeLiveSnapshot) -> Option<&'a ForgeLiveWorkItem> {
    model
        .selected_story
        .as_deref()
        .and_then(|id| live.active_work.iter().find(|item| item.story_id == id))
        .or_else(|| {
            live.selected_story_id
                .as_deref()
                .and_then(|id| live.active_work.iter().find(|item| item.story_id == id))
        })
        .or_else(|| live.active_work.first())
}

pub(super) fn view(model: &Model, live: &ForgeLiveSnapshot, on_msg: &Callback<Msg>) -> Html {
    let chosen = selected(model, live);
    html! {
        <div class="min-h-[36rem] rounded-xl bg-[#07101d] py-4 text-slate-200">
            <header class="flex flex-wrap items-end justify-between gap-3">
                <div>
                    <div class="flex items-center gap-2">
                        <p class="text-[10px] font-semibold uppercase tracking-[0.22em] text-[#c6a15b]">{"TECH / FORGE WORK IN FLIGHT"}</p>
                        <span class="rounded-full border border-emerald-400/25 bg-emerald-400/10 px-2 py-0.5 text-[9px] uppercase tracking-[0.12em] text-emerald-300">{"live service"}</span>
                    </div>
                    <h2 class="mt-1 font-serif text-2xl font-semibold text-white">{"Work in Flight"}</h2>
                    <p class="mt-1 text-sm font-light text-slate-400">
                        {"Claims, live role/run identity, OpenCode V2 session, measured usage, and Workflow node activity."}
                    </p>
                </div>
                <div class="rounded-lg border border-white/10 bg-white/[0.025] px-3 py-2 text-right">
                    <p class="text-[9px] uppercase tracking-[0.12em] text-slate-500">{"active claims"}</p>
                    <p class="font-mono text-lg text-white">{ live.active_work.len() }</p>
                </div>
            </header>

            <div class="mt-4 grid gap-4 xl:grid-cols-[minmax(18rem,0.72fr)_minmax(0,1.28fr)]">
                { work_list(live, chosen, on_msg) }
                <div class="space-y-4">
                    { run_panel(chosen, live.current_run.as_ref()) }
                    { activity_panel(live) }
                </div>
            </div>
        </div>
    }
}

fn work_list(live: &ForgeLiveSnapshot, chosen: Option<&ForgeLiveWorkItem>, on_msg: &Callback<Msg>) -> Html {
    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="border-b border-white/10 px-4 py-3">
                <p class="text-[9px] uppercase tracking-[0.14em] text-slate-500">{"Forge ownership"}</p>
                <h3 class="font-serif text-lg font-semibold text-white">{"In-flight work"}</h3>
            </div>
            if live.active_work.is_empty() {
                <div class="px-5 py-12 text-center">
                    <p class="font-serif text-lg text-white/75">{"Forge is idle"}</p>
                    <p class="mt-1 text-xs text-slate-500">{"No Claimed, Running, or Paused work item is currently owned by the engine."}</p>
                </div>
            } else {
                <div class="max-h-[38rem] space-y-1.5 overflow-y-auto p-2">
                    { for live.active_work.iter().map(|item| {
                        let active = chosen.is_some_and(|current| current.work_item_id == item.work_item_id);
                        let story_id = item.story_id.clone();
                        let onclick = {
                            let on_msg = on_msg.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::WorkSelected(story_id.clone())))
                        };
                        html! {
                            <button type="button" {onclick}
                                class={classes!(
                                    "w-full","rounded-lg","border","px-3","py-3","text-left","transition",
                                    if active { "border-[#c6a15b]/55 bg-[#c6a15b]/10" } else { "border-white/10 bg-black/10 hover:border-white/20" }
                                )}>
                                <div class="flex items-start justify-between gap-3">
                                    <div class="min-w-0">
                                        <p class="font-mono text-[9px] text-[#e0c489]">{ item.story_id.clone() }</p>
                                        <p class="mt-1 truncate text-xs font-medium text-white/85">{ item.title.clone() }</p>
                                    </div>
                                    { state_badge(&item.state) }
                                </div>
                                <p class="mt-2 truncate font-mono text-[9px] text-slate-500">
                                    {
                                        format!(
                                            "{} · {} · {}",
                                            item.kind.as_deref().unwrap_or("normal"),
                                            item.model_policy.as_deref().unwrap_or("default"),
                                            item.claimed_by.as_deref().unwrap_or("unclaimed")
                                        )
                                    }
                                </p>
                            </button>
                        }
                    }) }
                </div>
            }
        </section>
    }
}

fn state_badge(state: &str) -> Html {
    let tone = match state {
        "Running" => "border-emerald-400/30 bg-emerald-400/10 text-emerald-300",
        "Claimed" => "border-sky-400/30 bg-sky-400/10 text-sky-300",
        "Paused" => "border-amber-400/30 bg-amber-400/10 text-amber-300",
        _ => "border-white/15 text-slate-400",
    };
    html! {
        <span class={classes!("shrink-0","rounded-full","border","px-2","py-0.5","text-[8px]","font-semibold","uppercase","tracking-[0.1em]",tone)}>
            { state }
        </span>
    }
}

fn run_panel(item: Option<&ForgeLiveWorkItem>, run: Option<&ForgeLiveRun>) -> Html {
    let Some(item) = item else {
        return html! {
            <section class="grid min-h-52 place-items-center rounded-xl border border-white/10 bg-white/[0.025] p-6 text-center text-sm text-slate-500">
                {"Select in-flight work to inspect its execution facts."}
            </section>
        };
    };
    let run = run.filter(|run| run.story_id == item.story_id);
    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="border-b border-white/10 px-4 py-3">
                <p class="font-mono text-[9px] uppercase tracking-[0.13em] text-[#c6a15b]">{ item.story_id.clone() }</p>
                <h3 class="mt-1 font-serif text-lg font-semibold text-white">{ item.title.clone() }</h3>
            </div>
            if let Some(run) = run {
                <div class="grid gap-px bg-white/10 sm:grid-cols-2 xl:grid-cols-4">
                    { metric("Role / phase", run.run_phase.as_deref().or(run.run_type.as_deref()).unwrap_or("—"), run.agent_runtime.as_deref().unwrap_or("Forge")) }
                    { metric("Model", run.model_used.as_deref().or(item.model_policy.as_deref()).unwrap_or("—"), "execution model") }
                    { metric("Session", run.vendor_session_id.as_deref().map(compact).unwrap_or_else(|| "—".into()), "OpenCode V2") }
                    { metric("Candidate", run.commit_hash.as_deref().map(compact).unwrap_or_else(|| "—".into()), run.result_status.as_deref().unwrap_or("in flight")) }
                </div>
                <div class="grid gap-3 p-4 lg:grid-cols-2">
                    <div class="rounded-lg border border-white/10 bg-black/10 p-3">
                        <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Measured usage"}</p>
                        <div class="mt-3 grid grid-cols-3 gap-3">
                            { small_metric("Input", run.tokens_input.map(|v| v.to_string()).unwrap_or_else(|| "—".into())) }
                            { small_metric("Output", run.tokens_output.map(|v| v.to_string()).unwrap_or_else(|| "—".into())) }
                            { small_metric("Cost", run.cost_usd.map(|v| format!("USD {v:.4}")).unwrap_or_else(|| "—".into())) }
                        </div>
                        if let Some(source) = run.cost_source.as_deref() {
                            <p class="mt-2 text-[9px] text-slate-600">{ format!("source: {source}") }</p>
                        }
                    </div>
                    <div class="rounded-lg border border-white/10 bg-black/10 p-3">
                        <p class="text-[9px] font-semibold uppercase tracking-[0.13em] text-slate-500">{"Run facts"}</p>
                        <div class="mt-2 space-y-1 text-[10px] text-slate-400">
                            <p>{ format!("run {}", compact(&run.id)) }</p>
                            <p>{ format!("started {}", run.started_at.as_deref().unwrap_or("—")) }</p>
                            <p>{ format!("tests {}", run.tests_summary.as_deref().unwrap_or("—")) }</p>
                        </div>
                    </div>
                </div>
                if let Some(detail) = run.evidence_detail.as_deref().filter(|value| !value.trim().is_empty()) {
                    <p class="max-h-28 overflow-y-auto whitespace-pre-wrap border-t border-white/10 px-4 py-3 font-mono text-[9px] leading-5 text-slate-500">{ detail }</p>
                }
            } else {
                <p class="px-4 py-8 text-center text-xs italic text-slate-500">{"This active work item has no Story Run read model yet."}</p>
            }
        </section>
    }
}

fn activity_panel(live: &ForgeLiveSnapshot) -> Html {
    html! {
        <section class="overflow-hidden rounded-xl border border-white/10 bg-white/[0.025]">
            <div class="flex items-center justify-between border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] uppercase tracking-[0.14em] text-slate-500">{"Workflow ownership trace"}</p>
                    <h3 class="font-serif text-lg font-semibold text-white">{"Node activity"}</h3>
                </div>
                <span class="rounded-full border border-white/10 px-2 py-0.5 text-[10px] text-slate-400">{ live.node_activity.len() }</span>
            </div>
            if live.node_activity.is_empty() {
                <p class="px-4 py-8 text-center text-xs italic text-slate-500">{"No node activity is recorded for the selected work."}</p>
            } else {
                <div class="max-h-72 divide-y divide-white/5 overflow-y-auto">
                    { for live.node_activity.iter().map(|event| html! {
                        <div class="grid grid-cols-[minmax(7rem,0.35fr)_minmax(0,1fr)_auto] gap-3 px-4 py-2.5 text-[10px]">
                            <span class="font-mono text-slate-500">{ compact(&event.process_instance_id) }</span>
                            <span class="truncate text-slate-300">{ event.node_id.clone() }</span>
                            <span class="uppercase text-slate-500">{ event.status.clone() }</span>
                        </div>
                    }) }
                </div>
            }
        </section>
    }
}

fn metric(label: &'static str, value: String, hint: &str) -> Html {
    html! {
        <div class="bg-[#0a1422] px-3 py-3">
            <p class="text-[9px] font-semibold uppercase tracking-[0.12em] text-slate-500">{ label }</p>
            <p class="mt-1 truncate font-mono text-xs text-white">{ value }</p>
            <p class="mt-1 truncate text-[8px] text-slate-600">{ hint }</p>
        </div>
    }
}

fn small_metric(label: &'static str, value: String) -> Html {
    html! {
        <div>
            <p class="text-[8px] uppercase tracking-[0.1em] text-slate-600">{ label }</p>
            <p class="mt-1 font-mono text-xs text-slate-200">{ value }</p>
        </div>
    }
}

fn compact(value: &str) -> String {
    if value.len() <= 14 {
        value.to_string()
    } else {
        format!("{}…{}", &value[..8], &value[value.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_falls_back_to_the_service_selected_story() {
        let live = ForgeLiveSnapshot {
            active_work: vec![ForgeLiveWorkItem {
                work_item_id: "work-1".into(),
                story_id: "S-1".into(),
                title: "One".into(),
                state: "Running".into(),
                ..Default::default()
            }],
            selected_story_id: Some("S-1".into()),
            ..Default::default()
        };
        assert_eq!(selected(&Model::default(), &live).map(|item| item.story_id.as_str()), Some("S-1"));
    }
}
