//! Engine Work Status — terminal/retry outcomes beside the live engine queue.
//!
//! The queue itself is the sorter above this section. This panel answers a different
//! question: what did the engine do with work it already owned? The three buckets are
//! derived by ForgeReadDao as Done / Error / Retry; Yew only renders them.

use domain::ForgeLiveWorkItem;

use super::*;

pub(super) fn engine_line(tech: &PortalTechPage) -> Html {
    let by = |bucket: &str| {
        tech.live_ops
            .work_status
            .iter()
            .filter(|item| item.status_bucket.as_deref() == Some(bucket))
            .collect::<Vec<_>>()
    };
    let done = by("Done");
    let error = by("Error");
    let retry = by("Retry");

    html! {
        <section class="mb-4 overflow-hidden rounded-lg border border-white/10 bg-white/[0.02]">
            <div class="flex flex-wrap items-end justify-between gap-3 border-b border-white/10 px-4 py-3">
                <div>
                    <p class="text-[9px] font-semibold uppercase tracking-[0.16em] text-[#c6a15b]">{"FORGE ENGINE"}</p>
                    <h2 class="mt-0.5 font-serif text-lg font-semibold text-white">{"Engine Work Status"}</h2>
                    <p class="mt-1 text-[10px] text-slate-500">{"Work the engine already owned — successful, failed, or cleared back for retry."}</p>
                </div>
                <span class="text-[10px] text-slate-500">{ format!("{} recent", tech.live_ops.work_status.len()) }</span>
            </div>
            <div class="grid divide-y divide-white/10 lg:grid-cols-3 lg:divide-x lg:divide-y-0">
                { status_panel("DONE", &done) }
                { status_panel("ERROR", &error) }
                { status_panel("RETRY", &retry) }
            </div>
        </section>
    }
}

fn status_panel(title: &'static str, rows: &[&ForgeLiveWorkItem]) -> Html {
    let tone = match title {
        "DONE" => "text-emerald-300",
        "ERROR" => "text-rose-300",
        "RETRY" => "text-amber-300",
        _ => "text-slate-300",
    };
    html! {
        <article class="min-h-44 p-3">
            <div class="mb-2 flex items-center justify-between">
                <h3 class={classes!("text-[10px]","font-semibold","uppercase","tracking-[0.13em]",tone)}>{ title }</h3>
                <span class="text-[10px] text-slate-500">{ rows.len() }</span>
            </div>
            if rows.is_empty() {
                <p class="py-5 text-center text-[10px] italic text-slate-600">{"Nothing in this status."}</p>
            } else {
                <div class="space-y-1.5">
                    { for rows.iter().take(10).map(|item| status_card(item)) }
                </div>
            }
        </article>
    }
}

fn status_card(item: &ForgeLiveWorkItem) -> Html {
    let bucket = item.status_bucket.as_deref().unwrap_or(&item.state);
    let when = item
        .finished_at
        .as_deref()
        .or(item.updated_at.as_deref())
        .map(short_time)
        .unwrap_or_else(|| "time not recorded".into());
    html! {
        <div class="rounded-md border border-white/10 bg-white/[0.025] px-2.5 py-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="truncate text-[10px] text-slate-200">{ item.title.clone() }</p>
                    <p class="mt-1 font-mono text-[8px] text-slate-600">
                        { format!("{} · {}", compact_id(&item.story_id), when) }
                    </p>
                </div>
                <span class="shrink-0 rounded-full border border-white/15 px-1.5 py-0.5 text-[8px] uppercase text-slate-400">{ bucket }</span>
            </div>
            if let Some(error) = item.error_text.as_deref().filter(|value| !value.trim().is_empty()) {
                <p class="mt-2 line-clamp-2 text-[9px] leading-4 text-rose-200/65">{ error }</p>
            }
        </div>
    }
}
