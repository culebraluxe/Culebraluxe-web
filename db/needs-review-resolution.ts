import { randomUUID } from 'node:crypto'

import { PortalWriteError } from '../lib/portal-write-error'
import { normalizeIdentityHint } from '../lib/crm-intake-normalization'
import type { WebsiteIntakeRequestType } from '../lib/website-intake-types'
import {
  createPersonWithIdentities,
  findIdentityOwnership,
  personExists,
} from './person-identities'
import type {
  QueryExecutor,
  QueryRow,
  TransactionExecutor,
} from './query-executor'
import { neonTx } from './tx'
import type { TxRunner } from './tx'
import { persistCanonicalWebsiteIntake } from './website-intake'

// ---------------------------------------------------------------------------
// CRM-09B — Needs Review human resolution write service.
//
// Single resolution seam for the Portal "Needs Review" queue. The read
// projection (db/needs-review.ts) is unchanged: a resolution moves the receipt
// out of ('received','resolution_required') and the queue drops it naturally.
//
// Discriminated action:
//   attach — link the intake to an explicitly selected existing person (the
//            intake email is recorded as a user_supplied person_identity when
//            it is not already owned), persist the canonical interaction, and
//            transition the receipt to completed.
//   create — operator-authorized person creation from the intake identity
//            (user_supplied email evidence, role buyer, display name from the
//            intake), then persist the canonical interaction and transition the
//            receipt to completed.
//   reject — transition the receipt to rejected.
//
// Boundaries (CRM-09B architect brief):
//   - Identity matching is explicit operator choice (db/people.ts searchPeople
//     in the UI). NO fuzzy/automatic merge. findIdentityOwnership refuses when
//     the intake email is already owned by a different person — never silently
//     reassign, and never create a second owner (person_identity_unique is the
//     DB backstop).
//   - Idempotency: the receipt row is locked (for update) and the terminal
//     update is compare-and-set guarded, so resolving an already-resolved
//     receipt is a no-op conflict; the canonical interaction dedupes via
//     (source_system, source_external_id).
//   - Auditability (AUTH-05): resolved_by_user_id + resolved_at are written on
//     the durable receipt. The receipt is never deleted.
//   - Atomicity: every effect runs inside ONE transaction, so a failed receipt
//     transition can never orphan a created person or canonical interaction.
//
// The human path intentionally does not call transitionWebsiteIntakeReceipt:
// that pipeline seam only allows from='processing' with a claim token
// (processing_started_at), which a never-claimed human receipt does not have.
// This service reuses the same receipt state-machine semantics — compare-and-
// set on the actionable statuses and the same terminal states — inside its own
// transaction.
// ---------------------------------------------------------------------------

export type ResolveIntakeAction =
  | { kind: 'attach'; personId: string }
  | { kind: 'create' }
  | { kind: 'reject' }

export type ResolveIntakeInput = {
  submissionId: string
  action: ResolveIntakeAction
  /**
   * AUTH-05 acting app_user. The Portal server action derives this from the
   * authenticated session (portalWrite -> acting user) and threads it in, so
   * resolved_by_user_id on the durable receipt records WHO resolved the item.
   * Null when the service is invoked outside the session path (tests,
   * automated pipeline) — resolved_by_user_id stays null then.
   */
  actorAppUserId?: string | null
}

export type ResolveIntakeResult =
  | {
      submissionId: string
      status: 'completed'
      interactionId: string
      personId: string
    }
  | { submissionId: string; status: 'rejected' }

type ReceiptRow = {
  id: string
  request_type: WebsiteIntakeRequestType
  property_id: string | null
  display_name: string
  email: string
  message: string | null
  status: string
  created_at: string
}

const ACTIONABLE_STATUSES = new Set(['received', 'resolution_required'])

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value)
}

function isUniqueViolation(error: unknown) {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    error.code === '23505'
  )
}

