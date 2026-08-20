# CulebraLuxe Server Command → Authority Map

Status: AUTH-03 classification (PREPARED — not enforced until provider auth + owner bootstrap exist).

Coarse authorities only. No per-button authorities. Business-state legality stays with domain/workflow services.

## Writes (`app/portal/actions.ts`)

| Server action | Authority |
|---------------|-----------|
| `createTaskAction`, `completeTaskAction`, `cancelTaskAction`, `updateTaskDueAction` | `crm.write` |
| `logManualInteractionAction` | `crm.write` |
| `updatePersonNotesAction`, `updatePersonStatusAction` | `crm.write` |
| `rejectIntakeAction`, `attachIntakeToPersonAction` | `crm.write` |
| `createShowingAction`, `scheduleShowingAction`, `cancelShowingAction`, `completeShowingAction` | `deal.write` |
| `submitOfferAction`, `withdrawOfferAction`, `rejectOfferAction` | `deal.write` |
| `addOtherParticipantAction`, `endParticipantAction`, `updateParticipantRoleLabelAction` | `deal.write` |
| `updatePropertyFactsAction`, `updatePropertyVisibilityAction` | `listing.write` |
| `setPropertyMediaOrderAction`, `setPropertyHeroAction`, `unlinkPropertyMediaAction`, `updateMediaMetadataAction` | `listing.write` |
| Future settings mutations (assign roles, manage roles/authorities) | `settings.manage` |

## Reads

| Surface | Authority |
|---------|-----------|
| Portal dashboards / operational views | `portal.read` |
| Deal workspaces / deal listing | `deal.read` |
| Settings views (users/roles/authorities/status) | `settings.read` |

## Boundary notes

- `attachIntakeToPersonAction` remains `crm.write` at the authority layer, but the *business* decision to attach a person is identity-sensitive and will additionally be workflow-gated (not expressed as a separate authority).
- `completeShowingAction`, `submitOfferAction`/`rejectOfferAction`/`withdrawOfferAction`, and `endParticipantAction` are consequential commands; they are `deal.write` at the authority layer and will carry CRM-14 business-state gating separately.
