// ---------------------------------------------------------------------------
// CRM-23 — Durable Integration Inbox: contracts.
//
// INBOX RESPONSIBILITY (architect brief): durable receipt of inbound external
// facts; stable external-source id; dedupe; replay; retry; status; bounded
// failure handling; poison/dead-letter or HumanRequired escalation;
// correlation; source provenance.
//
// SYMMETRY (architect brief): Integration Inbox = facts arriving from outside
// that must be deduped and processed safely. Transactional Outbox (CRM-14J,
// lib/events/outbox-contracts.ts) = committed internal facts that must be
// delivered outward. This module is the inbox half — MQ-like reliability on
// the inbound side without an external broker.
//
// PRIVACY / RETENTION (criterion 10): the inbox row stores ONLY the neutral
// business facts the CRM needs (participant identities, thread reference,
// content/provenance REFERENCES) — never raw payloads, bodies, tokens or
// credentials. Raw artifacts stay behind the observer adapter boundary and
// are referenced, not duplicated.
// ---------------------------------------------------------------------------

import type { CreateInteractionInput } from '../crm-types'
import type {
  ExternalActivityEvent,
  ExternalIdentity,
  SourceCapability,
} from '../mac-observer/contracts'

// ---------------------------------------------------------------------------
// Status vocabulary
// ---------------------------------------------------------------------------

export const INTEGRATION_INBOX_STATUSES = [
  'received',
  'processing',
  'completed',
  'rejected',
  'resolution_required',
  'duplicate',
  'poisoned',
] as const

export type IntegrationInboxStatus = (typeof INTEGRATION_INBOX_STATUSES)[number]

export type IntegrationInboxTerminalStatus = Exclude<
  IntegrationInboxStatus,
  'received' | 'processing'
>

// ---------------------------------------------------------------------------
// Durable record
// ---------------------------------------------------------------------------

/**
 * One durable integration-inbox row. `source + sourceAccount +
 * externalEventId` is the UNIQUE dedupe key (stable external-source id); a
 * replayed event inserts nothing and reads back the SAME record. The neutral
 * event essentials are persisted so replay/reprocessing needs no source
 * re-fetch; raw payloads are never stored (referenced via
 * provenanceReference/contentReference).
 */
export type IntegrationInboxRecord = {
  id: string
  source: string
  sourceAccount: string
  externalEventId: string
  eventType: string
  occurredAt: string
  observedAt: string
  direction: 'inbound' | 'outbound' | null
  correlationId: string | null
  threadId: string | null
  subject: string | null
  summary: string | null
  contentReference: string | null
  provenanceReference: string | null
  /** Neutral participant identities (bounded jsonb; never raw payloads). */
  participantIdentities: ExternalIdentity[]
  /** Neutral contact candidates (identity/contact resolution inputs). */
  contactCandidates: ExternalIdentity[] | null
  /** Attachment descriptors (references, not bytes). */
  attachmentMetadata: Array<{
    referenceId: string
    filename?: string
    mimeType?: string
    sizeBytes?: number
  }> | null
  status: IntegrationInboxStatus
  /** Attempts so far (bounded retry; poisoned after maxAttempts). */
  attemptCount: number
  maxAttempts: number
  lastError: string | null
  /** Claim token while status === 'processing'. */
  processingStartedAt: string | null
  processingCompletedAt: string | null
  /** Canonical person the event converged onto (completed only). */
  resolvedPersonId: string | null
  /** Canonical interaction the event produced (completed only). */
  interactionId: string | null
  createdAt: string
  updatedAt: string
}

export type InsertIntegrationInboxInput = {
  source: string
  sourceAccount: string
  externalEventId: string
  eventType: string
  occurredAt: string
  observedAt: string
  direction: ExternalActivityEvent['direction'] | null
  correlationId: string | null
  threadId: string | null
  subject: string | null
  summary: string | null
  contentReference: string | null
  provenanceReference: string | null
  participantIdentities: ExternalIdentity[]
  contactCandidates: ExternalIdentity[] | null
  attachmentMetadata: IntegrationInboxRecord['attachmentMetadata']
  maxAttempts: number
}

