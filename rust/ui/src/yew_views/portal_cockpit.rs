use std::collections::HashSet;

use yew::prelude::*;

use crate::model::{
    Msg, PortalCockpitDeal, PortalCockpitInteraction, PortalCockpitPage, PortalCockpitStageCount,
    PortalCockpitTask,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct CockpitProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Cockpit;

impl Component for Cockpit {
    type Message = ();
    type Properties = CockpitProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("dashboard").expect("dashboard screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { cockpit(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalCockpitPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.cockpit.as_ref())
}

fn cockpit(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">
                    { if model.loading { "Loading Cockpit…" } else { "Cockpit data is not available." } }
                </p>
            </section>
        };
    };

    let attention = combined_tasks(data);
    let today = combined_tasks(data);

    html! {
        <div class="flex flex-col gap-4">
            { kpis(data) }

            <div class="grid gap-4 lg:grid-cols-3">
                { task_panel(
                    "Needs attention",
                    "/portal/attention",
                    &attention,
                    true,
                    model.loading,
                    on_msg,
                ) }
                { task_panel(
                    "Today",
                    "/portal/attention",
                    &today,
                    false,
                    model.loading,
                    on_msg,
                ) }

                <div class="flex flex-col gap-4">
                    { featured_deal(data.featured_deal.as_ref()) }
                    { pipeline(data) }
                </div>
            </div>

            { recent_activity(&data.recent_interactions) }
        </div>
    }
}

fn kpis(data: &PortalCockpitPage) -> Html {
    let values = [
        ("Clients", data.active_client_count, "/portal/clients", false),
        ("Live deals", data.live_deal_count, "/portal/deals", false),
        ("Upcoming", data.upcoming_count, "/portal/attention", false),
        (
            "Under contract",
            data.under_contract_count,
            "/portal/deals",
            false,
        ),
        (
            "In motion",
            data.active_workflow_count,
            "/portal/workflows",
            false,
        ),
        (
            "Blocked",
            data.blocked_workflow_count,
            "/portal/workflows",
            data.blocked_workflow_count > 0,
        ),
    ];

    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-6">
                { for values.into_iter().map(|(label, value, href, alert)| html! {
                    <a href={href} class="px-4 py-3 transition hover:bg-white/25">
                        <div class="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                            { label }
                        </div>
                        <div class={classes!(
                            "mt-1", "font-serif", "text-2xl", "font-light", "leading-none",
                            if alert { "text-[var(--portal-archive)]" } else { "text-[var(--portal-navy)]" }
                        )}>
                            { value }
                        </div>
                    </a>
                }) }
            </div>
        </section>
    }
}

fn combined_tasks(data: &PortalCockpitPage) -> Vec<(&PortalCockpitTask, bool)> {
    let mut seen = HashSet::new();
    data.overdue_tasks
        .iter()
        .map(|task| (task, true))
        .chain(data.tasks_due_soon.iter().map(|task| (task, false)))
        .filter(|(task, _)| seen.insert(task.id.as_str()))
        .take(5)
        .collect()
}

fn task_panel(
    heading: &'static str,
    href: &'static str,
    tasks: &[(&PortalCockpitTask, bool)],
    actions: bool,
    disabled: bool,
    on_msg: &Callback<Msg>,
) -> Html {
    let empty = if actions {
        "Nothing needs attention."
    } else {
        "Clear — nothing on the board today."
    };

    let surface = if actions {
        "portal-glass-panel portal-glass-panel-attention overflow-hidden rounded-[var(--portal-panel-radius)]"
    } else {
        "portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]"
    };
    let heading_tone = if actions {
        "font-serif text-lg font-light text-[var(--portal-attention-heading)]"
    } else {
        "font-serif text-lg font-light text-[var(--portal-panel-heading)]"
    };

    html! {
        <section class={surface}>
            <div class="flex items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <h2 class={heading_tone}>{ heading }</h2>
                <a href={href} class="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]">
                    {"View all →"}
                </a>
            </div>

            if tasks.is_empty() {
                <div class="px-4 py-6 text-sm font-light text-black/40">{ empty }</div>
            } else {
                { for tasks.iter().map(|(task, overdue)| {
                    if actions {
                        attention_row(task, disabled, on_msg)
                    } else {
                        today_row(task, *overdue)
                    }
                }) }
            }
        </section>
    }
}

