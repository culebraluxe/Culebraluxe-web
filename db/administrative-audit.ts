import { sql } from './client'
import {
  ADMIN_ACTION_EVENT_TYPE,
  isAdministrativeAction,
  type AdministrativeAuditInput,
} from '../lib/audit/administrative-actions'

// ---------------------------------------------------------------------------
// AUTH-05 — Generalized administrative audit write (settings / identity-link /
// reset / reconcile), reusing the existing security_audit_event table.
//
// Reuse-first (architect brief): a sibling admin_audit_event table is NOT
// created; ADMIN_ACTION rows live in security_audit_event with the structured
// action / resource_type / resource_id / outcome / request_id columns added by
// migration 039. Break-glass auth rows are untouched.
//
// Contract (BOUNDARY, docs/workflow-integration-contract.md):
//   - Application/control-plane, server-side only. Reads NEVER depend on audit
//     rows.
//   - Best-effort-AFTER-success: callers invoke this AFTER the mutation
//     succeeds; it does NOT gate the mutation (authority/domain validation
//     already gates). Callers may wrap in try/catch — a failed audit write
//     must never fail or roll back the mutation it describes.
//   - NEVER store secrets, tokens, notes, or PII beyond the minimal resource
//     reference. `action` is allow-listed (isAdministrativeAction) — a
//     non-listed verb is rejected here, so over-auditing cannot creep in
//     through this seam.
// ---------------------------------------------------------------------------

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value)
}

export async function recordAdministrativeAudit(
  input: AdministrativeAuditInput,
): Promise<void> {
  if (!isAdministrativeAction(input.action)) {
    throw new Error(
      `Action '${input.action}' is not on the administrative audit allow-list.`,
    )
  }
  if (input.resourceId != null && !isUuid(input.resourceId)) {
    throw new Error(
      `resourceId must be a valid uuid or null, got '${input.resourceId}'.`,
    )
  }

  await sql`
    insert into security_audit_event (
      app_user_id,
      event_type,
      authentication_method,
      action,
      resource_type,
      resource_id,
      outcome,
      request_id
    ) values (
      ${input.appUserId},
      ${ADMIN_ACTION_EVENT_TYPE},
      null,
      ${input.action},
      ${input.resourceType},
      ${input.resourceId},
      ${input.outcome},
      ${input.requestId ?? null}
    )
  `
}
