//! /portal/storyboard — read-only TECH snapshot of the authoritative story backlog.
//!
//! The data contract is deliberately the existing storyboard rows projection. This screen does not edit stories,
//! dispatch Forge, or reproduce the old TypeScript board controls; those are separate concerns. Yew owns the route,
//! state and rendering, and each row links to the already-existing story record.

use std::collections::BTreeMap;

use yew::prelude::*;

use crate::model::{Msg, Row};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct StoryboardProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Storyboard;

impl Component for Storyboard {
    type Message = ();
    type Properties = StoryboardProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("storyboard").expect("storyboard screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { storyboard(&props.model) }
            </PortalShell>
        }
    }
}

fn storyboard(model: &crate::model::Model) -> Html {
    if model.loading && model.rows.is_empty() {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">{"Loading Story Board…"}</p>
            </section>
        };
    }

    let counts = status_counts(&model.rows);
    let complete = count(&counts, "Complete");
    let active = count(&counts, "In Progress") + count(&counts, "Partial");
    let ready = count(&counts, "Ready") + count(&counts, "Batched");
    let attention = count(&counts, "Blocked") + count(&counts, "Failed") + count(&counts, "Hold");

    html! {
        <div class="flex flex-col gap-4">
            <header class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
                <p class="text-[10px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold)]">
                    {"TECH · Authoritative backlog"}
                </p>
                <div class="mt-1 flex flex-wrap items-end justify-between gap-3">
                    <div>
                        <h1 class="font-serif text-3xl font-light text-[var(--portal-navy)]">{"Story Board"}</h1>
                        <p class="mt-2 max-w-3xl text-sm font-light leading-6 text-black/55">
                            {"A read-only snapshot of the stories in the engineering backlog. Status, priority, batch and operating surface come directly from the stored story record."}
                        </p>
                    </div>
                    <span class="rounded-full border border-[var(--portal-gold)]/45 bg-[var(--portal-gold-pale)] px-3 py-1 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                        { format!("{} stories", model.rows.len()) }
                    </span>
                </div>
            </header>

            { summary(model.rows.len(), complete, active, ready, attention) }

            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-5 py-4">
                    <div>
                        <h2 class="font-serif text-xl font-light text-[var(--portal-panel-heading)]">{"Stories"}</h2>
                        <p class="mt-0.5 text-[10px] font-light text-black/40">
                            {"Open a story for its full execution specification and history."}
                        </p>
                    </div>
                    if model.loading {
                        <span class="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                            {"Refreshing…"}
                        </span>
                    }
                </div>

                if model.rows.is_empty() {
                    <div class="px-5 py-10 text-center text-sm font-light text-black/40">
                        {"No stories are available."}
                    </div>
                } else {
                    <div class="overflow-x-auto">
                        <table class="w-full min-w-[900px] text-left">
                            <thead>
                                <tr class="border-b border-[var(--portal-border)] bg-white/20 text-[9px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                                    <th class="px-5 py-2.5">{"Story"}</th>
                                    <th class="px-4 py-2.5">{"Workstream"}</th>
                                    <th class="px-4 py-2.5">{"Surface"}</th>
                                    <th class="px-4 py-2.5">{"Priority"}</th>
                                    <th class="px-4 py-2.5">{"Batch"}</th>
                                    <th class="px-5 py-2.5 text-right">{"Status"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                { for model.rows.iter().map(story_row) }
                            </tbody>
                        </table>
                    </div>
                }
            </section>
        </div>
    }
}

