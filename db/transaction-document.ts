import { PortalWriteError } from '../lib/portal-write-error'
import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor, QueryRow } from './query-executor'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from './workflow-command-receipt'

// ---------------------------------------------------------------------------
// Canonical transaction document repository (migration 027, DOC-01).
//
// A transaction document is a DEAL-scoped record that classifies a document,
// tracks its lifecycle state, ownership and source/provenance, and preserves
// signed-artifact lineage. File bytes live in the generic `media` asset store;
// this table references media.id (draft/current) and signed_media_id (signed
// artifact — a NEW media row, never a mutation of the draft bytes).
//
// Provider-specific signing concerns (BoldSign) are out of scope here —
// DOC-03/DOC-04 own them behind a provider-neutral seam later.
//
// State transitions reuse the claim-first command-receipt idempotency pattern
// (migration 018, db/workflow-command-receipt.ts) exactly like
// db/deal-stage.ts setDealStage: the same commandId executes its effect at
// most once, and every caller observes the winner's stored result. A pending
// receipt is an in-flight claim, never a terminal outcome, and never mutates
// state.
// ---------------------------------------------------------------------------

export const TRANSACTION_DOCUMENT_TYPES = [
  'agreement',
  'addendum',
  'disclosure',
  'title',
  'financing',
  'inspection',
  'appraisal',
  'closing',
  'other',
] as const
export type TransactionDocumentType = (typeof TRANSACTION_DOCUMENT_TYPES)[number]

export const TRANSACTION_DOCUMENT_STATES = [
  'draft',
  'ready',
  'sent',
  'signed',
  'voided',
  'superseded',
] as const
export type TransactionDocumentState = (typeof TRANSACTION_DOCUMENT_STATES)[number]

export const TRANSACTION_DOCUMENT_SOURCES = [
  'upload',
  'generated',
  'imported',
  'provider',
] as const
export type TransactionDocumentSource = (typeof TRANSACTION_DOCUMENT_SOURCES)[number]

export type TransactionDocument = {
  id: string
  dealId: string
  documentType: TransactionDocumentType
  documentTypeLabel: string | null
  title: string | null
  state: TransactionDocumentState
  source: TransactionDocumentSource
  sourceSystem: string | null
  sourceExternalId: string | null
  preparedByUserId: string | null
  partyPersonId: string | null
  mediaId: string | null
  signedMediaId: string | null
  signedAt: string | null
  supersedesDocumentId: string | null
  createdAt: string
  updatedAt: string
}

type TransactionDocumentRow = QueryRow & {
  id: string
  deal_id: string
  document_type: string
  document_type_label: string | null
  title: string | null
  state: string
  source: string
  source_system: string | null
  source_external_id: string | null
  prepared_by_user_id: string | null
  party_person_id: string | null
  media_id: string | null
  signed_media_id: string | null
  signed_at: string | null
  supersedes_document_id: string | null
  created_at: string
  updated_at: string
}

// NOTE: column lists are written literally (the Neon driver parameterizes
// interpolated strings — a `select ${cols}` would become `select $1`).

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function dateOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return String(value)
}

function mapTransactionDocument(row: TransactionDocumentRow): TransactionDocument {
  return {
    id: row.id,
    dealId: row.deal_id,
    documentType: row.document_type as TransactionDocumentType,
    documentTypeLabel: row.document_type_label ?? null,
    title: row.title ?? null,
    state: row.state as TransactionDocumentState,
    source: row.source as TransactionDocumentSource,
    sourceSystem: row.source_system ?? null,
    sourceExternalId: row.source_external_id ?? null,
    preparedByUserId: row.prepared_by_user_id ?? null,
    partyPersonId: row.party_person_id ?? null,
    mediaId: row.media_id ?? null,
    signedMediaId: row.signed_media_id ?? null,
    signedAt: dateOrNull(row.signed_at),
    supersedesDocumentId: row.supersedes_document_id ?? null,
    createdAt: dateOrNull(row.created_at) ?? '',
    updatedAt: dateOrNull(row.updated_at) ?? '',
  }
}