// Execute a TransactionExecutor-style service against an already-open
// transaction: every query it builds runs on `tx`, sharing one atomic boundary
// instead of starting a nested transaction. Queries run sequentially in build
// order (mirroring Neon's ordered transaction batch) so statements that guard
// on earlier inserts — e.g. the canonical interest/task inserts checking the
// interaction's existence — observe them.
function runIn(tx: QueryExecutor): TransactionExecutor {
  return async (buildQueries) => {
    const queries = buildQueries(tx)
    const results: QueryRow[][] = []
    for (const query of queries) {
      results.push(await query)
    }
    return results
  }
}

async function loadActionableReceipt(
  tx: QueryExecutor,
  submissionId: string,
): Promise<ReceiptRow> {
  const rows = await tx`
    select id, request_type, property_id, display_name, email, message,
      status, created_at
    from website_intake_submission
    where id = ${submissionId}
    for update
  `
  const row = rows[0] as ReceiptRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Submission not found.')
  }
  if (!ACTIONABLE_STATUSES.has(row.status)) {
    throw new PortalWriteError(
      'conflict',
      'Submission is already resolved or not actionable.',
    )
  }
  return row
}

function emailHint(email: string) {
  return normalizeIdentityHint({
    kind: 'email',
    value: email,
    evidence: 'user_supplied',
  })
}

async function personDisplayName(tx: QueryExecutor, personId: string) {
  const rows = await tx`
    select display_name from person where id = ${personId} limit 1
  `
  return (
    (rows[0] as { display_name: string } | undefined)?.display_name ??
    'another person'
  )
}

// Attach may only target the email's existing owner (or nobody). Attaching to
// a different person while the email is owned elsewhere is refused — never
// silently reassign.
async function assertAttachEmailNotOwnedElsewhere(
  tx: QueryExecutor,
  email: string,
  targetPersonId: string,
) {
  const ownership = await findIdentityOwnership(emailHint(email), tx)
  if (!ownership || ownership.personId === targetPersonId) return
  const ownerLabel = await personDisplayName(tx, ownership.personId)
  throw new PortalWriteError(
    'conflict',
    `This email already belongs to ${ownerLabel}. Attach to that person instead.`,
  )
}

// Create may only run when nobody owns the intake email; a second owner is
// refused up front (person_identity_unique is the DB backstop).
async function assertEmailNotOwned(tx: QueryExecutor, email: string) {
  const ownership = await findIdentityOwnership(emailHint(email), tx)
  if (!ownership) return
  const ownerLabel = await personDisplayName(tx, ownership.personId)
  throw new PortalWriteError(
    'conflict',
    `This email already belongs to ${ownerLabel}. Attach to that person instead of creating a new one.`,
  )
}

function canonicalInput(
  receipt: ReceiptRow,
  personId: string,
  interactionId: string,
) {
  return {
    interactionId,
    personId,
    propertyId: receipt.property_id ?? undefined,
    submissionId: receipt.id,
    requestType: receipt.request_type,
    // The interaction documents the intake event, so it occurred when the
    // submission was received — not when the human resolved it.
    occurredAt: receipt.created_at,
    displayName: receipt.display_name,
    email: receipt.email,
    message: receipt.message ?? undefined,
  }
}

async function completeReceipt(
  tx: QueryExecutor,
  submissionId: string,
  interactionId: string,
  actorAppUserId: string | null,
) {
  const rows = await tx`
    update website_intake_submission
    set status = 'completed',
        processing_started_at = null,
        interaction_id = ${interactionId},
        resolved_by_user_id = ${actorAppUserId},
        resolved_at = now(),
        updated_at = now()
    where id = ${submissionId}
      and status in ('received', 'resolution_required')
    returning id
  `
  return rows.length === 1
}

async function rejectReceipt(
  tx: QueryExecutor,
  submissionId: string,
  actorAppUserId: string | null,
) {
  const rows = await tx`
    update website_intake_submission
    set status = 'rejected',
        processing_started_at = null,
        resolved_by_user_id = ${actorAppUserId},
        resolved_at = now(),
        updated_at = now()
    where id = ${submissionId}
      and status in ('received', 'resolution_required')
    returning id
  `
  return rows.length === 1
}

