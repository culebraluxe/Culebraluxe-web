//! CORE Catch-Up — the relationship queue that answers "who needs me now, and why?"
//!
//! The server derives every row from canonical Person/interaction/showing/deal/task facts. This screen owns only
//! presentation and intent: selection, Handle, Snooze, and native phone/message/email links.

use yew::prelude::*;

use crate::app::api::{CatchUpAction, CatchUpRead};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalCatchUpItem, PortalCatchUpPage, PortalPage};

pub struct CatchUp;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalCatchUpPage>,
    pub selected_person_id: Option<String>,
    pub busy: bool,
    pub notice: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    Selected(String),
    HandleRequested { person_id: String, reason_code: String },
    SnoozeRequested { person_id: String, reason_code: String, days: i32 },
    ActionAnswered(Result<PortalPage, ApiError>),
}

impl Screen for CatchUp {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(CatchUpRead, Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => apply(model, answer, false),
            Msg::Selected(person_id) => {
                model.selected_person_id = Some(person_id);
                Cmd::none()
            }
            Msg::HandleRequested { person_id, reason_code } => {
                if model.busy {
                    return Cmd::none();
                }
                model.busy = true;
                model.notice = None;
                Cmd::request(
                    CatchUpAction::handle(person_id, reason_code),
                    Msg::ActionAnswered,
                )
            }
            Msg::SnoozeRequested {
                person_id,
                reason_code,
                days,
            } => {
                if model.busy {
                    return Cmd::none();
                }
                model.busy = true;
                model.notice = None;
                Cmd::request(
                    CatchUpAction::snooze(person_id, reason_code, days),
                    Msg::ActionAnswered,
                )
            }
            Msg::ActionAnswered(answer) => {
                model.busy = false;
                apply(model, answer, true)
            }
        }
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        html! {
            <div class="space-y-6">
                { template::portal_heading(
                    "Core",
                    "Catch-Up",
                    "Who needs attention now, and why — derived from relationship activity, showings, open follow-ups and active work.",
                ) }
                if let Some(notice) = &model.notice {
                    <p class="text-xs font-light text-[var(--portal-navy-soft)]" role="status">{ notice.clone() }</p>
                }
                { template::remote(&model.read, "Catch-Up", |page| workspace(page, model, &on_msg)) }
            </div>
        }
    }
}

fn apply(model: &mut Model, answer: Result<PortalPage, ApiError>, acted: bool) -> Cmd<Msg> {
    match answer.and_then(|page| {
        page.catch_up
            .ok_or_else(|| ApiError::decode("The answer had no Catch-Up queue in it."))
    }) {
        Ok(page) => {
            let selected_still_exists = model
                .selected_person_id
                .as_ref()
                .is_some_and(|id| page.items.iter().any(|item| &item.person_id == id));
            if !selected_still_exists {
                model.selected_person_id = page.items.first().map(|item| item.person_id.clone());
            }
            model.read = Remote::Loaded(page);
            if acted {
                model.notice = Some("Catch-Up updated.".into());
            }
        }
        Err(error) => {
            if acted {
                model.notice = Some(error.message.clone());
            }
            model.read = Remote::Failed(error);
        }
    }
    Cmd::none()
}

fn workspace(page: &PortalCatchUpPage, model: &Model, on_msg: &Callback<Msg>) -> Html {
    if page.items.is_empty() {
        return html! {
            <>
                <section class="grid gap-4 sm:grid-cols-2">
                    { template::metric("Needs you", "0", "No relationship signal currently needs action.") }
                    { template::metric("High priority", "0", "No unanswered inbound, overdue follow-up or post-showing gap.") }
                </section>
                { template::empty_panel("Caught up. New relationship signals will appear here automatically.") }
            </>
        };
    }

    let selected = selected(page, model.selected_person_id.as_deref());
    html! {
        <>
            <section class="grid gap-4 sm:grid-cols-2">
                { template::metric("Needs you", &page.total.to_string(), "One row per person, highest-priority reason first.") }
                { template::metric("High priority", &page.high_priority_count.to_string(), "Unanswered inbound, showing follow-up and overdue work.") }
            </section>
            <section class="grid min-h-[560px] overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel lg:grid-cols-[minmax(320px,0.9fr)_minmax(0,1.35fr)]">
                <div class="border-b border-[var(--portal-border)] lg:border-b-0 lg:border-r">
                    <div class="border-b border-[var(--portal-border)] px-5 py-4">
                        <h2 class="font-serif text-xl font-light">{"Needs you"}</h2>
                        <p class="mt-1 text-xs font-light text-black/40">{"The reason is explicit; no hidden lead score."}</p>
                    </div>
                    <div class="max-h-[650px] overflow-y-auto">
                        { for page.items.iter().map(|item| queue_row(item, selected.map(|row| row.person_id.as_str()), on_msg)) }
                    </div>
                </div>
                <div class="p-6">
                    { selected.map(|item| detail(item, model.busy, on_msg)).unwrap_or_else(|| template::empty_panel("Choose a relationship.")) }
                </div>
            </section>
        </>
    }
}