fn attention_row(task: &PortalCockpitTask, disabled: bool, on_msg: &Callback<Msg>) -> Html {
    let task_id = task.id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::CockpitTaskCompleteRequested {
                task_id: task_id.clone(),
            });
        })
    };
    let context = task.context_name.as_deref().unwrap_or("Task");

    html! {
        <div class="flex items-center gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0">
            <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] text-[10px] font-medium text-[var(--portal-navy-soft)]">
                { initials(context) }
            </div>
            <div class="min-w-0 flex-1">
                if let Some(person_id) = task.person_id.as_deref() {
                    <a
                        href={format!("/portal/clients/{person_id}")}
                        class="block truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                        { task.title.clone() }
                    </a>
                } else {
                    <div class="truncate text-sm font-medium">{ task.title.clone() }</div>
                }
                <div class="truncate text-xs font-light text-black/45">{ context }</div>
            </div>
            <div class="flex shrink-0 items-center gap-2">
                <div class="text-right text-[11px] font-light text-black/40">
                    { task.due_at_label.as_deref().unwrap_or("Unscheduled") }
                </div>
                <button
                    type="button"
                    {onclick}
                    disabled={disabled}
                    class="inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:opacity-40"
                >
                    {"Done"}
                </button>
            </div>
        </div>
    }
}

fn today_row(task: &PortalCockpitTask, overdue: bool) -> Html {
    html! {
        <div class="flex items-start gap-3 border-b border-[var(--portal-border)] px-4 py-2.5 last:border-b-0">
            <div class={classes!(
                "mt-1.5", "h-1.5", "w-1.5", "shrink-0", "rounded-full",
                if overdue { "bg-[var(--portal-archive)]" } else { "bg-[var(--portal-navy)]" }
            )} />
            <div class="min-w-0 flex-1">
                if let Some(person_id) = task.person_id.as_deref() {
                    <a
                        href={format!("/portal/clients/{person_id}")}
                        class="block truncate text-sm font-medium text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                        { task.title.clone() }
                    </a>
                } else {
                    <div class="truncate text-sm font-medium">{ task.title.clone() }</div>
                }
                <div class="truncate text-xs font-light text-black/45">
                    { if overdue { "Overdue · " } else { "" } }
                    { task.due_at_label.as_deref().unwrap_or("Unscheduled") }
                    {
                        task.context_name
                            .as_ref()
                            .map(|context| format!(" · {context}"))
                            .unwrap_or_default()
                    }
                </div>
            </div>
        </div>
    }
}

fn featured_deal(deal: Option<&PortalCockpitDeal>) -> Html {
    let Some(deal) = deal else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                <p class="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                    {"Next closing"}
                </p>
                <p class="mt-2 text-sm font-light text-black/40">{"No active deals."}</p>
            </section>
        };
    };

    let value = deal.offer_price.or(deal.list_price);
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex gap-3 p-4">
                if let Some(media_id) = deal.hero_media_id.as_deref() {
                    <img
                        src={format!("/api/media/{media_id}")}
                        alt={deal.property_name.clone()}
                        class="h-16 w-20 shrink-0 rounded-md object-cover"
                    />
                } else {
                    <div class="h-16 w-20 shrink-0 rounded-md bg-gradient-to-br from-[var(--portal-blue-pale)] to-[var(--portal-navy-soft)]" />
                }
                <div class="min-w-0 flex-1">
                    <p class="text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
                        {"Next closing"}
                    </p>
                    <a
                        href={format!("/portal/deals/{}", deal.id)}
                        class="mt-0.5 block truncate font-serif text-lg font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]"
                    >
                        { deal.property_name.clone() }
                    </a>
                    <p class="truncate text-xs font-light text-black/45">
                        { format!("{} · {}", format_currency(value), stage_label(&deal.stage)) }
                    </p>
                </div>
            </div>
        </section>
    }
}

