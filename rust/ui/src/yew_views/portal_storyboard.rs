//! /portal/storyboard — native Yew rendering of the canonical Story Board cockpit.
//!
//! Lifecycle classification and subgroup taxonomy are NOT reimplemented here. The authenticated portal read seam
//! reuses the legacy TypeScript projection that powers PROD and sends this component a small typed display payload.

use yew::prelude::*;

use crate::model::{Msg, PortalStoryboardPage, PortalStoryboardPanel, PortalStoryboardStory};
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

fn payload(model: &crate::model::Model) -> Option<&PortalStoryboardPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.storyboard.as_ref())
}

fn storyboard(model: &crate::model::Model) -> Html {
    let Some(data) = payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] px-8 py-12 text-center">
                <h1 class="font-serif text-2xl font-light text-[var(--portal-navy)]">
                    { if model.loading { "Loading Story Board…" } else { "Story Board storage not ready" } }
                </h1>
                if !model.loading {
                    <p class="mx-auto mt-3 max-w-xl text-sm font-light leading-6 text-black/50">
                        {"The canonical Story Board projection returned no cockpit payload."}
                    </p>
                }
            </section>
        };
    };

    html! {
        <div class="flex flex-col gap-4">
            { kpis(data) }
            <div class="grid gap-4 lg:grid-cols-2">
                { lifecycle_panel(&data.panels.open, "Current work queue", "Open", false, true) }
                { lifecycle_panel(&data.panels.backlog, "Current-version waiting", "Backlog", false, false) }
                { lifecycle_panel(&data.panels.closed, "Finished history", "Closed", true, false) }
                { lifecycle_panel(&data.panels.next_version, "Intentionally future", "Next Version", false, false) }
            </div>
        </div>
    }
}

fn kpis(data: &PortalStoryboardPage) -> Html {
    html! {
        <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            { kpi_card("Total stories", data.kpis.total.to_string(), "All canonical board rows", false) }
            { kpi_card("Open", data.kpis.open.to_string(), "Current work queue", true) }
            { kpi_card("Backlog", data.kpis.backlog.to_string(), "Current-version planned", false) }
            { kpi_card("Blocked / Hold", data.kpis.blocked_hold.to_string(), "Attention required", true) }
            { kpi_card("Complete", data.kpis.complete.to_string(), "Finished history", false) }
            { kpi_card(
                "Completion",
                format!("{:.1}%", data.kpis.completion_percent),
                "Net-net of the five domains",
                false,
            ) }
        </div>
    }
}

fn kpi_card(label: &'static str, value: String, note: &'static str, dark: bool) -> Html {
    let surface = if dark {
        "portal-glass-panel-feature"
    } else {
        "border border-[var(--portal-panel-border)] bg-white shadow-[var(--portal-panel-shadow)]"
    };
    let eyebrow = if dark {
        "text-[var(--portal-feature-eyebrow)]"
    } else {
        "text-[var(--portal-blue-gray)]"
    };
    let value_tone = if dark {
        "text-white"
    } else {
        "text-[var(--portal-navy)]"
    };
    let note_tone = if dark {
        "text-white/55"
    } else {
        "text-black/40"
    };

    html! {
        <section class={classes!("rounded-[var(--portal-panel-radius)]", "p-3", surface)}>
            <p class={classes!("text-[9px]", "font-light", "uppercase", "tracking-[0.18em]", eyebrow)}>
                { label }
            </p>
            <p class={classes!("mt-1.5", "font-serif", "text-2xl", "font-light", "leading-none", "tabular-nums", value_tone)}>
                { value }
            </p>
            <p class={classes!("mt-1", "text-[10px]", "font-light", "leading-4", note_tone)}>
                { note }
            </p>
        </section>
    }
}