fn selected<'a>(page: &'a PortalCatchUpPage, wanted: Option<&str>) -> Option<&'a PortalCatchUpItem> {
    wanted
        .and_then(|id| page.items.iter().find(|item| item.person_id == id))
        .or_else(|| page.items.first())
}

fn queue_row(item: &PortalCatchUpItem, selected: Option<&str>, on_msg: &Callback<Msg>) -> Html {
    let id = item.person_id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::Selected(id.clone())))
    };
    let active = selected == Some(item.person_id.as_str());
    html! {
        <button type="button" {onclick}
            class={classes!(
                "block", "w-full", "border-b", "border-[var(--portal-border)]", "px-5", "py-4", "text-left", "transition",
                if active { "bg-white/45" } else { "hover:bg-white/25" }
            )}>
            <div class="flex items-center justify-between gap-3">
                <span class="truncate font-serif text-lg font-light text-[var(--portal-navy)]">{ item.display_name.clone() }</span>
                <span class={classes!(
                    "shrink-0", "rounded-full", "px-2", "py-1", "text-[9px]", "font-medium", "uppercase", "tracking-[0.12em]",
                    if item.priority >= 85 { "bg-red-100 text-red-800" } else if item.priority >= 70 { "bg-amber-100 text-amber-800" } else { "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]" }
                )}>
                    { priority_label(item.priority) }
                </span>
            </div>
            <p class="mt-1 line-clamp-2 text-xs font-light leading-5 text-black/55">{ item.reason.clone() }</p>
            <p class="mt-2 text-[10px] font-light uppercase tracking-[0.12em] text-black/35">{ item.signal_at_label.clone() }</p>
        </button>
    }
}

fn detail(item: &PortalCatchUpItem, busy: bool, on_msg: &Callback<Msg>) -> Html {
    let handle = {
        let on_msg = on_msg.clone();
        let person_id = item.person_id.clone();
        let reason_code = item.reason_code.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::HandleRequested {
            person_id: person_id.clone(),
            reason_code: reason_code.clone(),
        }))
    };
    html! {
        <div class="space-y-6">
            <div>
                <p class="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">{ format!("{} · {}", item.role, item.status) }</p>
                <h2 class="mt-2 font-serif text-3xl font-light">{ item.display_name.clone() }</h2>
                <p class="mt-3 max-w-2xl text-sm font-light leading-6 text-black/60">{ item.reason.clone() }</p>
            </div>

            <div class="grid gap-3 sm:grid-cols-2">
                { fact("Last contact", item.last_contact_label.as_deref().unwrap_or("No recorded contact")) }
                { fact("Channel", item.last_contact_channel.as_deref().unwrap_or("—")) }
                { fact("Direction", item.last_contact_direction.as_deref().unwrap_or("—")) }
                { fact("Next follow-up", item.due_at_label.as_deref().unwrap_or("No dated task")) }
            </div>

            if let Some(summary) = &item.last_contact_summary {
                <section class="rounded-[var(--portal-panel-radius)] bg-white/35 p-4">
                    <p class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">{"Last context"}</p>
                    <p class="mt-2 text-sm font-light leading-6 text-black/65">{ summary.clone() }</p>
                </section>
            }

            if let Some(property) = &item.active_property_name {
                <section class="rounded-[var(--portal-panel-radius)] bg-white/35 p-4">
                    <p class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">{"Active work"}</p>
                    if let Some(deal_id) = &item.active_deal_id {
                        <a href={format!("/portal/deals/{deal_id}")} class="mt-2 inline-flex min-h-11 items-center font-serif text-lg font-light text-[var(--portal-navy)] hover:text-[var(--portal-navy-soft)]">
                            { property.clone() }
                        </a>
                    } else {
                        <p class="mt-2 font-serif text-lg font-light">{ property.clone() }</p>
                    }
                </section>
            }

            <div class="flex flex-wrap gap-2">
                <a href={format!("/portal/clients/{}", item.person_id)} class={action_class(false)}>{"Open client"}</a>
                if let Some(phone) = item.primary_phone.as_deref() {
                    <a href={format!("tel:{phone}")} class={action_class(false)}>{"Call"}</a>
                    <a href={format!("sms:{phone}")} class={action_class(false)}>{"iMessage"}</a>
                }
                if let Some(email) = item.primary_email.as_deref() {
                    <a href={format!("mailto:{email}")} class={action_class(false)}>{"Email"}</a>
                }
            </div>

            <div class="border-t border-[var(--portal-border)] pt-5">
                <p class="mb-3 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">{"Disposition"}</p>
                <div class="flex flex-wrap gap-2">
                    <button type="button" onclick={handle} disabled={busy} class={action_class(true)}>{"Handled"}</button>
                    { for [1, 3, 7].into_iter().map(|days| snooze_button(item, days, busy, on_msg)) }
                </div>
            </div>
        </div>
    }
}