export async function resolveIntake(
  input: ResolveIntakeInput,
  run: TxRunner = neonTx,
): Promise<ResolveIntakeResult> {
  if (!isUuid(input.submissionId)) {
    throw new PortalWriteError('validation', 'Submission identifier is invalid.')
  }
  if (
    input.action.kind === 'attach' &&
    !isUuid(input.action.personId)
  ) {
    throw new PortalWriteError('validation', 'Person identifier is invalid.')
  }
  if (input.actorAppUserId != null && !isUuid(input.actorAppUserId)) {
    throw new PortalWriteError(
      'validation',
      'Acting user identifier is invalid.',
    )
  }
  if (
    input.action.kind !== 'attach' &&
    input.action.kind !== 'create' &&
    input.action.kind !== 'reject'
  ) {
    throw new PortalWriteError('validation', 'Resolution action is invalid.')
  }

  const actorAppUserId = input.actorAppUserId ?? null

  return run(async (tx) => {
    const receipt = await loadActionableReceipt(tx, input.submissionId)

    if (input.action.kind === 'reject') {
      const done = await rejectReceipt(tx, receipt.id, actorAppUserId)
      if (!done) {
        throw new PortalWriteError(
          'conflict',
          'Submission is already resolved or not actionable.',
        )
      }
      return { submissionId: receipt.id, status: 'rejected' }
    }

    let personId: string
    if (input.action.kind === 'attach') {
      const targetPersonId = input.action.personId
      const exists = await personExists(targetPersonId, tx)
      if (!exists) {
        throw new PortalWriteError('not-found', 'Selected person does not exist.')
      }
      await assertAttachEmailNotOwnedElsewhere(
        tx,
        receipt.email,
        targetPersonId,
      )
      personId = targetPersonId
    } else {
      await assertEmailNotOwned(tx, receipt.email)
      personId = randomUUID()
      try {
        await createPersonWithIdentities(
          {
            personId,
            displayName: receipt.display_name,
            role: 'buyer',
            identities: [
              { kind: 'email', normalizedValue: receipt.email, isPrimary: true },
            ],
          },
          runIn(tx),
        )
      } catch (error) {
        if (isUniqueViolation(error)) {
          // A concurrent claim won the email between the ownership check and
          // the insert. Never create a second owner; surface a clear conflict.
          throw new PortalWriteError(
            'conflict',
            'This email already belongs to an existing person. Attach to that person instead.',
          )
        }
        throw error
      }
    }

    const persisted = await persistCanonicalWebsiteIntake(
      canonicalInput(receipt, personId, randomUUID()),
      runIn(tx),
      tx,
    )

    if (input.action.kind === 'attach') {
      // Record the intake email as a user_supplied person_identity on the
      // explicitly selected person. No-op when the email is already owned by
      // them (asserted above); on conflict (identity_type, identity_value)
      // keeps the DB unique backstop authoritative.
      await tx`
        insert into person_identity (
          person_id, identity_type, identity_value, source_system, is_primary
        )
        select ${personId}, 'email', ${receipt.email}, null,
          not exists (
            select 1 from person_identity
            where person_id = ${personId}
              and identity_type = 'email'
          )
        where not exists (
          select 1 from person_identity
          where person_id = ${personId}
            and identity_type = 'email'
            and identity_value = ${receipt.email}
        )
        on conflict (identity_type, identity_value) do nothing
      `
    }

    const done = await completeReceipt(
      tx,
      receipt.id,
      persisted.interactionId,
      actorAppUserId,
    )
    if (!done) {
      throw new PortalWriteError(
        'conflict',
        'Submission is already resolved or not actionable.',
      )
    }

    return {
      submissionId: receipt.id,
      status: 'completed',
      interactionId: persisted.interactionId,
      personId,
    }
  })
}
