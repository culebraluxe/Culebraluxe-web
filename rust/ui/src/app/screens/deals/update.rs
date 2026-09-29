//! The Deals update: the read, search, commands, and every message.

#[allow(unused_imports)]
use super::*;

pub(super) fn read(ctx: &ScreenCtx) -> Cmd<Msg> {
    Cmd::request(
        DealsRead {
            deal_id: ctx.id.clone(),
        },
        Msg::Loaded,
    )
}

pub(super) fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
    (
        Model {
            read: Remote::Loading,
            ..Model::default()
        },
        read(ctx),
    )
}

/// A search for people once two characters are typed; fewer clears the list.
pub(super) fn search(
    people: &mut Vec<crate::model::PortalDealPersonCandidate>,
    searching: &mut bool,
    purpose: &'static str,
    query: &str,
) -> Cmd<Msg> {
    let query = query.trim().to_string();
    people.clear();
    if query.chars().count() < 2 {
        *searching = false;
        return Cmd::none();
    }
    *searching = true;
    Cmd::request(
        DealPeopleSearch {
            query: query.clone(),
        },
        move |answer| Msg::PeopleLoaded {
            purpose,
            query,
            answer,
        },
    )
}

/// Send one workspace command, if the user may and none is in flight.
pub(super) fn command(
    model: &mut Model,
    ctx: &ScreenCtx,
    busy_action: impl Into<String>,
    command: PortalDealCommand,
) -> Cmd<Msg> {
    let action = match &command {
        PortalDealCommand::CreateShowing { .. }
        | PortalDealCommand::ScheduleShowing { .. }
        | PortalDealCommand::CancelShowing { .. }
        | PortalDealCommand::CompleteShowing { .. } => "showing.write",
        _ => "deal.write",
    };
    if !ctx.can(action) || model.deal_workspace.busy_action.is_some() {
        return Cmd::none();
    }
    let Some(deal_id) = ctx.id.clone() else {
        model.error = Some("This workspace is missing its deal identifier.".into());
        return Cmd::none();
    };
    model.deal_workspace.busy_action = Some(busy_action.into());
    model.error = None;
    Cmd::request(
        DealWorkspaceCommand { deal_id, command },
        Msg::DealWorkspaceCommandAnswered,
    )
}

pub(super) fn workspace(model: &Model) -> Option<&crate::model::PortalDealWorkspace> {
    model
        .read
        .loaded()
        .and_then(|deals| deals.workspace.as_ref())
}

