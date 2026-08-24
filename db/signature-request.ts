import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import type {
  SignatureRequest,
  SignatureRequestStatus,
} from '../lib/signature/contracts'
import {
  SIGNATURE_REQUEST_STATUSES,
  SIGNATURE_REQUEST_TRANSITIONS,
  isSignatureRequestStatus,
} from '../lib/signature/contracts'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor, QueryRow } from './query-executor'
import {
  normalizeEmail,
  parseIssuedParticipants,
} from '../lib/agreements/participants'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from './workflow-command-receipt'

// ---------------------------------------------------------------------------
// Canonical signature request repository (migration 036, DOC-03).
//
// A signature request is the CANONICAL, provider-neutral record of a signing
// request against a transaction document (DOC-01). It stores ONLY the neutral
// status model (requested -> sent -> viewed -> signed -> completed, plus
// declined/voided/expired/error sinks); provider-specific ids/state live in a
// DOC-04 provider table behind the SignatureProvider seam and never reach this
// table (and never transaction_document — the FINAL signed outcome is reflected
// there only by DOC-05 reconciliation).
//
// State transitions reuse the claim-first command-receipt idempotency pattern
// (migration 018, db/workflow-command-receipt.ts) exactly like
// db/transaction-document.ts: the same commandId executes its effect at most
// once, and every caller observes the winner's stored result. A pending
// receipt is an in-flight claim, never a terminal outcome, and never mutates
// state.
//
// This service NEVER calls the provider (rejected design: synchronous provider
// calls inside a domain service). Provider observations arrive as NEUTRAL
// statuses (mapped at the seam in lib/signature) and are applied through
// applySignatureRequestStatus / cancel / decline.
// ---------------------------------------------------------------------------

export type SignatureRequestRow = QueryRow & {
  id: string
  transaction_document_id: string
  status: string
  message: string | null
  execution_role: string | null
  execution_slot_id: string | null
  created_by_user_id: string | null
  created_at: string
  updated_at: string
}

function dateOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return String(value)
}

function mapSignatureRequest(row: SignatureRequestRow): SignatureRequest {
  return {
    id: row.id,
    transactionDocumentId: row.transaction_document_id,
    status: row.status as SignatureRequestStatus,
    message: row.message ?? null,
    executionRole: row.execution_role ?? null,
    executionSlotId: row.execution_slot_id ?? null,
    createdByUserId: row.created_by_user_id ?? null,
    createdAt: dateOrNull(row.created_at) ?? '',
    updatedAt: dateOrNull(row.updated_at) ?? '',
  }
}

// ---------------------------------------------------------------------------
// Send (idempotent: one active request per transaction_document)
// ---------------------------------------------------------------------------

export type SendSignatureRequestInput = {
  commandId: string
  transactionDocumentId: string
  message?: string | null
  createdByUserId?: string | null
  /** CRM-27 — the agreement execution role this request fulfills (e.g. BUYER,
   *  SELLER). Optional/null for requests not tied to a role. Provider-neutral. */
  executionRole?: string | null
  /** CRM-27 — the ISSUED participant/signature slot this request satisfies
   *  (e.g. "BUYER:1"). Keyed to the immutable issued participant snapshot; a
   *  completed request proves completion of that exact issued slot. */
  executionSlotId?: string | null
  /** CRM-27 — the single intended recipient email for a slot-bound send. The
   *  canonical send boundary verifies it matches the immutable slot; the client
   *  is never authoritative for it. */
  slotRecipientEmail?: string | null
}

export type SendSignatureRequestResult = {
  signatureRequest: SignatureRequest
  /** True when an ALREADY-ACTIVE request was returned (duplicate send). */
  existing: boolean
}

/**
 * Record a provider-neutral signing request. Idempotent at TWO levels:
 *   1. claim-first receipt on commandId (a replayed send returns the winner's
 *      stored result);
 *   2. one active request per transaction_document — the partial unique index
 *      (migration 036) plus INSERT ... ON CONFLICT ... DO NOTHING + re-select
 *      mean a send for a document with an active request returns the existing
 *      request, never a duplicate (same pattern as createTransactionDocument).
 * No provider call happens here.
 */