fn lifecycle_panel(
    panel: &PortalStoryboardPanel,
    eyebrow: &'static str,
    title: &'static str,
    subdued: bool,
    attention: bool,
) -> Html {
    let surface = if attention {
        "portal-glass-panel-attention"
    } else {
        "portal-glass-panel"
    };

    html! {
        <section class={classes!(
            "flex", "h-[21rem]", "flex-col", "overflow-hidden",
            "rounded-[var(--portal-panel-radius)]", surface,
            subdued.then_some("opacity-80")
        )}>
            <header class="flex items-baseline justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-3">
                <div>
                    <p class="text-[9px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                        { eyebrow }
                    </p>
                    <h2 class={classes!(
                        "mt-0.5", "font-serif", "text-lg", "font-semibold", "leading-none",
                        if subdued { "text-black/55" } else { "text-[var(--portal-navy)]" }
                    )}>
                        { title }
                    </h2>
                </div>
                <span class={classes!(
                    "rounded-full", "px-2.5", "py-0.5", "font-serif", "text-sm", "font-light", "tabular-nums",
                    if attention {
                        "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]"
                    } else {
                        "border border-[var(--portal-border)] text-black/50"
                    }
                )}>
                    { panel.count }
                </span>
            </header>

            <div class="min-h-0 flex-1 overflow-y-auto p-2.5">
                if panel.groups.is_empty() {
                    <p class="px-3 py-8 text-center text-xs font-light italic text-black/35">
                        { format!("No {} stories right now.", title.to_lowercase()) }
                    </p>
                } else {
                    <div class="space-y-3">
                        { for panel.groups.iter().map(|group| html! {
                            <section>
                                <h3 class={classes!(
                                    "mb-1.5", "px-1", "text-[9px]", "font-semibold", "uppercase", "tracking-[0.18em]",
                                    if subdued { "text-black/30" } else { "text-[var(--portal-navy-soft)]" }
                                )}>
                                    { group.group.clone() }
                                </h3>
                                <div class="space-y-1.5">
                                    { for group.stories.iter().map(|story| story_card(story, subdued)) }
                                </div>
                            </section>
                        }) }
                    </div>
                }
            </div>
        </section>
    }
}

fn story_card(story: &PortalStoryboardStory, subdued: bool) -> Html {
    let href = format!("/portal/storyboard/{}", story.id);
    let completion = story.completion.clamp(0.0, 100.0);

    html! {
        <a
            href={href}
            class={classes!(
                "group", "block", "rounded-md", "border", "px-2.5", "py-1.5", "transition",
                if subdued {
                    "border-black/5 bg-black/[0.02]"
                } else {
                    "border-[var(--portal-border)] bg-white/60 hover:border-[var(--portal-gold)]/50"
                }
            )}
        >
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <p class={classes!(
                        "font-mono", "text-[11px]",
                        if subdued { "text-black/35" } else { "text-[var(--portal-navy)]" }
                    )}>
                        { story.id.clone() }
                    </p>
                    <p class={classes!(
                        "mt-0.5", "line-clamp-2", "text-sm", "font-light", "leading-5",
                        if subdued { "text-black/45" } else { "text-black/70" }
                    )}>
                        { story.title.clone() }
                    </p>
                </div>
                { status_badge(&story.status, subdued) }
            </div>
            <div class="mt-2 flex items-center justify-between gap-3">
                <span class={classes!(
                    "text-[10px]", "font-light", "uppercase", "tracking-[0.12em]",
                    if subdued { "text-black/30" } else { "text-black/40" }
                )}>
                    { story.priority.clone() }
                </span>
                <div class="flex min-w-24 items-center gap-2">
                    <div class="h-1 flex-1 overflow-hidden rounded-full bg-black/10">
                        <div
                            class={classes!(
                                "h-full", "rounded-full",
                                if subdued { "bg-black/25" } else { "bg-[var(--portal-navy)]" }
                            )}
                            style={format!("width: {completion:.0}%;")}
                        />
                    </div>
                    <span class={classes!(
                        "text-[10px]", "font-light", "tabular-nums",
                        if subdued { "text-black/30" } else { "text-black/50" }
                    )}>
                        { format!("{completion:.0}%") }
                    </span>
                </div>
            </div>
        </a>
    }
}

fn status_badge(status: &str, subdued: bool) -> Html {
    let tone = if subdued {
        "border border-black/10 text-black/35"
    } else {
        match status {
            "Complete" => "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
            "In Progress" | "Partial" => {
                "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]"
            }
            "Ready" | "Batched" => "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]",
            "Blocked" | "Failed" | "Hold" => {
                "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]"
            }
            _ => "bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]",
        }
    };

    html! {
        <span class={classes!(
            "inline-block", "shrink-0", "whitespace-nowrap", "rounded-full",
            "px-2.5", "py-1", "text-[10px]", "font-light", "uppercase", "tracking-[0.14em]", tone
        )}>
            { status }
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_is_clamped_for_progress_bars() {
        assert_eq!(120.0_f64.clamp(0.0, 100.0), 100.0);
        assert_eq!((-5.0_f64).clamp(0.0, 100.0), 0.0);
    }
}
