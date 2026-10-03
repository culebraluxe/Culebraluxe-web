//! The deal's tasks and its activity.

#[allow(unused_imports)]
use super::*;

pub(super) fn tasks_card(
    model: &Vm<'_>,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let title_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskTitleChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let detail_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskDetailChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let due_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceTaskDueChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCreateTaskRequested))
    };

    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Open Tasks"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Milestones and follow-ups on this deal."}</p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.open_tasks.len() }</span>
            </div>
            if workspace.open_tasks.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No open tasks on this deal."}</p>
            } else {
                <div>
                    { for workspace.open_tasks.iter().map(|task| {
                        let task_id = task.id.clone();
                        let complete = {
                            let on_msg = on_msg.clone();
                            Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCompleteTaskRequested { task_id: task_id.clone() }))
                        };
                        html! {
                            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
                                <div class="flex items-start justify-between gap-3">
                                    <div>
                                        <div class="font-serif text-lg font-light text-[var(--portal-navy)]">{ task.title.clone() }</div>
                                        if let Some(detail) = task.detail.as_ref() {
                                            <p class="mt-1 text-sm font-light text-black/50">{ detail.clone() }</p>
                                        }
                                        <p class="mt-2 text-xs font-light text-black/40">{ task.due_at_label.clone().unwrap_or_else(|| "Unscheduled".into()) }</p>
                                    </div>
                                    if task.is_overdue {
                                        <span class="rounded-full bg-[var(--portal-archive-pale)] px-2 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-archive)]">{"Overdue"}</span>
                                    }
                                </div>
                                <button type="button" onclick={complete} disabled={busy}
                                    class="mt-3 min-h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-35">
                                    {"Complete"}
                                </button>
                            </div>
                        }
                    }) }
                </div>
            }
            <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
                <input value={model.deal_workspace.task_title.clone()} oninput={title_change} placeholder="New task title…" class={field_class()} />
                <input value={model.deal_workspace.task_detail.clone()} oninput={detail_change} placeholder="Detail (optional)…" class={field_class()} />
                <input type="datetime-local" value={model.deal_workspace.task_due_at.clone()} oninput={due_change} class={field_class()} />
                <button type="button" onclick={create} disabled={busy || model.deal_workspace.task_title.trim().is_empty()}
                    class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                    {"Add task"}
                </button>
            </div>
        </section>
    }
}

pub(super) fn activity_card(workspace: &crate::model::PortalDealWorkspace) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Recent Contract Activity"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Interactions tied to this contract."}</p>
                </div>
                <span class="text-xs font-light text-black/35">{ workspace.activity.len() }</span>
            </div>
            if workspace.activity.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No deal activity yet."}</p>
            } else {
                <div>
                    { for workspace.activity.iter().map(|item| html! {
                        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
                            <div class="flex flex-wrap items-center gap-2 text-[10px] font-light uppercase tracking-[0.11em] text-[var(--portal-navy-soft)]">
                                <span>{ channel_label(&item.channel) }</span>
                                if let Some(direction) = item.direction.as_ref() {
                                    <span class="text-black/30">{ direction.clone() }</span>
                                }
                                <span class="ml-auto normal-case tracking-normal text-black/35">{ item.occurred_at_label.clone() }</span>
                            </div>
                            <div class="mt-1 text-sm font-medium text-[var(--portal-navy)]">
                                { item.person_name.clone().unwrap_or_else(|| "—".into()) }
                            </div>
                            <p class="mt-1 text-sm font-light text-black/55">
                                { item.summary.clone().or(item.title.clone()).unwrap_or_else(|| "Interaction".into()) }
                            </p>
                        </div>
                    }) }
                </div>
            }
        </section>
    }
}