pub(super) fn trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(super) fn update(model: &mut Model, msg: Msg, ctx: &ScreenCtx) -> Cmd<Msg> {
    let create = &mut model.deal_create;
    let ws = &mut model.deal_workspace;
    match msg {
        Msg::Loaded(answer) => {
            model.refreshing = false;
            match answer.and_then(|page| {
                page.deals
                    .ok_or_else(|| ApiError::decode("The answer had no contracts in it."))
            }) {
                Ok(deals) => model.read = Remote::Loaded(deals),
                // A failed refresh keeps the workspace the user was reading and says why.
                Err(error) if model.read.loaded().is_some() => model.error = Some(error.message),
                Err(error) => model.read = Remote::Failed(error),
            }
        }
        Msg::FilterChanged(filter) => model.controls.filter = Some(filter),

        // ---- the portfolio's create panel ---------------------------------------------------------------------------
        Msg::DealCreateToggled => {
            create.open = !create.open;
            model.error = None;
        }
        Msg::DealCreatePropertyChanged(value) => {
            create.property_id = value;
            model.error = None;
        }
        Msg::DealCreateClientQueryChanged(value) => {
            // Typing past a chosen client un-chooses it: the id and the text must name the same person.
            if value != create.client_label {
                create.client_person_id.clear();
                create.client_label.clear();
            }
            create.client_query = value;
            model.error = None;
            let query = create.client_query.clone();
            return search(&mut create.people, &mut create.searching, "client", &query);
        }
        Msg::DealCreateClientSelected { id, label } => {
            create.client_person_id = id;
            create.client_label = label.clone();
            create.client_query = label;
            create.people.clear();
            create.searching = false;
            model.error = None;
        }
        Msg::DealCreateOwnerChanged(value) => {
            create.owner_user_id = value;
            model.error = None;
        }
        Msg::DealCreateNotesChanged(value) => {
            create.notes = value;
            model.error = None;
        }
        Msg::DealCreateRequested => {
            if create.submitting || !ctx.can("deal.write") {
                return Cmd::none();
            }
            let Some(property_id) = trimmed(&create.property_id) else {
                model.error = Some("Choose a property first.".into());
                return Cmd::none();
            };
            let Some(client_person_id) = trimmed(&create.client_person_id) else {
                model.error = Some("Select an existing client person first.".into());
                return Cmd::none();
            };
            create.submitting = true;
            model.error = None;
            return Cmd::request(
                DealCreate {
                    property_id,
                    client_person_id,
                    owner_user_id: trimmed(&create.owner_user_id),
                    notes: trimmed(&create.notes),
                },
                Msg::DealCreated,
            );
        }
        Msg::DealCreated(answer) => {
            create.submitting = false;
            match answer {
                Ok(created) => {
                    *create = DealCreateState::default();
                    return Cmd::navigate(format!("/portal/deals/{}", created.id));
                }
                Err(error) => model.error = Some(error.message),
            }
        }

        Msg::PeopleLoaded {
            purpose,
            query,
            answer,
        } => {
            let (current, people, searching) = match purpose {
                "client" => (
                    &create.client_query,
                    &mut create.people,
                    &mut create.searching,
                ),
                "participant" => (
                    &ws.participant_query,
                    &mut ws.participant_people,
                    &mut ws.participant_searching,
                ),
                _ => (
                    &ws.structural_query,
                    &mut ws.structural_people,
                    &mut ws.structural_searching,
                ),
            };
            if current.trim() != query {
                return Cmd::none();
            }
            *searching = false;
            match answer {
                Ok(found) => *people = found.people,
                Err(error) => model.error = Some(error.message),
            }
        }

        // ---- the workspace: tasks -----------------------------------------------------------------------------------
        Msg::DealWorkspaceTaskTitleChanged(value) => {
            ws.task_title = value;
            model.error = None;
        }
        Msg::DealWorkspaceTaskDetailChanged(value) => {
            ws.task_detail = value;
            model.error = None;
        }
        Msg::DealWorkspaceTaskDueChanged(value) => {
            ws.task_due_at = value;
            model.error = None;
        }
        Msg::DealWorkspaceCreateTaskRequested => {
            let Some(title) = trimmed(&ws.task_title) else {
                model.error = Some("Task title is required.".into());
                return Cmd::none();
            };
            let detail = trimmed(&ws.task_detail);
            let due_at = trimmed(&ws.task_due_at);
            return command(
                model,
                ctx,
                "task:create",
                PortalDealCommand::CreateTask {
                    title,
                    detail,
                    due_at,
                },
            );
        }
        Msg::DealWorkspaceCompleteTaskRequested { task_id } => {
            return command(
                model,
                ctx,
                format!("task:complete:{task_id}"),
                PortalDealCommand::CompleteTask { task_id },
            )
        }

        // ---- offers -------------------------------------------------------------------------------------------------
        Msg::DealWorkspaceOfferAmountChanged { key, value } => {
            if value.is_empty() {
                ws.offer_amounts.remove(&key);
            } else {
                ws.offer_amounts.insert(key, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceOfferTermChanged { key, field, value } => {
            let target = match field {
                "financing" => &mut ws.offer_financing,
                "deposit" => &mut ws.offer_deposits,
                "inspectionDays" => &mut ws.offer_inspection_days,
                "sellerCredits" => &mut ws.offer_seller_credits,
                "closingDate" => &mut ws.offer_closing_dates,
                "contingencies" => &mut ws.offer_contingencies,
                "expiresAt" => &mut ws.offer_expirations,
                _ => return Cmd::none(),
            };
            if value.is_empty() {
                target.remove(&key);
            } else {
                target.insert(key, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceSubmitOfferRequested { parent_offer_id } => {
            let key = parent_offer_id.as_deref().unwrap_or("root").to_string();
            let Some(amount) = ws.offer_amounts.get(&key).and_then(|value| trimmed(value)) else {
                model.error = Some("Offer amount is required.".into());
                return Cmd::none();
            };
            // READ THE TERMS WHILE `ws` IS BORROWED, THEN LET IT GO. `command` takes `&mut model` whole, so a live
            // `&mut model.deal_workspace` at the call site is a borrow error (E0502/E0499); the locals below own
            // their strings and end the borrow before it.
            let financing_type = ws.offer_financing.get(&key).and_then(|value| trimmed(value));
            let deposit_amount = ws.offer_deposits.get(&key).and_then(|value| trimmed(value));
            let inspection_days = ws.offer_inspection_days.get(&key).and_then(|value| trimmed(value));
            let seller_credits = ws.offer_seller_credits.get(&key).and_then(|value| trimmed(value));
            let proposed_closing_date = ws.offer_closing_dates.get(&key).and_then(|value| trimmed(value));
            let contingencies = ws.offer_contingencies.get(&key).and_then(|value| trimmed(value));
            let expires_at = ws.offer_expirations.get(&key).and_then(|value| trimmed(value));
            let client_id = workspace(model)
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone());
            let Some(person_id) = client_id else {
                model.error = Some("This deal does not have an active client.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("offer:submit:{key}"),
                PortalDealCommand::SubmitOffer {
                    person_id,
                    amount,
                    parent_offer_id,
                    financing_type,
                    deposit_amount,
                    inspection_days,
                    seller_credits,
                    proposed_closing_date,
                    contingencies,
                    expires_at,
                },
            );
        }
        Msg::DealWorkspaceWithdrawOfferRequested { offer_id } => {
            return command(
                model,
                ctx,
                format!("offer:withdraw:{offer_id}"),
                PortalDealCommand::WithdrawOffer { offer_id },
            )
        }
        Msg::DealWorkspaceRejectOfferRequested { offer_id } => {
            return command(
                model,
                ctx,
                format!("offer:reject:{offer_id}"),
                PortalDealCommand::RejectOffer { offer_id },
            )
        }

        // ---- showings -----------------------------------------------------------------------------------------------
        Msg::DealWorkspaceCreateShowingRequested => {
            let workspace = workspace(model);
            let person_id = workspace
                .and_then(|workspace| workspace.client.as_ref())
                .map(|client| client.id.clone());
            let property_id = workspace
                .and_then(|workspace| workspace.property.as_ref())
                .map(|property| property.id.clone());
            let Some(person_id) = person_id else {
                model.error = Some("This deal does not have an active client.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                "showing:create",
                PortalDealCommand::CreateShowing {
                    person_id,
                    property_id,
                },
            );
        }
        Msg::DealWorkspaceShowingTimeChanged { showing_id, value } => {
            if value.is_empty() {
                ws.showing_times.remove(&showing_id);
            } else {
                ws.showing_times.insert(showing_id, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceScheduleShowingRequested { showing_id } => {
            let Some(scheduled_at) = ws
                .showing_times
                .get(&showing_id)
                .and_then(|value| trimmed(value))
            else {
                model.error = Some("Choose a showing date and time first.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("showing:schedule:{showing_id}"),
                PortalDealCommand::ScheduleShowing {
                    showing_id,
                    scheduled_at,
                },
            );
        }
        Msg::DealWorkspaceCancelShowingRequested { showing_id } => {
            return command(
                model,
                ctx,
                format!("showing:cancel:{showing_id}"),
                PortalDealCommand::CancelShowing { showing_id },
            )
        }
        Msg::DealWorkspaceCompleteShowingRequested { showing_id } => {
            return command(
                model,
                ctx,
                format!("showing:complete:{showing_id}"),
                PortalDealCommand::CompleteShowing { showing_id },
            )
        }

        // ---- other participants -------------------------------------------------------------------------------------
        Msg::DealWorkspaceParticipantQueryChanged(value) => {
            if value != ws.participant_label {
                ws.participant_person_id.clear();
                ws.participant_label.clear();
            }
            ws.participant_query = value.clone();
            model.error = None;
            return search(
                &mut ws.participant_people,
                &mut ws.participant_searching,
                "participant",
                &value,
            );
        }
        Msg::DealWorkspaceParticipantSelected { id, label } => {
            ws.participant_person_id = id;
            ws.participant_label = label.clone();
            ws.participant_query = label;
            ws.participant_people.clear();
            ws.participant_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceParticipantRoleChanged(value) => {
            ws.participant_role_label = value;
            model.error = None;
        }
        Msg::DealWorkspaceAddParticipantRequested => {
            let Some(person_id) = trimmed(&ws.participant_person_id) else {
                model.error = Some("Select an existing person first.".into());
                return Cmd::none();
            };
            let Some(role_label) = trimmed(&ws.participant_role_label) else {
                model.error = Some("Enter a participant role label.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                "participant:add",
                PortalDealCommand::AddOtherParticipant {
                    person_id,
                    role_label,
                },
            );
        }
        Msg::DealWorkspaceOtherRoleChanged {
            participant_id,
            value,
        } => {
            if value.is_empty() {
                ws.other_role_labels.remove(&participant_id);
            } else {
                ws.other_role_labels.insert(participant_id, value);
            }
            model.error = None;
        }
        Msg::DealWorkspaceUpdateOtherRequested { participant_id } => {
            let Some(role_label) = ws
                .other_role_labels
                .get(&participant_id)
                .and_then(|value| trimmed(value))
            else {
                model.error = Some("Enter a new role label.".into());
                return Cmd::none();
            };
            return command(
                model,
                ctx,
                format!("participant:update:{participant_id}"),
                PortalDealCommand::UpdateOtherParticipant {
                    participant_id,
                    role_label,
                },
            );
        }
        Msg::DealWorkspaceEndOtherRequested { participant_id } => {
            return command(
                model,
                ctx,
                format!("participant:end:{participant_id}"),
                PortalDealCommand::EndOtherParticipant { participant_id },
            )
        }

        // ---- structural participants (client, owner, seller) --------------------------------------------------------
        Msg::DealWorkspaceStructuralRoleChanged(role) => {
            ws.structural_role = role;
            ws.structural_query.clear();
            ws.structural_person_id.clear();
            ws.structural_label.clear();
            ws.structural_owner_user_id.clear();
            ws.structural_people.clear();
            ws.structural_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceStructuralQueryChanged(value) => {
            if value != ws.structural_label {
                ws.structural_person_id.clear();
                ws.structural_label.clear();
            }
            ws.structural_query = value.clone();
            model.error = None;
            return search(
                &mut ws.structural_people,
                &mut ws.structural_searching,
                "structural",
                &value,
            );
        }
        Msg::DealWorkspaceStructuralPersonSelected { id, label } => {
            ws.structural_person_id = id;
            ws.structural_label = label.clone();
            ws.structural_query = label;
            ws.structural_people.clear();
            ws.structural_searching = false;
            model.error = None;
        }
        Msg::DealWorkspaceStructuralOwnerChanged(value) => {
            ws.structural_owner_user_id = value;
            model.error = None;
        }
        Msg::DealWorkspaceSetStructuralRequested => {
            let role = ws.structural_role.clone();
            if !matches!(role.as_str(), "client" | "owner" | "seller") {
                model.error = Some("Choose client, owner, or seller first.".into());
                return Cmd::none();
            }
            let (person_id, user_id) = if role == "owner" {
                let Some(id) = trimmed(&ws.structural_owner_user_id) else {
                    model.error = Some("Choose an owner user first.".into());
                    return Cmd::none();
                };
                (None, Some(id))
            } else {
                let Some(id) = trimmed(&ws.structural_person_id) else {
                    model.error = Some("Select an existing person first.".into());
                    return Cmd::none();
                };
                (Some(id), None)
            };
            return command(
                model,
                ctx,
                format!("structural:set:{role}"),
                PortalDealCommand::SetStructuralParticipant {
                    role,
                    person_id,
                    user_id,
                },
            );
        }
        Msg::DealWorkspaceEndStructuralRequested { participant_id } => {
            return command(
                model,
                ctx,
                format!("structural:end:{participant_id}"),
                PortalDealCommand::EndStructuralParticipant { participant_id },
            )
        }

        Msg::DealWorkspaceCommandAnswered(answer) => {
            let completed = ws.busy_action.take().unwrap_or_default();
            if let Err(error) = answer {
                // Refused: the form keeps what was typed, so it can be corrected and sent again.
                model.error = Some(error.message);
                return Cmd::none();
            }
            if completed == "task:create" {
                ws.task_title.clear();
                ws.task_detail.clear();
                ws.task_due_at.clear();
            } else if let Some(key) = completed.strip_prefix("offer:submit:") {
                ws.offer_amounts.remove(key);
                ws.offer_financing.remove(key);
                ws.offer_deposits.remove(key);
                ws.offer_inspection_days.remove(key);
                ws.offer_seller_credits.remove(key);
                ws.offer_closing_dates.remove(key);
                ws.offer_contingencies.remove(key);
                ws.offer_expirations.remove(key);
            } else if completed == "participant:add" {
                ws.participant_query.clear();
                ws.participant_person_id.clear();
                ws.participant_label.clear();
                ws.participant_role_label.clear();
                ws.participant_people.clear();
            } else if completed.starts_with("structural:set:") {
                ws.structural_role.clear();
                ws.structural_query.clear();
                ws.structural_person_id.clear();
                ws.structural_label.clear();
                ws.structural_owner_user_id.clear();
                ws.structural_people.clear();
            } else if let Some(id) = completed.strip_prefix("participant:update:") {
                ws.other_role_labels.remove(id);
            } else if let Some(id) = completed.strip_prefix("showing:schedule:") {
                ws.showing_times.remove(id);
            }
            model.error = None;
            model.refreshing = true;
            return read(ctx);
        }
    }
    Cmd::none()
}
