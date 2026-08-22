// ---------------------------------------------------------------------------
// CRM-14J — Canonical command type identifiers (dependency-free constants).
//
// Single source of truth for the stable machine command identifiers used by
// the canonical command layer (lib/commands). workflow_app/command-types.ts
// (the workflow command inventory, CRM-14G) re-exports these constants so the
// router inventory and the canonical registry can never drift apart. This
// module imports nothing, so any layer (including tests) can import it.
//
// A command type is a stable machine identifier, NOT a workflow node name and
// NOT a provider integration name. The workflow engine only ever sees these
// identifiers through the application command request; it never interprets
// them.
// ---------------------------------------------------------------------------

export const DEAL_SET_STAGE_UNDER_CONTRACT = 'deal.set_stage_under_contract'
export const DEAL_SET_STAGE_CLOSED = 'deal.set_stage_closed'
export const DEAL_SET_CLOSING_DATE = 'deal.set_closing_date'
export const DEAL_SET_FINANCING_TYPE = 'deal.set_financing_type'
export const DEAL_SET_APPRAISAL_REQUIRED = 'deal.set_appraisal_required'
export const DEAL_SET_LENDER_CLEAR_TO_CLOSE = 'deal.set_lender_clear_to_close'
export const OFFER_ACCEPT = 'offer.accept'
export const TASK_CREATE = 'task.create'
export const TASK_COMPLETE = 'task.complete'
export const TASK_CANCEL = 'task.cancel'
