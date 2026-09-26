//! CORE — the Cockpit (`/portal/dashboard`) and its "View all" list, Attention (`/portal/attention`).
//!
//! Both read the one Cockpit projection and share one command, marking a task done; its answer is the refreshed Cockpit,
//! so the finished task leaves the list in the same step. One command at a time, and a refusal is said above the lists
//! in the service's own words rather than blanking the page.

use std::collections::HashSet;

use yew::prelude::*;

use crate::app::api::{CockpitCompleteTask, CockpitRead};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{
    CommandNotice, PortalCockpitDeal, PortalCockpitInteraction, PortalCockpitPage,
    PortalCockpitStageCount, PortalCockpitTask, PortalPage,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalCockpitPage>,
    /// A task is being marked done; every Done button waits for the answer.
    pub busy: bool,
    pub notice: Option<CommandNotice>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    CompleteRequested { task_id: String },
    Completed(Result<PortalPage, ApiError>),
}

fn init() -> (Model, Cmd<Msg>) {
    (
        Model {
            read: Remote::Loading,
            ..Model::default()
        },
        Cmd::request(CockpitRead, Msg::Loaded),
    )
}

fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Loaded(answer) => model.read = Remote::from_result(answer.and_then(cockpit_of)),
        Msg::CompleteRequested { task_id } => {
            if model.busy || task_id.trim().is_empty() || model.read.loaded().is_none() {
                return Cmd::none();
            }
            model.busy = true;
            model.notice = None;
            return Cmd::request(CockpitCompleteTask { task_id }, Msg::Completed);
        }
        Msg::Completed(answer) => {
            model.busy = false;
            match answer.and_then(cockpit_of) {
                Ok(cockpit) => {
                    model.read = Remote::Loaded(cockpit);
                    model.notice = Some(CommandNotice::success("Marked done."));
                }
                Err(error) => model.notice = Some(CommandNotice::failure(error.message)),
            }
        }
    }
    Cmd::none()
}

fn cockpit_of(page: PortalPage) -> Result<PortalCockpitPage, ApiError> {
    page.cockpit
        .ok_or_else(|| ApiError::decode("The answer had no Cockpit in it."))
}

fn notice(model: &Model) -> Html {
    match &model.notice {
        Some(notice) => html! {
            <p class={classes!("text-xs", "font-light", if notice.ok { "text-emerald-700" } else { "text-red-700" })} role="status">
                { notice.message.clone() }
            </p>
        },
        None => Html::default(),
    }
}

pub struct Cockpit;

impl Screen for Cockpit {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        init()
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg)
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        html! {
            <div class="flex flex-col gap-4">
                { notice(model) }
                { template::remote(&model.read, "the Cockpit", |data| cockpit(data, model.busy, &on_msg)) }
            </div>
        }
    }
}

/// Every task that needs attention: overdue first, then due soon — the list the Cockpit's panel shows five of.
pub struct Attention;

impl Screen for Attention {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        init()
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        update(model, msg)
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        html! {
            <div class="space-y-6">
                <div>{ template::back_link(ctx) }</div>
                { template::portal_heading(
                    "Cockpit",
                    "Needs attention",
                    "Every open task that is overdue or due soon, overdue first. Mark one done and it leaves the list.",
                ) }
                { notice(model) }
                { template::remote(&model.read, "the tasks", |data| {
                    let tasks = combined_tasks(data, usize::MAX);
                    task_panel("Needs attention", None, &tasks, true, model.busy, &on_msg)
                }) }
            </div>
        }
    }
}

fn cockpit(data: &PortalCockpitPage, busy: bool, on_msg: &Callback<Msg>) -> Html {
    let attention = combined_tasks(data, 5);
    let today = combined_tasks(data, 5);

    html! {
        <div class="flex flex-col gap-4">
            { kpis(data) }

            <div class="grid gap-4 lg:grid-cols-3">
                { task_panel(
                    "Needs attention",
                    Some("/portal/attention"),
                    &attention,
                    true,
                    busy,
                    on_msg,
                ) }
                { task_panel(
                    "Today",
                    Some("/portal/attention"),
                    &today,
                    false,
                    busy,
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
        (
            "Clients",
            data.active_client_count,
            "/portal/clients",
            false,
        ),
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

fn combined_tasks(data: &PortalCockpitPage, limit: usize) -> Vec<(&PortalCockpitTask, bool)> {
    let mut seen = HashSet::new();
    data.overdue_tasks
        .iter()
        .map(|task| (task, true))
        .chain(data.tasks_due_soon.iter().map(|task| (task, false)))
        .filter(|(task, _)| seen.insert(task.id.as_str()))
        .take(limit)
        .collect()
}

fn task_panel(
    heading: &'static str,
    href: Option<&'static str>,
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
                if let Some(href) = href {
                    <a href={href} class="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]">
                        {"View all →"}
                    </a>
                }
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
            on_msg.emit(Msg::CompleteRequested {
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
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)] p-4">
            <div class="mb-3 flex items-center justify-between gap-3">
                <h2 class="font-serif text-lg font-light text-[var(--portal-panel-heading)]">{"Pipeline"}</h2>
                <a href="/portal/deals" class="text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]">
                    {"View all →"}
                </a>
            </div>
            <div class="space-y-2">
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
        .unwrap_or("");

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
    if value == 0.0 {
        return "—".into();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task(id: &str) -> serde_json::Value {
        json!({ "id": id, "title": "Call back" })
    }

    #[test]
    fn a_task_is_marked_done_once_and_the_answer_is_the_new_cockpit() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = Attention::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/cockpit");
        Attention::update(
            &mut model,
            request.respond(Ok(json!({ "cockpit": { "overdueTasks": [task("t1")], "tasksDueSoon": [task("t1"), task("t2")] } }))),
            &ctx,
        );
        let tasks = combined_tasks(model.read.loaded().unwrap(), usize::MAX);
        assert_eq!(tasks.len(), 2, "a task in both lists is listed once");

        let request = Attention::update(
            &mut model,
            Msg::CompleteRequested {
                task_id: "t1".into(),
            },
            &ctx,
        )
        .into_requests()
        .remove(0);
        assert_eq!(
            request.body,
            Some(json!({ "action": "completeTask", "taskId": "t1" }))
        );
        assert!(
            Attention::update(
                &mut model,
                Msg::CompleteRequested {
                    task_id: "t2".into()
                },
                &ctx
            )
            .into_requests()
            .is_empty(),
            "one at a time"
        );
        Attention::update(
            &mut model,
            request.respond(Ok(json!({ "cockpit": { "tasksDueSoon": [task("t2")] } }))),
            &ctx,
        );
        assert!(!model.busy);
        assert_eq!(
            combined_tasks(model.read.loaded().unwrap(), usize::MAX).len(),
            1
        );

        Attention::update(
            &mut model,
            Msg::Completed(Err(ApiError::network("Task is closed."))),
            &ctx,
        );
        assert_eq!(
            model.notice,
            Some(CommandNotice::failure("Task is closed."))
        );
        assert!(model.read.loaded().is_some(), "a refusal keeps the list");
    }
}
