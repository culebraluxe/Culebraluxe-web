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
// CRM-22 — canonical non-closing milestone deadline commands (deal columns
// deal.inspection_deadline / deal.financing_deadline, set via the single
// canonical service db/deal-deadline.ts).
export const DEAL_SET_INSPECTION_DEADLINE = 'deal.set_inspection_deadline'
export const DEAL_SET_FINANCING_DEADLINE = 'deal.set_financing_deadline'
export const OFFER_ACCEPT = 'offer.accept'
export const TASK_CREATE = 'task.create'
export const TASK_COMPLETE = 'task.complete'
export const TASK_CANCEL = 'task.cancel'

// CRM-23 — canonical interaction intake command. The integration inbox (and
// any future intake path) persists canonical interactions through this
// command, never by writing the interaction table directly: CRM changes
// happen through the canonical Business Command layer.
export const INTERACTION_RECORD = 'interaction.record'

// DOC-03 — provider-neutral signature commands (the Signature Provider Seam).
// These are canonical application commands registered in lib/commands/register
// and dispatched by the signature application router (lib/signature) — NOT
// workflow engine commands (the workflow engine never signs).
export const SIGNATURE_REQUEST_SEND = 'signature.request.send'
export const SIGNATURE_REQUEST_STATUS = 'signature.request.status'
export const SIGNATURE_REQUEST_CANCEL = 'signature.request.cancel'
export const SIGNATURE_REQUEST_DECLINE = 'signature.request.decline'

// DOC-06 — canonical issuance command. Issuing a form instance as an immutable
// transaction document is a meaningful business action routed through the
// Business Command seam (claim-first receipt, durable transaction boundary).
// Draft-field editing stays OUTSIDE the command layer (plain form-instance
// updates — no durable business record is created).
export const DOCUMENT_ISSUE = 'document.issue'
