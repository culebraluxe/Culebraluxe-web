//! Participants: the list, each row's controls, and the add forms with their person search.

#[allow(unused_imports)]
use super::*;

pub(super) fn participants_card(
    model: &Vm<'_>,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    html! {
        <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
            <div class="border-b border-[var(--portal-panel-border)] px-5 py-4">
                <h2 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Participants"}</h2>
                <p class="mt-1 text-xs font-light text-black/40">{"Canonical deal_participant roles."}</p>
            </div>
            if workspace.participants.is_empty() {
                <p class="px-5 py-6 text-sm font-light text-black/40">{"No participants on record."}</p>
            } else {
                <div>
                    { for workspace.participants.iter().map(|participant| participant_row(model, participant, on_msg, busy)) }
                </div>
            }
            { add_participant_form(model, on_msg, busy) }
            { structural_participant_form(model, workspace, on_msg, busy) }
        </section>
    }
}

pub(super) fn participant_row(
    model: &Vm<'_>,
    participant: &crate::model::PortalDealWorkspaceParticipant,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let role = participant
        .role_label
        .as_deref()
        .unwrap_or_else(|| participant_role_label(&participant.role_category));
    let participant_id = participant.id.clone();
    let end_other = {
        let on_msg = on_msg.clone();
        let id = participant_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceEndOtherRequested {
                participant_id: id.clone(),
            })
        })
    };
    let end_structural = {
        let on_msg = on_msg.clone();
        let id = participant_id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceEndStructuralRequested {
                participant_id: id.clone(),
            })
        })
    };

    html! {
        <div class="border-b border-[var(--portal-panel-border)] px-5 py-4 last:border-b-0">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="text-[10px] font-light uppercase tracking-[0.15em] text-[var(--portal-navy-soft)]">
                        { role }
                    </div>
                    <div class="mt-1 font-serif text-lg font-light text-[var(--portal-navy)]">
                        { participant.name.clone() }
                    </div>
                    if let Some(detail) = participant.detail.as_ref() {
                        <div class="mt-1 truncate text-xs font-light text-black/40">{ detail.clone() }</div>
                    }
                </div>
                <span class="text-[10px] font-light uppercase tracking-[0.12em] text-black/30">
                    { participant.kind.clone() }
                </span>
            </div>
            if participant.role_category == "other" {
                { other_participant_controls(model, participant, on_msg, end_other, busy) }
            } else if participant.role_category != "client" {
                <button type="button" onclick={end_structural} disabled={busy}
                    class="mt-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-archive)] disabled:opacity-35">
                    {"End role"}
                </button>
            }
        </div>
    }
}

pub(super) fn other_participant_controls(
    model: &Vm<'_>,
    participant: &crate::model::PortalDealWorkspaceParticipant,
    on_msg: &Callback<Msg>,
    end_other: Callback<MouseEvent>,
    busy: bool,
) -> Html {
    let id = participant.id.clone();
    let value = model
        .deal_workspace
        .other_role_labels
        .get(&id)
        .cloned()
        .unwrap_or_else(|| participant.role_label.clone().unwrap_or_default());
    let oninput = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::DealWorkspaceOtherRoleChanged {
                participant_id: id.clone(),
                value,
            });
        })
    };
    let save = {
        let on_msg = on_msg.clone();
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            on_msg.emit(Msg::DealWorkspaceUpdateOtherRequested {
                participant_id: id.clone(),
            })
        })
    };

    html! {
        <div class="mt-3 flex flex-wrap items-center gap-2">
            <input value={value} {oninput} class="h-8 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/65 px-2 text-xs font-light outline-none focus:border-[var(--portal-navy)]" />
            <button type="button" onclick={save} disabled={busy}
                class="h-8 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)] disabled:opacity-35">
                {"Save"}
            </button>
            <button type="button" onclick={end_other} disabled={busy}
                class="h-8 px-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-archive)] disabled:opacity-35">
                {"End"}
            </button>
        </div>
    }
}

