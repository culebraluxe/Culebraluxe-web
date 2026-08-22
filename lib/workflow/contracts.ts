// Pre-CRM-14 integration plumbing — provider-neutral contracts.
//
// CulebraLuxe owns canonical business truth and domain validation. The future
// workflow engine owns orchestration runtime. These types define the small seam
// between them. They carry no runtime behavior and no engine dependency.

// ---------------------------------------------------------------------------
// Domain event contract (Story 42)
// ---------------------------------------------------------------------------

export type DomainEventType =
  | 'DEAL_CREATED'
  | 'DEAL_STAGE_CHANGED'
  | 'OFFER_CREATED'
  | 'OFFER_COUNTERED'
  | 'OFFER_WITHDRAWN'
  | 'OFFER_REJECTED'
  | 'SHOWING_REQUESTED'
  | 'SHOWING_SCHEDULED'
  | 'SHOWING_COMPLETED'
  | 'SHOWING_CANCELLED'
  | 'TASK_CREATED'
  | 'TASK_COMPLETED'
  | 'TASK_CANCELLED'
  | 'INTERACTION_RECORDED'
  | 'PARTICIPANT_ADDED'
  | 'PARTICIPANT_ENDED'
  | 'PROPERTY_STATUS_CHANGED'
  // DOC-03 — Signature Provider Seam neutral events. Downstream consumers
  // (DOC-05 reconciliation) subscribe to these neutral events, never to
  // provider webhooks.
  | 'SIGNATURE_REQUEST_SENT'
  | 'SIGNATURE_REQUEST_COMPLETED'
  | 'SIGNATURE_REQUEST_DECLINED'
  | 'SIGNATURE_REQUEST_VOIDED'

export type AggregateType =
  | 'deal'
  | 'offer'
  | 'showing'
  | 'task'
  | 'interaction'
  | 'deal_participant'
  | 'property'
  | 'person'
  // DOC-03 — canonical signature_request aggregate (provider-free).
  | 'signature_request'
  // DOC-06 — canonical issued transaction document aggregate.
  | 'transaction_document'

export type DomainEvent = {
  eventId: string
  eventType: DomainEventType
  occurredAt: string // ISO UTC
  actorAppUserId: string | null
  aggregateType: AggregateType
  aggregateId: string
  correlationId: string | null
  causationId: string | null
  payload: Record<string, unknown>
}

// ---------------------------------------------------------------------------
// Command envelope (Story 44)
// ---------------------------------------------------------------------------

export type CommandType = string // stable machine identifier (see command inventory)

export type CommandEnvelope = {
  commandId: string
  commandType: CommandType
  actorAppUserId: string | null
  aggregateType: AggregateType
  aggregateId: string | null // null for create commands without a pre-existing id
  correlationId: string | null
  causationId: string | null
  requestedAt: string // ISO UTC
  input: Record<string, unknown>
}

// ---------------------------------------------------------------------------
// Command result contract (Story 45)
// ---------------------------------------------------------------------------

export type CommandOutcome =
  | 'success'
  | 'validation_failure'
  | 'not_found'
  | 'conflict'
  | 'unauthorized'
  | 'precondition_failure'

export type CommandResult = {
  commandId: string
  outcome: CommandOutcome
  emittedEvents: DomainEvent[]
  aggregateId: string | null
  message: string | null
  replayed: boolean // true when an idempotent command was recognized as a replay
  // -------------------------------------------------------------------------
  // CRM-14J canonical command layer — additive normalized-result fields. All
  // optional so every existing producer keeps compiling; the canonical
  // dispatcher (lib/commands) fills them for the shared seam.
  // -------------------------------------------------------------------------
  /** Structured success payload carried by a canonical command handler. */
  value?: unknown
  /** Structured error detail: stable code + message + retryability hint. */
  error?: { code: string; message: string; retryable?: boolean }
  /** Durable receipt identity when this execution wrote a command receipt. */
  receiptId?: string
}

// ---------------------------------------------------------------------------
// Conflict contract (Story 49) — application-level optimistic/concurrency
// ---------------------------------------------------------------------------

export type ConflictReason =
  | 'state_changed'
  | 'already_completed'
  | 'already_cancelled'
  | 'parent_offer_not_actionable'
  | 'participant_already_ended'
  | 'showing_already_completed'
  | 'property_status_not_editable'

// ---------------------------------------------------------------------------
// Workflow subject contract (Story 55)
// ---------------------------------------------------------------------------

export type WorkflowSubjectType = 'deal' | 'property' | 'person'

export type WorkflowSubject = {
  subjectType: WorkflowSubjectType
  subjectId: string
}

// ---------------------------------------------------------------------------
// Correlation / causation readiness (Story 47)
// ---------------------------------------------------------------------------
// Existing idempotency/correlation surfaces are interaction(source_system,
// source_external_id) and website_intake_submission.status + interaction_id.
// The minimal correlation chain is:
//
//   workflow instance id → CommandEnvelope.correlationId
//   CommandEnvelope.commandId → CommandResult.emittedEvents[].causationId
//   DomainEvent.eventId → DomainEvent.correlationId / .causationId
//
// No new columns are required for the initial seam; the engine retains its own
// instance/execution ids and passes them through these string fields.