// ---------------------------------------------------------------------------
// Durability boundary (Postgres V1 transport; tests inject an in-memory fake)
// ---------------------------------------------------------------------------

export type TransitionIntegrationInboxInput = {
  receiptId: string
  claimToken: string
  from: IntegrationInboxStatus
  to: IntegrationInboxTerminalStatus
  interactionId?: string
  resolvedPersonId?: string
}

export type FailIntegrationInboxInput = {
  receiptId: string
  claimToken: string
  error: string
  /** When attempts >= maxAttempts the receipt goes 'poisoned', else retry. */
  attempts: number
  maxAttempts: number
}

/**
 * The durable boundary the inbox processor needs. Production implements it
 * over `integration_inbox` (migration 044, db/integration-inbox.ts); tests
 * inject an in-memory implementation.
 */
export interface IntegrationInboxDurability {
  /** Insert-or-read on the unique (source, sourceAccount, externalEventId). */
  insertOrReadReceipt(
    input: InsertIntegrationInboxInput,
  ): Promise<{ record: IntegrationInboxRecord; created: boolean }>
  /**
   * Claim: 'received' -> 'processing' with a fresh claim token; a 'processing'
   * receipt whose claim is stale (> 15 minutes — crash between claim and
   * transition) is re-claimed. A fresh in-flight claim returns null.
   */
  claimReceipt(receiptId: string): Promise<IntegrationInboxRecord | null>
  /** Terminal transition guarded by the claim token. */
  transitionReceipt(
    input: TransitionIntegrationInboxInput,
  ): Promise<boolean>
  /**
   * Bounded failure handling: 'processing' -> 'received' (retry later) with
   * attemptCount + 1, or -> 'poisoned' (dead-letter / HumanRequired
   * escalation) when attempts >= maxAttempts. Returns the updated record, or
   * null when the claim-token guard failed (in-flight elsewhere). Other
   * events are never blocked.
   */
  failReceipt(input: FailIntegrationInboxInput): Promise<IntegrationInboxRecord | null>
  /** Pending (received/processing-stale) receipts for a replay/retry worker. */
  listPending(limit: number): Promise<IntegrationInboxRecord[]>
  /** Poisoned/dead-lettered receipts for HumanRequired escalation. */
  listPoisoned(limit: number): Promise<IntegrationInboxRecord[]>
  /** The ONLY canonical CRM write: the interaction row (via the command seam). */
  persistInteraction(
    input: CreateInteractionInput,
  ): Promise<{ interactionId: string; created: boolean }>
}

// ---------------------------------------------------------------------------
// Processing outcomes
// ---------------------------------------------------------------------------

export type IntegrationInboxProcessingOutcome =
  | {
      outcome: 'completed'
      record: IntegrationInboxRecord
      interactionId?: string
      resolvedPersonId?: string
      created: boolean
    }
  | {
      outcome: 'duplicate'
      record: IntegrationInboxRecord
      existingInteractionId?: string
    }
  | { outcome: 'rejected'; record: IntegrationInboxRecord; reason: string }
  | {
      outcome: 'resolution_required'
      record: IntegrationInboxRecord
      reason: string
    }
  | { outcome: 'in_flight'; record: IntegrationInboxRecord }
  | {
      outcome: 'failed_retryable'
      record: IntegrationInboxRecord
      error: string
      attempts: number
    }
  | {
      outcome: 'poisoned'
      record: IntegrationInboxRecord
      error: string
      attempts: number
      escalated: true
    }
  | {
      outcome: 'skipped_unsupported'
      record?: IntegrationInboxRecord
      reason: string
    }

// ---------------------------------------------------------------------------
// Processing configuration
// ---------------------------------------------------------------------------

export type IntegrationInboxConfiguration = {
  /** Bounded retry ceiling before poison/dead-letter (default 3). */
  maxAttempts: number
  /**
   * Sources the processor is willing to accept (capability guard). Unproven /
   * unsupported sources are skipped honestly.
   */
  capabilities: Record<string, SourceCapability>
}