pub(super) fn add_participant_form(model: &Vm<'_>, on_msg: &Callback<Msg>, busy: bool) -> Html {
    let query_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceParticipantQueryChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceParticipantRoleChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let add = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceAddParticipantRequested))
    };

    html! {
        <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
            <p class="text-[10px] font-medium uppercase tracking-[0.13em] text-black/40">{"Add participant"}</p>
            <div class="relative mt-2">
                <input type="search" value={model.deal_workspace.participant_query.clone()} oninput={query_change}
                    placeholder="Search person…" class={field_class()} />
                if !model.deal_workspace.participant_people.is_empty() {
                    <div class="absolute z-20 mt-1 max-h-48 w-full overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-xl">
                        { for model.deal_workspace.participant_people.iter().map(|person| workspace_person_choice(person, "participant", on_msg)) }
                    </div>
                }
            </div>
            <input value={model.deal_workspace.participant_role_label.clone()} oninput={role_change}
                placeholder="Role: lender, inspector, notario…" class={field_class()} />
            <button type="button" onclick={add} disabled={busy || model.deal_workspace.participant_person_id.is_empty()}
                class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-white disabled:opacity-35">
                {"Add"}
            </button>
        </div>
    }
}

pub(super) fn structural_participant_form(
    model: &Vm<'_>,
    workspace: &crate::model::PortalDealWorkspace,
    on_msg: &Callback<Msg>,
    busy: bool,
) -> Html {
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            on_msg.emit(Msg::DealWorkspaceStructuralRoleChanged(
                crate::app::exec::select_value(&event),
            ))
        })
    };
    let query_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::DealWorkspaceStructuralQueryChanged(
                crate::app::exec::input_value(&event),
            ))
        })
    };
    let owner_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            on_msg.emit(Msg::DealWorkspaceStructuralOwnerChanged(
                crate::app::exec::select_value(&event),
            ))
        })
    };
    let save = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::DealWorkspaceSetStructuralRequested))
    };
    let role = model.deal_workspace.structural_role.as_str();

    html! {
        <div class="border-t border-[var(--portal-panel-border)] bg-white/20 px-5 py-4">
            <p class="text-[10px] font-medium uppercase tracking-[0.13em] text-black/40">{"Replace structural role"}</p>
            <select value={model.deal_workspace.structural_role.clone()} onchange={role_change} class={field_class()}>
                <option value="" selected={role.is_empty()}>{"Choose role…"}</option>
                <option value="client" selected={role == "client"}>{"Client"}</option>
                <option value="owner" selected={role == "owner"}>{"Owner"}</option>
                <option value="seller" selected={role == "seller"}>{"Seller"}</option>
            </select>
            if role == "owner" {
                <select value={model.deal_workspace.structural_owner_user_id.clone()} onchange={owner_change} class={field_class()}>
                    <option value="" selected={model.deal_workspace.structural_owner_user_id.is_empty()}>{"Choose active user…"}</option>
                    { for workspace.owner_candidates.iter().map(|user| html! {
                        <option value={user.id.clone()} selected={user.id == model.deal_workspace.structural_owner_user_id}>{ user.display_name.clone() }</option>
                    }) }
                </select>
            } else if matches!(role, "client" | "seller") {
                <div class="relative">
                    <input type="search" value={model.deal_workspace.structural_query.clone()} oninput={query_change}
                        placeholder="Search person…" class={field_class()} />
                    if !model.deal_workspace.structural_people.is_empty() {
                        <div class="absolute z-20 mt-1 max-h-48 w-full overflow-y-auto rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white shadow-xl">
                            { for model.deal_workspace.structural_people.iter().map(|person| workspace_person_choice(person, "structural", on_msg)) }
                        </div>
                    }
                </div>
            }
            <button type="button" onclick={save} disabled={busy || role.is_empty()}
                class="mt-2 min-h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-navy)] px-3 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-35">
                {"Set / replace"}
            </button>
        </div>
    }
}

pub(super) fn workspace_person_choice(
    person: &PortalDealPersonCandidate,
    purpose: &'static str,
    on_msg: &Callback<Msg>,
) -> Html {
    let id = person.id.clone();
    let label = person.display_name.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| {
            if purpose == "participant" {
                on_msg.emit(Msg::DealWorkspaceParticipantSelected {
                    id: id.clone(),
                    label: label.clone(),
                });
            } else {
                on_msg.emit(Msg::DealWorkspaceStructuralPersonSelected {
                    id: id.clone(),
                    label: label.clone(),
                });
            }
        })
    };
    html! {
        <button type="button" {onclick}
            class="block w-full border-b border-[var(--portal-panel-border)] px-3 py-2 text-left last:border-b-0 hover:bg-[var(--portal-mist)]/45">
            <span class="block text-sm font-medium text-[var(--portal-navy)]">{ person.display_name.clone() }</span>
            <span class="text-[11px] font-light text-black/40">
                { person.email.clone().or(person.phone.clone()).or(person.location.clone()).unwrap_or_else(|| person.role.clone()) }
            </span>
        </button>
    }
}