function validateSignedPair(
  signedMediaId: string | null | undefined,
  signedAt: string | null | undefined,
  where: string,
): void {
  const hasMedia = signedMediaId != null
  const hasDate = signedAt != null
  if (hasMedia !== hasDate) {
    throw new PortalWriteError(
      'validation',
      `${where}: signed_media_id and signed_at must be set together.`,
    )
  }
}

export type CreateTransactionDocumentInput = {
  dealId: string
  documentType: string
  documentTypeLabel?: string | null
  title?: string | null
  state?: string
  source: string
  sourceSystem?: string | null
  sourceExternalId?: string | null
  preparedByUserId?: string | null
  partyPersonId?: string | null
  mediaId?: string | null
  signedMediaId?: string | null
  signedAt?: string | null
  supersedesDocumentId?: string | null
}

/**
 * Create a transaction document for a deal. Idempotent for externally-sourced
 * documents: the unique partial index (deal_id, source_system,
 * source_external_id) where source_external_id is not null means a repeated
 * create with the same external source returns the existing row.
 */
export async function createTransactionDocument(
  input: CreateTransactionDocumentInput,
  execute?: QueryExecutor,
): Promise<TransactionDocument> {
  if (!input.dealId.trim()) {
    throw new PortalWriteError('validation', 'dealId is required.')
  }
  if (!TRANSACTION_DOCUMENT_TYPES.includes(input.documentType as TransactionDocumentType)) {
    throw new PortalWriteError(
      'validation',
      `documentType must be one of: ${TRANSACTION_DOCUMENT_TYPES.join(', ')}.`,
    )
  }
  const state = input.state ?? 'draft'
  if (!TRANSACTION_DOCUMENT_STATES.includes(state as TransactionDocumentState)) {
    throw new PortalWriteError(
      'validation',
      `state must be one of: ${TRANSACTION_DOCUMENT_STATES.join(', ')}.`,
    )
  }
  if (!TRANSACTION_DOCUMENT_SOURCES.includes(input.source as TransactionDocumentSource)) {
    throw new PortalWriteError(
      'validation',
      `source must be one of: ${TRANSACTION_DOCUMENT_SOURCES.join(', ')}.`,
    )
  }
  if (input.documentTypeLabel != null && input.documentTypeLabel.length > 120) {
    throw new PortalWriteError(
      'validation',
      'documentTypeLabel must be 120 characters or fewer.',
    )
  }
  validateSignedPair(input.signedMediaId, input.signedAt, 'createTransactionDocument')

  const q = execute ?? (await executor())
  const rows = await q`
    insert into transaction_document (
      deal_id, document_type, document_type_label, title, state, source,
      source_system, source_external_id, prepared_by_user_id, party_person_id,
      media_id, signed_media_id, signed_at, supersedes_document_id
    ) values (
      ${input.dealId}, ${input.documentType}, ${input.documentTypeLabel ?? null},
      ${input.title ?? null}, ${state}, ${input.source},
      ${input.sourceSystem ?? null}, ${input.sourceExternalId ?? null},
      ${input.preparedByUserId ?? null}, ${input.partyPersonId ?? null},
      ${input.mediaId ?? null}, ${input.signedMediaId ?? null},
      ${input.signedAt ?? null}, ${input.supersedesDocumentId ?? null}
    )
    on conflict (deal_id, source_system, source_external_id)
      where source_external_id is not null
      do nothing
    returning id, deal_id, document_type, document_type_label, title, state,
      source, source_system, source_external_id, prepared_by_user_id,
      party_person_id, media_id, signed_media_id, signed_at,
      supersedes_document_id, created_at, updated_at
  `
  const row = rows[0] as TransactionDocumentRow | undefined
  if (row) return mapTransactionDocument(row)

  // Source-idempotent hit: return the existing externally-sourced row.
  const existing = await q`
    select id, deal_id, document_type, document_type_label, title, state,
      source, source_system, source_external_id, prepared_by_user_id,
      party_person_id, media_id, signed_media_id, signed_at,
      supersedes_document_id, created_at, updated_at
    from transaction_document
    where deal_id = ${input.dealId}
      and source_system = ${input.sourceSystem ?? null}
      and source_external_id = ${input.sourceExternalId ?? null}
    order by created_at asc, id
    limit 1
  `
  const existingRow = existing[0] as TransactionDocumentRow | undefined
  if (!existingRow) {
    throw new PortalWriteError(
      'conflict',
      'Could not create transaction document (source idempotency conflict).',
    )
  }
  return mapTransactionDocument(existingRow)
}


