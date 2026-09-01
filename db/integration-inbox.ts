import type { QueryExecutor } from './query-executor'
import type { CreateInteractionInput } from '../lib/crm-types'
import type {
  InsertIntegrationInboxInput,
  IntegrationInboxDurability,
  IntegrationInboxRecord,
  IntegrationInboxStatus,
  IntegrationInboxTerminalStatus,
} from '../lib/integration-inbox/contracts'
import type { ExternalIdentity } from '../lib/mac-observer/contracts'

// ---------------------------------------------------------------------------
// Integration inbox repository (migration 044, CRM-23).
//
// Insert-or-read idempotency mirrors calendar_intake_receipt (CRM-08) and
// website_intake_submission (CRM-04): UNIQUE (source, source_account,
// external_event_id) is the replay dedupe key — a replayed external event
// inserts nothing and reads back the SAME receipt.
//
// Lifecycle:
//   insertOrRead  -> status 'received'
//   claimReceipt  -> 'received' -> 'processing' (or re-claims a 'processing'
//                    receipt whose claim is stale, > 15 minutes old)
//   transitionReceipt -> 'processing' -> 'completed' (interaction_id or
//                    resolved_person_id required) | 'rejected' |
//                    'resolution_required' | 'duplicate'
//   failReceipt   -> 'processing' -> 'received' (attempt_count + 1, retry) or
//                    -> 'poisoned' (dead-letter / HumanRequired escalation)
//                    when attempts >= max_attempts — bounded failure handling
//                    that never blocks other events.
//
// These functions never call observers or providers (rejected design) — the
// processor (lib/integration-inbox/processor.ts) composes observation with
// these writes. Only NEUTRAL event facts are stored: no raw payloads, bodies,
// tokens, or credentials (privacy/retention criterion 10).
//
// NOTE (Neon gotcha, docs/DEV_DATABASE.md): column lists are written LITERALLY
// in every query — interpolating a column-list constant would be
// parameterized as a value, not expanded as columns.
// ---------------------------------------------------------------------------

type InboxRow = {
  id: string
  source: string
  source_account: string
  external_event_id: string
  event_type: string
  occurred_at: string
  observed_at: string
  direction: 'inbound' | 'outbound' | null
  correlation_id: string | null
  thread_id: string | null
  subject: string | null
  summary: string | null
  content_reference: string | null
  provenance_reference: string | null
  participant_identities: unknown
  contact_candidates: unknown
  attachment_metadata: unknown
  status: IntegrationInboxStatus
  attempt_count: number
  max_attempts: number
  last_error: string | null
  processing_started_at: string | null
  processing_completed_at: string | null
  resolved_person_id: string | null
  interaction_id: string | null
  created_at: string
  updated_at: string
}

function parseIdentities(value: unknown): ExternalIdentity[] {
  return Array.isArray(value) ? (value as ExternalIdentity[]) : []
}