export async function sendSignatureRequest(
  input: SendSignatureRequestInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (!input.transactionDocumentId.trim()) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'transactionDocumentId is required.',
      replayed: false,
    }
  }
  if (input.message != null && input.message.length > 500) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: null,
      message: 'message must be 500 characters or fewer.',
      replayed: false,
    }
  }

  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }

    let outcome: CommandOutcome = 'success'
    let aggregateId: string | null = null
    let message: string | null = null
    let value: SendSignatureRequestResult | undefined

    const docRows = input.executionSlotId
      ? await tx`
          select id, source_snapshot
          from transaction_document
          where id = ${input.transactionDocumentId}
          limit 1
        `
      : await tx`
          select id
          from transaction_document
          where id = ${input.transactionDocumentId}
          limit 1
        `
    const docRow = docRows[0] as
      | { id?: unknown; source_snapshot?: unknown }
      | undefined
    if (!docRows[0]) {
      outcome = 'not_found'
      message = 'Transaction document not found.'
    } else {
      // CRM-27 (canonical send-boundary enforcement): for a slot-bound request,
      // re-validate inside the canonical send service — the client is never
      // authoritative for role/slot/recipient. Resolve the supplied slot against
      // the EXACT issued document's immutable snapshot and reject an arbitrary or
      // cross-document slot label, a role mismatch, or a recipient mismatch.
      let proceed = true
      let slotRole: string | null = input.executionRole ?? null
      if (input.executionSlotId) {
        const parsed = parseIssuedParticipants(
          (docRow!.source_snapshot as { issuedParticipants?: unknown } | undefined)
            ?.issuedParticipants,
        )
        if (!parsed.ok) {
          proceed = false
          outcome = 'validation_failure'
          message = `Invalid issued-participant snapshot: ${parsed.error}`
        } else {
          const slot = parsed.slots.find((s) => s.slotId === input.executionSlotId)
          if (!slot) {
            proceed = false
            outcome = 'validation_failure'
            message = `Execution slot '${input.executionSlotId}' does not exist in document ${input.transactionDocumentId}.`
          } else if (input.executionRole && input.executionRole !== slot.role) {
            proceed = false
            outcome = 'validation_failure'
            message = `Execution role '${input.executionRole}' does not match slot '${slot.slotId}'.`
          } else if (
            input.slotRecipientEmail &&
            normalizeEmail(input.slotRecipientEmail) !== normalizeEmail(slot.email)
          ) {
            proceed = false
            outcome = 'validation_failure'
            message = 'Recipient does not match the immutable execution slot.'
          } else {
            slotRole = slot.role
          }
        }
      }

      if (proceed) {
        const rows = await tx`
          insert into signature_request (
            transaction_document_id, status, message, created_by_user_id, execution_role,
            execution_slot_id
          ) values (
            ${input.transactionDocumentId}, 'requested', ${input.message ?? null},
            ${input.createdByUserId ?? null}, ${slotRole},
            ${input.executionSlotId ?? null}
          )
        on conflict (transaction_document_id)
          where status in ('requested', 'sent', 'viewed', 'signed')
          do nothing
        returning id, transaction_document_id, status, message, created_by_user_id,
          execution_role, execution_slot_id, created_at, updated_at
      `
      const row = rows[0] as SignatureRequestRow | undefined
      if (row) {
        aggregateId = row.id
        value = { signatureRequest: mapSignatureRequest(row), existing: false }
      } else {
        // Duplicate send (or concurrent winner): return the existing ACTIVE
        // request — never a duplicate.
        const active = await tx`
          select id, transaction_document_id, status, message, created_by_user_id,
          execution_role, execution_slot_id, created_at, updated_at
          from signature_request
          where transaction_document_id = ${input.transactionDocumentId}
            and status in ('requested', 'sent', 'viewed', 'signed')
          order by created_at asc, id
          limit 1
        `
        const activeRow = active[0] as SignatureRequestRow | undefined
        if (!activeRow) {
          outcome = 'conflict'
          message = 'Could not record the signature request (active-request conflict).'
        } else if ((activeRow.execution_slot_id ?? null) !== (input.executionSlotId ?? null)) {
          outcome = 'conflict'
          message =
            'Another signature request is active for this document; complete or void it before sending a different execution slot.'
        } else {
          aggregateId = activeRow.id
          value = { signatureRequest: mapSignatureRequest(activeRow), existing: true }
        }
      }
      }
    }

    await finalizeReceipt(tx, input.commandId, outcome, aggregateId, message)

    return {
      commandId: input.commandId,
      outcome,
      emittedEvents: [],
      aggregateId,
      message,
      replayed: false,
      value,
    }
  })
}

// ---------------------------------------------------------------------------
// Neutral status application (the seam's status-mapping boundary)
// ---------------------------------------------------------------------------

export type ApplySignatureRequestStatusInput = {
  commandId: string
  signatureRequestId: string
  /**
   * Neutral target status observed at the seam (provider status already
   * mapped). Absent/null = read-only status (no transition).
   */
  targetStatus?: SignatureRequestStatus | null
}

export type ApplySignatureRequestStatusResult = {
  signatureRequest: SignatureRequest
  /** True when the status actually changed (vs. no-op or read). */
  transitioned: boolean
}

/**
 * Apply a neutral status to a signature request (from a provider status poll,
 * a normalized webhook, or an application action). Idempotent via the
 * claim-first receipt; re-applying the current status is a success no-op;
 * illegal transitions are rejected without mutation; the update is a
 * compare-and-set on the current status (concurrent change -> conflict).
 */