export type TransitionTransactionDocumentInput = {
  commandId: string
  to: string
  signedMediaId?: string | null
  signedAt?: string | null
}

/** Allowed transitions between canonical document states. */
export const TRANSACTION_DOCUMENT_TRANSITIONS: Record<
  TransactionDocumentState,
  readonly TransactionDocumentState[]
> = {
  draft: ['ready', 'voided', 'superseded'],
  ready: ['sent', 'voided', 'superseded'],
  sent: ['signed', 'voided'],
  signed: ['voided', 'superseded'],
  voided: [],
  superseded: [],
}

/**
 * Transition a document's lifecycle state with claim-first command-receipt
 * idempotency (same pattern as db/deal-stage.ts setDealStage). The 'signed'
 * transition requires the signed artifact lineage (a NEW media row) so the
 * draft bytes are never mutated.
 */
export async function transitionTransactionDocumentState(
  documentId: string,
  input: TransitionTransactionDocumentInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  if (!TRANSACTION_DOCUMENT_STATES.includes(input.to as TransactionDocumentState)) {
    return {
      commandId: input.commandId,
      outcome: 'validation_failure',
      emittedEvents: [],
      aggregateId: documentId,
      message: `State '${input.to}' is not a valid document state.`,
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
    let aggregateId: string | null = documentId
    let message: string | null = null

    const curRows = await tx`
      select id, state from transaction_document where id = ${documentId} limit 1
    `
    const current = curRows[0] as { state?: string } | undefined
    if (!current) {
      outcome = 'not_found'
      aggregateId = null
      message = 'Transaction document not found.'
    } else {
      const from = current.state as TransactionDocumentState
      const allowed = TRANSACTION_DOCUMENT_TRANSITIONS[from]
      if (!allowed.includes(input.to as TransactionDocumentState)) {
        outcome = 'validation_failure'
        aggregateId = null
        message = `Transition ${from} -> ${input.to} is not allowed.`
      } else if (input.to === 'signed') {
        if (input.signedMediaId == null || input.signedAt == null) {
          outcome = 'validation_failure'
          aggregateId = null
          message = 'Signing requires signedMediaId and signedAt (a NEW media row for the signed artifact).'
        }
      }
    }

    if (outcome === 'success') {
      const updated = await tx`
        update transaction_document
        set state = ${input.to},
            signed_media_id = case when ${input.to} = 'signed'
              then ${input.signedMediaId ?? null} else signed_media_id end,
            signed_at = case when ${input.to} = 'signed'
              then ${input.signedAt ?? null} else signed_at end,
            updated_at = now()
        where id = ${documentId}
          and state = ${current?.state ?? ''}
        returning id
      `
      if (!updated[0]) {
        outcome = 'conflict'
        aggregateId = null
        message = `State changed concurrently for document ${documentId}.`
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
    }
  })
}

export async function listTransactionDocumentsByDeal(
  dealId: string,
  execute?: QueryExecutor,
): Promise<TransactionDocument[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, deal_id, document_type, document_type_label, title, state,
      source, source_system, source_external_id, prepared_by_user_id,
      party_person_id, media_id, signed_media_id, signed_at,
      supersedes_document_id, created_at, updated_at
    from transaction_document
    where deal_id = ${dealId}
    order by created_at asc, id
  `
  return rows.map((row) => mapTransactionDocument(row as TransactionDocumentRow))
}

export async function getTransactionDocument(
  id: string,
  execute?: QueryExecutor,
): Promise<TransactionDocument | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, deal_id, document_type, document_type_label, title, state,
      source, source_system, source_external_id, prepared_by_user_id,
      party_person_id, media_id, signed_media_id, signed_at,
      supersedes_document_id, created_at, updated_at
    from transaction_document
    where id = ${id}
  `
  const row = rows[0] as TransactionDocumentRow | undefined
  return row ? mapTransactionDocument(row) : null
}