fn summary(total: usize, complete: usize, active: usize, ready: usize, attention: usize) -> Html {
    let metrics = [
        ("Total", total, false),
        ("Complete", complete, false),
        ("Active", active, false),
        ("Ready / Batched", ready, false),
        ("Needs attention", attention, attention > 0),
    ];

    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-5">
                { for metrics.into_iter().map(|(label, value, alert)| html! {
                    <div class="border-b border-r border-[var(--portal-border)] px-4 py-3 last:border-r-0 xl:border-b-0">
                        <p class="text-[9px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">{ label }</p>
                        <p class={classes!(
                            "mt-1", "font-serif", "text-2xl", "font-light",
                            if alert { "text-[var(--portal-archive)]" } else { "text-[var(--portal-navy)]" }
                        )}>{ value }</p>
                    </div>
                }) }
            </div>
        </section>
    }
}

fn story_row(row: &Row) -> Html {
    let title = cell(row, 0);
    let workstream = cell(row, 1);
    let priority = cell(row, 2);
    let batch = cell(row, 3);
    let surface = cell(row, 4);
    let status = row.badge.as_deref().unwrap_or("Unknown");
    let href = format!("/portal/storyboard/{}", row.id);

    html! {
        <tr class="border-b border-[var(--portal-border)] last:border-b-0 transition hover:bg-white/25">
            <td class="px-5 py-3">
                <a href={href} class="block">
                    <span class="font-mono text-[11px] text-[var(--portal-navy-soft)]">{ row.id.clone() }</span>
                    <span class="mt-0.5 block max-w-[520px] text-sm font-medium text-[var(--portal-navy)]">{ title }</span>
                </a>
            </td>
            <td class="px-4 py-3 text-xs font-light text-black/55">{ workstream }</td>
            <td class="px-4 py-3">
                <span class="rounded-full border border-[var(--portal-border)] bg-white/35 px-2 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
                    { surface }
                </span>
            </td>
            <td class="px-4 py-3 text-xs font-light text-black/55">{ priority }</td>
            <td class="px-4 py-3 text-xs font-light text-black/45">{ batch }</td>
            <td class="px-5 py-3 text-right">{ status_badge(status) }</td>
        </tr>
    }
}

fn cell(row: &Row, index: usize) -> String {
    row.cells.get(index).cloned().unwrap_or_else(|| "—".into())
}

fn status_counts(rows: &[Row]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        let status = row.badge.as_deref().unwrap_or("Unknown").to_string();
        *counts.entry(status).or_insert(0) += 1;
    }
    counts
}

fn count(counts: &BTreeMap<String, usize>, status: &str) -> usize {
    counts.get(status).copied().unwrap_or(0)
}

fn status_badge(status: &str) -> Html {
    let tone = match status {
        "Complete" => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
        "In Progress" | "Partial" => "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
        "Ready" | "Batched" => "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]",
        "Blocked" | "Failed" | "Hold" => "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]",
        _ => "bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]",
    };
    html! {
        <span class={classes!(
            "inline-flex", "whitespace-nowrap", "rounded-full", "px-2.5", "py-1",
            "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]", tone
        )}>
            { status }
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story(id: &str, status: &str) -> Row {
        Row {
            id: id.into(),
            cells: vec![
                format!("Story {id}"),
                "HARDEN".into(),
                "High".into(),
                "no batch".into(),
                "TECH".into(),
            ],
            badge: Some(status.into()),
        }
    }

    #[test]
    fn snapshot_counts_status_without_inventing_story_state() {
        let rows = vec![
            story("A", "Complete"),
            story("B", "In Progress"),
            story("C", "Ready"),
            story("D", "Blocked"),
        ];
        let counts = status_counts(&rows);
        assert_eq!(count(&counts, "Complete"), 1);
        assert_eq!(count(&counts, "In Progress"), 1);
        assert_eq!(count(&counts, "Ready"), 1);
        assert_eq!(count(&counts, "Blocked"), 1);
        assert_eq!(count(&counts, "Unknown"), 0);
    }

    #[test]
    fn missing_optional_cells_render_as_dash() {
        let row = Row {
            id: "A".into(),
            cells: vec!["Title".into()],
            badge: None,
        };
        assert_eq!(cell(&row, 0), "Title");
        assert_eq!(cell(&row, 4), "—");
    }
}