fn pipeline(data: &PortalCockpitPage) -> Html {
    const STAGES: &[&str] = &[
        "new_lead",
        "qualified",
        "showing",
        "offer",
        "under_contract",
    ];

    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Pipeline"}</h2>
                <a href="/portal/deals" class="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]">
                    {"View all →"}
                </a>
            </div>
            <div class="space-y-2 p-4">
                { for STAGES.iter().map(|stage| {
                    let count = stage_count(&data.pipeline, stage);
                    let percent = if data.live_deal_count > 0 {
                        ((count as f64 / data.live_deal_count as f64) * 100.0).round() as i64
                    } else {
                        0
                    };
                    html! {
                        <div class="flex items-center gap-3">
                            <div class="w-[6.5rem] shrink-0 text-[10px] font-light uppercase tracking-[0.1em] text-black/50">
                                { stage_label(stage) }
                            </div>
                            <div class="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-[var(--portal-blue-pale)]">
                                <div class="h-full bg-[var(--portal-navy)]" style={format!("width: {percent}%")} />
                            </div>
                            <div class="w-4 shrink-0 text-right text-xs font-light tabular-nums text-black/45">
                                { count }
                            </div>
                        </div>
                    }
                }) }
            </div>
        </section>
    }
}

fn recent_activity(interactions: &[PortalCockpitInteraction]) -> Html {
    html! {
        <section class="portal-glass-panel portal-glass-panel-soft overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-4 py-3">
                <h2 class="font-serif text-lg font-light text-[var(--portal-soft-heading)]">{"Recent activity"}</h2>
                <a href="/portal/activity" class="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]">
                    {"View all →"}
                </a>
            </div>
            if interactions.is_empty() {
                <div class="px-4 py-6 text-sm font-light text-black/40">{"No recent relationship activity."}</div>
            } else {
                { for interactions.iter().take(5).map(activity_row) }
            }
        </section>
    }
}

fn activity_row(interaction: &PortalCockpitInteraction) -> Html {
    let detail = interaction
        .summary
        .as_deref()
        .or(interaction.title.as_deref())
        .unwrap_or("Activity recorded");

    html! {
        <div class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5 border-b border-[var(--portal-border)] px-4 py-2 last:border-b-0 sm:grid-cols-[7.5rem_5.5rem_1fr_auto]">
            <div class="truncate text-xs font-light text-black/40">
                { interaction.occurred_at_label.clone() }
            </div>
            <div class="hidden text-[10px] font-light uppercase tracking-[0.12em] text-[var(--portal-blue-gray)] sm:block">
                { channel_label(&interaction.channel) }
            </div>
            <div class="min-w-0 truncate text-sm font-medium">
                { interaction.person_name.clone() }
                <span class="font-light text-black/50">
                    { format!(" — {detail}") }
                </span>
            </div>
        </div>
    }
}

fn stage_count(rows: &[PortalCockpitStageCount], stage: &str) -> i64 {
    rows.iter()
        .find(|row| row.stage == stage)
        .map(|row| row.count)
        .unwrap_or(0)
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "new_lead" => "New Lead",
        "qualified" => "Qualified",
        "showing" => "Showing",
        "offer" => "Offer",
        "under_contract" => "Under Contract",
        "closed" => "Closed",
        _ => "Unknown",
    }
}

fn channel_label(channel: &str) -> &'static str {
    match channel {
        "website" => "Website",
        "calendar" => "Calendar",
        "document" => "Document",
        "manual" => "Manual Entry",
        "imessage" => "iMessage",
        "sms" => "SMS",
        "email" => "Email",
        "call" => "Phone Call",
        "meeting" => "Meeting",
        "showing" => "Showing",
        "whatsapp" => "WhatsApp",
        _ => "Note",
    }
}

fn initials(value: &str) -> String {
    value
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect()
}

fn format_currency(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let negative = value < 0.0;
    let digits = format!("{:.0}", value.abs());
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    let number: String = grouped.chars().rev().collect();
    format!("{}{}{}", if negative { "-" } else { "" }, "$", number)
}