export async function applySignatureRequestStatus(
  input: ApplySignatureRequestStatusInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (input.targetStatus != null && !isSignatureRequestStatus(input.targetStatus)) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: input.signatureRequestId,
      message: `'${String(input.targetStatus)}' is not a valid neutral signature status.`,
      replayed: false,
    }
  }
  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }
    return transitionTo(tx, input, input.targetStatus ?? null)
  })
}

export type CancelSignatureRequestInput = {
  commandId: string
  signatureRequestId: string
}

/** Cancel a signature request -> neutral 'voided' (idempotent, receipt-backed). */
export async function cancelSignatureRequest(
  input: CancelSignatureRequestInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }
    return transitionTo(tx, input, 'voided')
  })
}

export type DeclineSignatureRequestInput = {
  commandId: string
  signatureRequestId: string
}

/** Record a recipient decline -> neutral 'declined' (idempotent, receipt-backed). */
export async function declineSignatureRequest(
  input: DeclineSignatureRequestInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  return run(async (tx) => {
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      const replay = replayOutcome(receipt)
      return {
        commandId: input.commandId,
        outcome: replay.outcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: replay.message,
        replayed: true,
      }
    }
    return transitionTo(tx, input, 'declined')
  })
}

type TransitionInput = {
  commandId: string
  signatureRequestId: string
}

/**
 * Shared transition core (called with the command already claimed): read the
 * request, validate the transition, compare-and-set, finalize the receipt.
 * `target` null means a read-only status application (no transition).
 */
async function transitionTo(
  tx: QueryExecutor,
  input: TransitionInput,
  target: SignatureRequestStatus | null,
): Promise<CommandResult> {
  let outcome: CommandOutcome = 'success'
  let aggregateId: string | null = null
  let message: string | null = null
  let value: ApplySignatureRequestStatusResult | undefined

  const curRows = await tx`
    select id, transaction_document_id, status, message, created_by_user_id,
      execution_role, execution_slot_id, created_at, updated_at
    from signature_request
    where id = ${input.signatureRequestId}
    limit 1
  `
  const current = curRows[0] as SignatureRequestRow | undefined
  if (!current) {
    outcome = 'not_found'
    message = 'Signature request not found.'
  } else {
    const from = current.status as SignatureRequestStatus
    if (target === null || target === from) {
      // Read-only, or re-application of the current status: success no-op.
      aggregateId = current.id
      value = {
        signatureRequest: mapSignatureRequest(current),
        transitioned: false,
      }
    } else if (!SIGNATURE_REQUEST_TRANSITIONS[from].includes(target)) {
      outcome = 'validation_failure'
      message = `Transition ${from} -> ${target} is not allowed.`
    } else {
      const updated = await tx`
        update signature_request
        set status = ${target}, updated_at = now()
        where id = ${input.signatureRequestId}
          and status = ${from}
        returning id, transaction_document_id, status, message, created_by_user_id,
          execution_role, execution_slot_id, created_at, updated_at
      `
      const updatedRow = updated[0] as SignatureRequestRow | undefined
      if (updatedRow) {
        aggregateId = updatedRow.id
        value = {
          signatureRequest: mapSignatureRequest(updatedRow),
          transitioned: true,
        }
      } else {
        outcome = 'conflict'
        message = `Signature request ${input.signatureRequestId} changed concurrently.`
      }
    }
  }

  await finalizeReceipt(tx, input.commandId, outcome, aggregateId, message)

  return {
    commandId: input.commandId,
    outcome,
    emittedEvents: [],
    aggregateId,
    message,
    replayed: false,
    value,
  }
}

// ---------------------------------------------------------------------------
// Reads (injectable executor, never write)
// ---------------------------------------------------------------------------

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export async function getSignatureRequest(
  id: string,
  execute?: QueryExecutor,
): Promise<SignatureRequest | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, transaction_document_id, status, message, created_by_user_id,
      execution_role, execution_slot_id, created_at, updated_at
    from signature_request
    where id = ${id}
    limit 1
  `
  const row = rows[0] as SignatureRequestRow | undefined
  return row ? mapSignatureRequest(row) : null
}

export async function getActiveSignatureRequestForDocument(
  transactionDocumentId: string,
  execute?: QueryExecutor,
): Promise<SignatureRequest | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, transaction_document_id, status, message, created_by_user_id,
      execution_role, execution_slot_id, created_at, updated_at
    from signature_request
    where transaction_document_id = ${transactionDocumentId}
      and status in ('requested', 'sent', 'viewed', 'signed')
    order by created_at asc, id
    limit 1
  `
  const row = rows[0] as SignatureRequestRow | undefined
  return row ? mapSignatureRequest(row) : null
}

export async function listSignatureRequestsByDocument(
  transactionDocumentId: string,
  execute?: QueryExecutor,
): Promise<SignatureRequest[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, transaction_document_id, status, message, created_by_user_id,
      execution_role, execution_slot_id, created_at, updated_at
    from signature_request
    where transaction_document_id = ${transactionDocumentId}
    order by created_at asc, id
  `
  return rows.map((row) => mapSignatureRequest(row as SignatureRequestRow))
}
