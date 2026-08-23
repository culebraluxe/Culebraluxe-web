// ---------------------------------------------------------------------------
// AUTH-05 — Sensitive Administrative Write Audit: allow-list + metadata shape.
//
// Single source of truth for WHICH mutations warrant a durable
// actor/action/resource/occurred_at/outcome record. This module imports
// nothing from the database layer (the command-type constants it re-exports
// are pure strings), so any layer — including tests — can import it.
//
// Two audit mechanisms, one allow-list:
//   (a)/(c) administrative actions (settings mutations, identity link/unlink,
//           admin reset/reconcile) -> generalized write into
//           security_audit_event (db/administrative-audit.ts). These rows are
//           event_type = ADMIN_ACTION with structured action / resource_type /
//           resource_id / outcome / request_id columns.
//   (d)     consequential business commands (offer.accept, deal.set_stage_*,
//           deal.set_financing_type, deal.set_closing_date) -> the command
//           receipt itself records actor_app_user_id (threaded through the
//           canonical command seam), because the receipt already gives
//           idempotent ordering + outcome + aggregate + timestamp.
//
// Rejected shapes (docs boundary, docs/workflow-integration-contract.md):
//   - auditing every write (event-sourcing creep),
//   - a new application_event table,
//   - secrets / tokens / notes / PII beyond the minimal resource reference,
//   - audit as the authorization mechanism.
// ---------------------------------------------------------------------------

import {
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_STAGE_UNDER_CONTRACT,
  OFFER_ACCEPT,
} from '../commands/command-types'

// ---------------------------------------------------------------------------
// Administrative action verbs (a)/(c) — the allow-list.
// ---------------------------------------------------------------------------

export const USER_ROLE_ASSIGN = 'user.role.assign'
export const USER_ROLE_UNASSIGN = 'user.role.unassign'
export const USER_STATUS_SET = 'user.status.set'
export const IDENTITY_LINK = 'identity.link'
export const IDENTITY_UNLINK = 'identity.unlink'
export const ADMIN_RESET = 'admin.reset'
export const ADMIN_RECONCILE = 'admin.reconcile'

/** Every administrative action the generalized audit write may record. */
export const ADMINISTRATIVE_ACTIONS = [
  USER_ROLE_ASSIGN,
  USER_ROLE_UNASSIGN,
  USER_STATUS_SET,
  IDENTITY_LINK,
  IDENTITY_UNLINK,
  ADMIN_RESET,
  ADMIN_RECONCILE,
] as const

export type AdministrativeAction = (typeof ADMINISTRATIVE_ACTIONS)[number]

/** Runtime guard set (mirrors the const tuple; cheap membership checks). */
export const ADMINISTRATIVE_ACTION_SET: ReadonlySet<string> = new Set(
  ADMINISTRATIVE_ACTIONS,
)

/** event_type for generalized administrative audit rows (reuses the table). */
export const ADMIN_ACTION_EVENT_TYPE = 'ADMIN_ACTION'

/** Audit outcome vocabulary: success / denied / conflict. */
export type AdministrativeOutcome = 'success' | 'denied' | 'conflict'

// ---------------------------------------------------------------------------
// Consequential business commands (d) — the receipt-audited allow-list.
// ---------------------------------------------------------------------------

/**
 * Command types whose RECEIPT must record actor_app_user_id when the caller
 * supplies one. Threaded through the canonical command seam
 * (db/workflow-command-receipt.ts claim/finalize). Commands OUTSIDE this set
 * still get receipts (idempotency), but do not capture an actor.
 */
export const AUDITED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  OFFER_ACCEPT,
  DEAL_SET_STAGE_UNDER_CONTRACT,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_CLOSING_DATE,
])

// ---------------------------------------------------------------------------
// Metadata shape (shared vocabulary)
// ---------------------------------------------------------------------------

/**
 * The durable record for an allow-listed administrative mutation:
 *   actor      -> app_user_id (FK; display name is derived via join — never
 *                 denormalized, keeping PII to the minimal resource reference)
 *   action     -> allow-listed verb (user.role.assign, identity.link, ...)
 *   resource   -> resource_type + resource_id (aggregateId / table+row ref)
 *   occurred_at-> server timestamp (column default)
 *   outcome    -> success | denied | conflict
 *   requestId  -> optional request/correlation id for chaining
 */
export type AdministrativeAuditInput = {
  appUserId: string | null
  action: AdministrativeAction
  resourceType: string | null
  resourceId: string | null
  outcome: AdministrativeOutcome
  requestId?: string | null
}

/** True when the action verb is on the administrative allow-list. */
export function isAdministrativeAction(value: string): value is AdministrativeAction {
  return ADMINISTRATIVE_ACTION_SET.has(value)
}