fn snooze_button(item: &PortalCatchUpItem, days: i32, busy: bool, on_msg: &Callback<Msg>) -> Html {
    let person_id = item.person_id.clone();
    let reason_code = item.reason_code.clone();
    let on_msg = on_msg.clone();
    let onclick = Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SnoozeRequested {
        person_id: person_id.clone(),
        reason_code: reason_code.clone(),
        days,
    }));
    html! {
        <button type="button" {onclick} disabled={busy} class={action_class(false)}>
            { format!("Snooze {days}d") }
        </button>
    }
}

fn fact(label: &str, value: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] bg-white/30 p-4">
            <p class="text-[9px] font-light uppercase tracking-[0.16em] text-black/35">{ label.to_owned() }</p>
            <p class="mt-1 text-sm font-light text-black/65">{ value.to_owned() }</p>
        </div>
    }
}

fn action_class(primary: bool) -> Classes {
    classes!(
        "inline-flex", "min-h-10", "items-center", "rounded-[var(--portal-tab-radius)]", "px-3", "text-[10px]",
        "font-medium", "uppercase", "tracking-[0.12em]", "transition", "disabled:opacity-40",
        if primary { "bg-[var(--portal-navy)] text-white hover:bg-[var(--portal-navy-soft)]" } else { "border border-[var(--portal-border)] bg-white/35 text-[var(--portal-navy)] hover:bg-white/60" }
    )
}

fn priority_label(priority: i32) -> &'static str {
    if priority >= 85 {
        "Now"
    } else if priority >= 70 {
        "Soon"
    } else {
        "Quiet"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::screen::ScreenCtx;
    use serde_json::json;

    #[test]
    fn it_reads_the_queue_selects_a_person_and_posts_disposition_intents() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = CatchUp::init(&ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/catch-up");

        let answer = json!({ "catchUp": {
            "generatedAt": "2026-09-28T12:00:00Z",
            "total": 1,
            "highPriorityCount": 1,
            "items": [{
                "personId": "p1",
                "displayName": "Alicia Rivera",
                "role": "buyer",
                "status": "active",
                "reasonCode": "unanswered_inbound",
                "reason": "An inbound message has no later outgoing reply.",
                "priority": 100,
                "signalAt": "2026-09-28T11:00:00Z",
                "signalAtLabel": "Sep 28, 2026 07:00 AM"
            }]
        }});
        CatchUp::update(&mut model, request.respond(Ok(answer)), &ctx);
        assert_eq!(model.selected_person_id.as_deref(), Some("p1"));

        let cmd = CatchUp::update(&mut model, Msg::SnoozeRequested {
            person_id: "p1".into(),
            reason_code: "unanswered_inbound".into(),
            days: 3,
        }, &ctx);
        let request = cmd.into_requests().remove(0);
        assert_eq!(request.path, "/api/portal/rust-ui/catch-up");
        assert_eq!(request.body.unwrap()["days"], 3);
    }
}
