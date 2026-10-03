//! Showings: the card, each showing and its dates.

#[allow(unused_imports)]
use super::*;

pub(super) fn showings_card(
    model: &Vm<'_>,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let create = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceCreateShowingRequested))
    };
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--portal-panel-border)] px-5 py-4">
                <div>
                    <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Showings"}</h2>
                    <p class="mt-1 text-xs font-light text-black/40">{"Showing history for this deal."}</p>
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-xs font-light text-black/35">{ workspace.showings.len() }</span>
                    if workspace.client.is_some() {
                        <button type="button" onclick={create} disabled={busy} class={small_action_class()}>{"Request showing"}</button>
                    }
                </div>
            </div>
            if workspace.showings.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No showings on record for this deal."}</p>
            } else {
                <div>
                    { for workspace.showings.iter().map(|showing| showing_row(model, showing, on_msg, busy)) }
                </div>
            }
        </section>
    }
}

pub(super) fn showing_row(
    model: &Vm<'_>,
    showing: &crate::model::PortalDealWorkspaceShowing,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let id = showing.id.clone();
    let time = model
        .deal_workspace
        .showing_times
        .get(&id)
        .cloned()
        .unwrap_or_default();
    let time_change = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceShowingTimeChanged {
                showing_id: id.clone(),
                value: crate::app::exec::input_value(&event),
            })
        })
    };
    let schedule = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceScheduleShowingRequested {
                showing_id: id.clone(),
            })
        })
    };
    let cancel = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceCancelShowingRequested {
                showing_id: id.clone(),
            })
        })
    };
    let complete = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceCompleteShowingRequested {
                showing_id: id.clone(),
            })
        })
    };
    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex flex-wrap items-center gap-2">
                <a href={format!("/portal/clients/{}", showing.person_id)}
                    class="font-serif text-lg font-light text-[var(--portal-navy)]">{ showing.person_name.clone() }</a>
                <span class="rounded-full bg-[var(--portal-blue-pale)] px-2.5 py-1 text-[9px] uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">{ title_case(&showing.status) }</span>
            </div>
            <p class="mt-2 text-xs font-light text-black/45">
                { format_showing_dates(showing) }
            </p>
            if let Some(feedback) = showing.feedback.as_ref() {
                <p class="mt-2 text-sm font-light text-black/55">{ feedback.clone() }</p>
            }
            if showing.status == "requested" {
                <div class="mt-3 flex flex-wrap items-center gap-2">
                    <input type="datetime-local" value={time} oninput={time_change}
                        class="h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2 text-xs font-light outline-none focus:border-[var(--portal-navy)]" />
                    <button type="button" onclick={schedule} disabled={busy} class={small_action_class()}>{"Schedule"}</button>
                    <button type="button" onclick={complete.clone()} disabled={busy} class={small_action_class()}>{"Complete"}</button>
                    <button type="button" onclick={cancel.clone()} disabled={busy} class={small_action_class()}>{"Cancel"}</button>
                </div>
            } else if showing.status == "scheduled" {
                <div class="mt-3 flex flex-wrap gap-2">
                    <button type="button" onclick={complete} disabled={busy} class={small_action_class()}>{"Complete"}</button>
                    <button type="button" onclick={cancel} disabled={busy} class={small_action_class()}>{"Cancel"}</button>
                </div>
            }
        </div>
    }
}

pub(super) fn property_descriptor(property: &crate::model::PortalDealWorkspaceProperty) -> String {
    let mut parts = Vec::new();
    if let Some(kind) = property
        .property_type
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(kind.clone());
    }
    if let Some(bedrooms) = property.bedrooms {
        parts.push(format!("{} bed", format_number(bedrooms)));
    }
    if let Some(bathrooms) = property.bathrooms {
        parts.push(format!("{} bath", format_number(bathrooms)));
    }
    if let Some(square_feet) = property.square_feet {
        parts.push(format!("{} SF", group_integer(square_feet)));
    }
    if parts.is_empty() {
        "No details on file".into()
    } else {
        parts.join(" · ")
    }
}

pub(super) fn format_showing_dates(showing: &crate::model::PortalDealWorkspaceShowing) -> String {
    let mut parts = vec![format!("Requested {}", showing.requested_at_label)];
    if let Some(value) = showing.scheduled_at_label.as_ref() {
        parts.push(format!("Scheduled {value}"));
    }
    if let Some(value) = showing.completed_at_label.as_ref() {
        parts.push(format!("Completed {value}"));
    }
    if let Some(value) = showing.cancelled_at_label.as_ref() {
        parts.push(format!("Cancelled {value}"));
    }
    parts.join(" · ")
}