function mapRecord(row: InboxRow): IntegrationInboxRecord {
  return {
    id: row.id,
    source: row.source,
    sourceAccount: row.source_account,
    externalEventId: row.external_event_id,
    eventType: row.event_type,
    occurredAt: row.occurred_at,
    observedAt: row.observed_at,
    direction: row.direction,
    correlationId: row.correlation_id,
    threadId: row.thread_id,
    subject: row.subject,
    summary: row.summary,
    contentReference: row.content_reference,
    provenanceReference: row.provenance_reference,
    participantIdentities: parseIdentities(row.participant_identities),
    contactCandidates: row.contact_candidates
      ? (row.contact_candidates as ExternalIdentity[])
      : null,
    attachmentMetadata: row.attachment_metadata
      ? (row.attachment_metadata as IntegrationInboxRecord['attachmentMetadata'])
      : null,
    status: row.status,
    attemptCount: row.attempt_count,
    maxAttempts: row.max_attempts,
    lastError: row.last_error,
    processingStartedAt: row.processing_started_at,
    processingCompletedAt: row.processing_completed_at,
    resolvedPersonId: row.resolved_person_id,
    interactionId: row.interaction_id,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

/** Insert-or-read on the unique (source, source_account, external_event_id). */
export async function insertOrReadIntegrationInbox(
  input: InsertIntegrationInboxInput,
  execute: QueryExecutor,
): Promise<{ record: IntegrationInboxRecord; created: boolean }> {
  const inserted = await execute`
    insert into integration_inbox (
      source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      max_attempts
    ) values (
      ${input.source}, ${input.sourceAccount}, ${input.externalEventId},
      ${input.eventType}, ${input.occurredAt}, ${input.observedAt},
      ${input.direction}, ${input.correlationId}, ${input.threadId},
      ${input.subject}, ${input.summary}, ${input.contentReference},
      ${input.provenanceReference},
      ${JSON.stringify(input.participantIdentities)}::jsonb,
      ${input.contactCandidates ? JSON.stringify(input.contactCandidates) : null}::jsonb,
      ${input.attachmentMetadata ? JSON.stringify(input.attachmentMetadata) : null}::jsonb,
      ${input.maxAttempts}
    )
    on conflict (source, source_account, external_event_id) do nothing
    returning id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
  `
  const created = inserted[0] as InboxRow | undefined
  if (created) return { record: mapRecord(created), created: true }

  const existing = await execute`
    select id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
    from integration_inbox
    where source = ${input.source}
      and source_account = ${input.sourceAccount}
      and external_event_id = ${input.externalEventId}
    limit 1
  `
  const row = existing[0] as InboxRow | undefined
  if (!row) {
    throw new Error('Integration inbox receipt could not be resolved.')
  }
  return { record: mapRecord(row), created: false }
}

/**
 * Claim a receipt for processing: 'received' -> 'processing' with a fresh
 * claim token; a 'processing' receipt whose claim is older than 15 minutes
 * (crash between claim and transition) is re-claimed. A fresh in-flight claim
 * returns null — the caller treats it as in-flight and does no work.
 */
export async function claimIntegrationInbox(
  receiptId: string,
  execute: QueryExecutor,
): Promise<IntegrationInboxRecord | null> {
  const rows = await execute`
    update integration_inbox
    set status = 'processing',
        processing_started_at = date_trunc('milliseconds', now()),
        updated_at = now()
    where id = ${receiptId}
      and (
        status = 'received'
        or (status = 'processing'
          and processing_started_at <= now() - interval '15 minutes')
      )
    returning id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
  `
  const row = rows[0] as InboxRow | undefined
  return row ? mapRecord(row) : null
}

export type TransitionIntegrationInboxInput = {
  receiptId: string
  claimToken: string
  from: IntegrationInboxStatus
  to: IntegrationInboxTerminalStatus
  interactionId?: string
  resolvedPersonId?: string
}

const allowedTransitions = new Set([
  'processing:completed',
  'processing:rejected',
  'processing:resolution_required',
  'processing:duplicate',
])

/** Terminal transition guarded by the claim token (mirrors CRM-04/CRM-08). */
export async function transitionIntegrationInbox(
  input: TransitionIntegrationInboxInput,
  execute: QueryExecutor,
): Promise<boolean> {
  if (!allowedTransitions.has(`${input.from}:${input.to}`)) {
    throw new Error('Integration inbox transition is not allowed.')
  }
  if (
    input.to === 'completed' &&
    !input.interactionId &&
    !input.resolvedPersonId
  ) {
    throw new Error('A completed inbox receipt needs an interaction or person.')
  }
  if (
    input.to !== 'completed' &&
    (input.interactionId || input.resolvedPersonId)
  ) {
    throw new Error('Only a completed inbox receipt may carry convergence ids.')
  }

  const rows = await execute`
    update integration_inbox
    set status = ${input.to},
        processing_started_at = null,
        processing_completed_at = now(),
        interaction_id = ${input.interactionId ?? null},
        resolved_person_id = ${input.resolvedPersonId ?? null},
        updated_at = now()
    where id = ${input.receiptId}
      and status = ${input.from}
      and processing_started_at = ${input.claimToken}
    returning id
  `
  return rows.length === 1
}

export type FailIntegrationInboxInput = {
  receiptId: string
  claimToken: string
  error: string
  attempts: number
  maxAttempts: number
}

/**
 * Bounded failure handling: 'processing' -> 'received' (retry later) with
 * attempt_count + 1, or -> 'poisoned' (dead-letter / HumanRequired
 * escalation) when attempts >= maxAttempts. Returns the updated record, or
 * null when the claim-token guard failed (in-flight elsewhere).
 */
export async function failIntegrationInbox(
  input: FailIntegrationInboxInput,
  execute: QueryExecutor,
): Promise<IntegrationInboxRecord | null> {
  const rows = await execute`
    update integration_inbox
    set status = case
          when ${input.attempts} >= ${input.maxAttempts} then 'poisoned'
          else 'received'
        end,
        attempt_count = ${input.attempts},
        last_error = ${input.error},
        processing_started_at = null,
        processing_completed_at = case
          when ${input.attempts} >= ${input.maxAttempts} then now()
          else null
        end,
        updated_at = now()
    where id = ${input.receiptId}
      and status = 'processing'
      and processing_started_at = ${input.claimToken}
    returning id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
  `
  const row = rows[0] as InboxRow | undefined
  return row ? mapRecord(row) : null
}

/** Pending (retryable) receipts oldest-first for a replay/retry worker. */
export async function listPendingIntegrationInbox(
  limit: number,
  execute: QueryExecutor,
): Promise<IntegrationInboxRecord[]> {
  const rows = await execute`
    select id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
    from integration_inbox
    where status = 'received'
    order by created_at asc
    limit ${limit}
  `
  return (rows as InboxRow[]).map(mapRecord)
}

/** Poisoned (dead-lettered) receipts for HumanRequired escalation. */
export async function listPoisonedIntegrationInbox(
  limit: number,
  execute: QueryExecutor,
): Promise<IntegrationInboxRecord[]> {
  const rows = await execute`
    select id, source, source_account, external_event_id, event_type,
      occurred_at, observed_at, direction, correlation_id, thread_id,
      subject, summary, content_reference, provenance_reference,
      participant_identities, contact_candidates, attachment_metadata,
      status, attempt_count, max_attempts, last_error,
      processing_started_at, processing_completed_at,
      resolved_person_id, interaction_id, created_at, updated_at
    from integration_inbox
    where status = 'poisoned'
    order by updated_at desc
    limit ${limit}
  `
  return (rows as InboxRow[]).map(mapRecord)
}

// ---------------------------------------------------------------------------
// Durability factory — wires this repository (plus the canonical interaction
// insert) into the processor's IntegrationInboxDurability shape.
// ---------------------------------------------------------------------------

export function createIntegrationInboxDurability(
  execute: QueryExecutor,
  persistInteraction: (
    input: CreateInteractionInput,
  ) => Promise<{ interactionId: string; created: boolean }>,
): IntegrationInboxDurability {
  return {
    insertOrReadReceipt: (input) => insertOrReadIntegrationInbox(input, execute),
    claimReceipt: (receiptId) => claimIntegrationInbox(receiptId, execute),
    transitionReceipt: (input) =>
      transitionIntegrationInbox(
        {
          receiptId: input.receiptId,
          claimToken: input.claimToken,
          from: input.from,
          to: input.to,
          interactionId: input.interactionId,
          resolvedPersonId: input.resolvedPersonId,
        },
        execute,
      ),
    failReceipt: (input) =>
      failIntegrationInbox(
        {
          receiptId: input.receiptId,
          claimToken: input.claimToken,
          error: input.error,
          attempts: input.attempts,
          maxAttempts: input.maxAttempts,
        },
        execute,
      ),
    listPending: (limit) => listPendingIntegrationInbox(limit, execute),
    listPoisoned: (limit) => listPoisonedIntegrationInbox(limit, execute),
    persistInteraction,
  }
}
