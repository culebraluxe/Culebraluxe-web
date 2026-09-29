//! The engine line: its queue, the running and finished runs, and recent history.

#[allow(unused_imports)]
use super::*;

pub(super) fn engine_line(tech: &PortalTechPage) -> Html {
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

pub(super) fn engine_queue(tech: &PortalTechPage) -> Html {
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

pub(super) fn engine_runs_panel(
    title: &'static str,
    runs: &[&PortalTechEngineRun],
    results: bool,
) -> Html {
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

pub(super) fn engine_run_card(run: &PortalTechEngineRun, results: bool) -> Html {
    let status = if run.stale {
        "INTERRUPTED"
    } else {
        run.status.as_str()
    };
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

pub(super) fn recent_history(tech: &PortalTechPage) -> Html {
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

pub(super) fn history_card(item: &PortalTechHistory) -> Html {
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
