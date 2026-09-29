//! The selected-work editor under every view: title, status, dates, owner, notes, and save.

use super::*;

pub(super) fn selected_work_editor(
    _model: &Vm<'_>,
    projects: &PortalProjectsPage,
    item: Option<&PortalProjectWorkItem>,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(item) = item else {
        return html! {
            <section class="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
                <div class="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left">
                    <span class="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                        {"Selected work"}
                    </span>
                    <span class="min-w-0 flex-1 truncate text-[13px] font-light text-black/45">
                        {"Select a work item to edit it."}
                    </span>
                </div>
            </section>
        };
    };

    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectWorkCollapsedToggled))
    };
    let title_change = input_msg(on_msg, MsgKind::Title);
    let owner_change = input_msg(on_msg, MsgKind::Owner);
    let due_change = input_msg(on_msg, MsgKind::Due);
    let planned_start_change = input_msg(on_msg, MsgKind::PlannedStart);
    let planned_finish_change = input_msg(on_msg, MsgKind::PlannedFinish);
    let notes_change = textarea_msg(on_msg);
    let status_change = select_status_msg(on_msg);
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ProjectWorkSaveRequested))
    };
    let summary = format!(
        "{} · {}",
        status_label(&item.status),
        due_label(item.due_at.as_deref())
    );

    html! {
        <section class="shrink-0 overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-soft-bg)] shadow-sm">
            <button
                type="button"
                onclick={toggle}
                aria-expanded={(!projects.work_collapsed).to_string()}
                class="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left"
            >
                <span class="shrink-0 text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
                    {"Selected work"}
                </span>
                <span class="min-w-0 flex-1 truncate text-[15px] font-medium text-[var(--portal-navy)]">
                    { item.title.clone() }
                </span>
                <span class="hidden shrink-0 text-[12px] font-light text-[var(--portal-blue-gray)] sm:inline">
                    { summary }
                </span>
                <span class="flex shrink-0 items-center gap-1 text-[11px] font-medium text-[var(--portal-blue-gray)]">
                    { if projects.work_collapsed { "Expand" } else { "Collapse" } }
                    <span class="text-base leading-none" aria-hidden="true">
                        { if projects.work_collapsed { "⌄" } else { "⌃" } }
                    </span>
                </span>
            </button>
            if !projects.work_collapsed {
                <div class="grid grid-cols-2 gap-2 border-t border-[var(--portal-panel-border)] px-3 pb-3 pt-2 md:grid-cols-3 xl:grid-cols-4 xl:items-end">
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Title"}
                        <input value={item.title.clone()} oninput={title_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Status"}
                        <select value={item.status.clone()} onchange={status_change} class={work_input_class()}>
                            <option value="open" selected={item.status == "open"}>{"Not started"}</option>
                            <option value="doing" selected={item.status == "doing"}>{"In progress"}</option>
                            <option value="done" selected={item.status == "done"}>{"Complete"}</option>
                            <option value="dismissed" selected={item.status == "dismissed"}>{"Dismissed"}</option>
                        </select>
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Due"}
                        <input type="date" value={date_value(item.due_at.as_deref())} oninput={due_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Planned start"}
                        <input type="date" value={date_value(item.planned_start.as_deref())} oninput={planned_start_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Planned finish"}
                        <input type="date" value={date_value(item.planned_finish.as_deref())} oninput={planned_finish_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Owner"}
                        <input value={item.owner.clone().unwrap_or_default()} oninput={owner_change} class={work_input_class()} />
                    </label>
                    <label class="block min-w-0 text-[10px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">
                        {"Notes"}
                        <textarea
                            value={item.notes.clone()}
                            oninput={notes_change}
                            rows="2"
                            class="mt-1 min-h-[3.5rem] w-full resize-y rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 py-2 text-[12px] font-light leading-snug text-black/70 outline-none"
                        />
                    </label>
                    <button
                        type="button"
                        onclick={save}
                        disabled={!projects.work_dirty || projects.saving}
                        class="h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90 disabled:opacity-35"
                    >
                        { if projects.saving { "Saving…" } else { "Save" } }
                    </button>
                </div>
                if projects.active_view == "timeline" {
                    <div class="flex flex-wrap items-end gap-2 border-t border-[var(--portal-panel-border)] px-3 py-2 text-xs text-[var(--portal-blue-gray)]">
                        <label class="min-w-[210px] flex-1">
                            <span class="block text-[10px] font-semibold uppercase tracking-[0.1em]">{"Add predecessor (finish to start)"}</span>
                            <select value={projects.timeline_link_target_id.clone().unwrap_or_default()}
                                onchange={on_msg.reform(|event: Event| Msg::ProjectTimelineLinkTargetSelected(crate::app::exec::select_value(&event)))}
                                class={work_input_class()}>
                                <option value="">{"Choose work item"}</option>
                            { for projects.items.iter().filter(|candidate| candidate.project_id.as_deref() == item.project_id.as_deref()
                                && candidate.id != item.id && !projects.dependencies.iter().any(|edge| edge.source_id == candidate.id && edge.target_id == item.id))
                                    .map(|candidate| html! { <option value={candidate.id.clone()}>{candidate.title.clone()}</option> }) }
                            </select>
                        </label>
                        <button type="button" disabled={projects.saving || projects.timeline_link_target_id.is_none()}
                            onclick={on_msg.reform(|_: MouseEvent| Msg::ProjectTimelineLinkAddRequested)}
                            class="h-9 rounded bg-[var(--portal-navy)] px-3 text-white disabled:opacity-35">{"Add link"}</button>
                        <div class="flex-1" aria-label="Predecessors">
                            <span class="block text-[10px] font-semibold uppercase tracking-[0.1em]">{"Predecessors"}</span>
                            { for projects.dependencies.iter().filter(|edge| edge.target_id == item.id && Some(edge.project_id.as_str()) == item.project_id.as_deref())
                                .map(|edge| {
                                    let source = projects.items.iter().find(|candidate| candidate.id == edge.source_id);
                                    let name = source.map(|candidate| candidate.title.as_str()).unwrap_or("Work item");
                                    let broken = source.is_some_and(|source| crate::timeline::items_link_broken(source, item));
                                    let source_id = edge.source_id.clone();
                                    html! { <span class={classes!("mr-2", "inline-flex", "items-center", "gap-1", "rounded", "border", "px-2", "py-1",
                                            if broken { "border-red-400 bg-red-50 text-red-700" } else { "border-[var(--portal-panel-border)]" })}
                                        title={if broken { format!("Starts before {name} finishes") } else { String::new() }}>
                                        { if broken { format!("⚠ {name} — starts before it finishes") } else { name.to_owned() } }
                                        <button type="button" aria-label={format!("Remove dependency from {name}")}
                                            disabled={projects.saving}
                                            onclick={on_msg.reform(move |_: MouseEvent| Msg::ProjectTimelineLinkRemoveRequested(source_id.clone()))}
                                            class="rounded px-1 text-[var(--portal-gold-muted)] hover:bg-white">{"×"}</button>
                                    </span> }
                                }) }
                        </div>
                    </div>
                }
            }
        </section>
    }
}


#[derive(Clone, Copy)]
pub(super) enum MsgKind {
    Title,
    Owner,
    Due,
    PlannedStart,
    PlannedFinish,
}

pub(super) fn input_msg(on_msg: &Callback<Msg>, kind: MsgKind) -> Callback<InputEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: InputEvent| {
        let value = crate::app::exec::input_value(&event);
        on_msg.emit(match kind {
            MsgKind::Title => Msg::ProjectWorkTitleChanged(value),
            MsgKind::Owner => Msg::ProjectWorkOwnerChanged(value),
            MsgKind::Due => Msg::ProjectWorkDueChanged(value),
            MsgKind::PlannedStart => Msg::ProjectWorkPlannedStartChanged(value),
            MsgKind::PlannedFinish => Msg::ProjectWorkPlannedFinishChanged(value),
        });
    })
}

pub(super) fn textarea_msg(on_msg: &Callback<Msg>) -> Callback<InputEvent> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: InputEvent| {
        let value = crate::app::exec::textarea_value(&event);
        on_msg.emit(Msg::ProjectWorkNotesChanged(value));
    })
}

pub(super) fn select_status_msg(on_msg: &Callback<Msg>) -> Callback<Event> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: Event| {
        let value = crate::app::exec::select_value(&event);
        on_msg.emit(Msg::ProjectWorkStatusChanged(value));
    })
}

pub(super) fn work_input_class() -> Classes {
    classes!(
        "mt-1",
        "block",
        "h-8",
        "w-full",
        "rounded-[var(--portal-tab-radius)]",
        "border",
        "border-[var(--portal-panel-border)]",
        "bg-white/70",
        "px-2.5",
        "text-[12px]",
        "font-light",
        "text-black/70",
        "outline-none"
    )
}
