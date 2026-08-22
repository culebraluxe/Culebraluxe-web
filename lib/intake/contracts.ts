// ---------------------------------------------------------------------------
// INTAKE-01 — Canonical Intake Message Contract.
//
// ONE normalized intake message; two acquisition lanes; one transformation
// stack.
//
//   source-specific adapter          -> CanonicalIntakeMessage  (this module)
//   CanonicalIntakeMessage           -> durable intake transport/inbox
//                                        (lib/intake/inbox.ts -> integration
//                                        inbox, migration 044)
//   durable inbox row                -> identity resolution (existing CRM
//                                        intake stubs — person/context)
//   resolved intent                  -> BusinessCommand
//                                        (interaction.record, lib/commands)
//   BusinessCommand                  -> canonical CRM truth (interaction row)
//
// The REAL-TIME lane (mac-observer adapters) and the BATCH lane (import
// adapters) differ ONLY at the edge: each edge has its own adapter, and every
// adapter lowers into THIS envelope. Downstream code (durable inbox, identity
// resolution, Business Command layer) never parses source-specific fields —
// it consumes the neutral envelope and nothing else.
//
// The envelope carries the architect-brief fields:
//   - stable source system                         source.system
//   - source item/event id                         source.itemId
//   - batch/import id when relevant                source.batchId
//   - occurred_at                                  occurredAt
//   - participant/source identities                participants / contactCandidates
//   - event type                                   eventType
//   - raw/provenance reference                     provenance.rawReference
//   - correlation/causation metadata               correlationId / causationId
//   - bounded source-specific payload              sourcePayload (opaque, bounded)
//
// DUPLICATE / REPLAY IDENTITY: `intakeDedupeKey` = (source.system,
// source.itemId) — the canonical replay identity. It maps onto the canonical
// interaction idempotency key (source_system, source_external_id) and onto
// the durable inbox key (source, source_account, external_event_id): the
// realtime lane scopes by account namespace (source.account), the batch lane
// does not (account is empty, so the durable key is exactly the canonical
// (system, itemId) — re-running an import with ANY import id replays, never
// duplicates). The batch/import id is provenance, never identity.
//
// PROVENANCE / RAW SOURCE OWNERSHIP: the adapter that lowered the raw
// observation owns the raw artifact. The envelope carries only
// provenance.adapter/adapterVersion/rawReference; canonical CRM never stores
// raw payloads (privacy/retention policy — same rule as CRM-23). A batch
// adapter may attach a BOUNDED opaque sourcePayload (import-row extras) that
// is validated to ≤ 32 KB and is never parsed by downstream code.
//
// NO NEW CANONICAL CRM STATE MODEL: this module defines a message contract
// only — no SQL, no table, no new durable state. Canonical CRM truth stays
// the existing `interaction` row reached through the existing integration
// inbox + intake stubs + Business Command layer.
// ---------------------------------------------------------------------------

import type { JsonObject } from '../crm-types'

/** The canonical envelope version. Adapters emit exactly this version. */
export const INTAKE_MESSAGE_SCHEMA_VERSION = 1 as const

/**
 * The two allowed acquisition lanes. Batch and realtime differ only at the
 * edge (their adapters); everything downstream is shared. A future lane gets
 * its own adapter and still emits this same envelope.
 */
export type IntakeAcquisitionLane = 'realtime' | 'batch'

/** Identity kinds the envelope may carry (source-neutral, mirror of the
 *  existing ExternalIdentityKind so the durable inbox projection stays
 *  structurally identical for both lanes). */
export type IntakeIdentityKind =
  | 'email'
  | 'phone'
  | 'contact'
  | 'apple_account'
  | 'whatsapp'
  | 'external'

/** One neutral identity on the envelope (never a raw provider object). */
export type IntakeIdentity = {
  kind: IntakeIdentityKind
  value: string
  displayName?: string
}

/** A participant with its transport role on the fact. */
export type IntakeParticipantRole =
  | 'sender'
  | 'recipient'
  | 'organizer'
  | 'attendee'
  | 'subject'
  | 'contact'

export type IntakeParticipant = IntakeIdentity & {
  role: IntakeParticipantRole
}

/** Attachment descriptor — a reference, never bytes. */
export type IntakeAttachment = {
  /** Stable provider/import-side reference to the artifact. */
  referenceId: string
  filename?: string
  mimeType?: string
  sizeBytes?: number
}

/** Content essentials + body REFERENCE (never the raw body itself). */
export type IntakeContent = {
  subject?: string
  summary?: string
  contentReference?: string
}

/** Thread / conversation reference where the source expresses one. */
export type IntakeThread = {
  id?: string
  conversationId?: string
  inReplyTo?: string
}

/** Trusted business context ONLY when the source asserts it (exact match). */
export type IntakeContext = {
  propertyId?: string
  propertySlug?: string
  propertyUrl?: string
  dealId?: string
}

/**
 * Raw/provenance ownership. `adapter` + `adapterVersion` name the lowering
 * adapter (who owns translation of the raw source); `rawReference` points at
 * the bounded, revocable raw artifact — never the payload itself.
 */
export type IntakeProvenance = {
  rawReference?: string
  adapter: string
  adapterVersion: string
}

/** Stable source identity on the envelope. */
export type IntakeSource = {
  /** Stable source system (realtime: 'calendar'|'mail'|…; batch:
   *  e.g. 'import:csv:contacts-v1'). Dedupe axis 1. */
  system: string
  /** Stable source item/event id within the system. Dedupe axis 2. */
  itemId: string
  /** Source account namespace (realtime lane; per-account scoping). */
  account?: string
  /** Batch/import id when the lane is batch (or a realtime bulk replay). */
  batchId?: string
}

/**
 * THE canonical normalized intake message. Every acquisition lane (realtime
 * observers, batch imports, future edges) emits exactly this envelope; no
 * source-specific field may leak into it as a required downstream input.
 * `sourcePayload` is the ONLY source-specific surface and it is bounded,
 * optional and opaque — downstream never parses it.
 */
export interface CanonicalIntakeMessage {
  schemaVersion: typeof INTAKE_MESSAGE_SCHEMA_VERSION
  /** Which acquisition lane produced this message. */
  acquisitionLane: IntakeAcquisitionLane
  source: IntakeSource
  /** Source-typed event kind, e.g. 'calendar.event_created', 'contact.imported'. */
  eventType: string
  /** When the fact happened at the source (ISO UTC). */
  occurredAt: string
  /** When the edge acquired/imported the fact (ISO UTC). */
  observedAt?: string
  /** Transport direction when the source expresses one. */
  direction?: 'inbound' | 'outbound'
  /** Participant identities with transport roles. */
  participants: IntakeParticipant[]
  /** Candidate canonical contact identities (identity-resolution inputs). */
  contactCandidates?: IntakeIdentity[]
  thread?: IntakeThread
  content?: IntakeContent
  attachments?: IntakeAttachment[]
  context?: IntakeContext
  /** Correlation metadata (join this fact to a business flow if applicable). */
  correlationId?: string
  /** Causation metadata (the prior fact/command this message answers, if any). */
  causationId?: string
  provenance: IntakeProvenance
  /**
   * BOUNDED, OPAQUE source-specific payload (batch import-row extras, e.g. a
   * normalized row's remaining columns). Validated to ≤
   * INTAKE_SOURCE_PAYLOAD_MAX_BYTES. Downstream code never reads it.
   */
  sourcePayload?: JsonObject
}

/**
 * Bounded source-specific payload ceiling. Matches the existing 32 KB
 * metadata bound applied on every other intake path (CRM-23 policy), so batch
 * import extras obey the same retention discipline as realtime metadata.
 */
export const INTAKE_SOURCE_PAYLOAD_MAX_BYTES = 32 * 1024

/**
 * The canonical duplicate/replay identity: (source.system, source.itemId).
 * Stable across lanes, accounts and batch ids — the batch/import id is
 * provenance, never identity, so re-running an import replays instead of
 * duplicating. This is the identity downstream dedupe (interaction
 * (source_system, source_external_id); integration inbox
 * (source, source_account, external_event_id)) is derived from.
 */
export function intakeDedupeKey(message: CanonicalIntakeMessage): string {
  return `${message.source.system}|${message.source.itemId}`
}

/**
 * The durable inbox dedupe key projected from the envelope. Realtime scopes
 * by account namespace; batch leaves the account empty so the durable key
 * equals the canonical (system, itemId).
 */
export function intakeSourceIdentity(message: CanonicalIntakeMessage): {
  source: string
  sourceAccount: string
  externalEventId: string
} {
  return {
    source: message.source.system,
    sourceAccount: message.source.account ?? '',
    externalEventId: message.source.itemId,
  }
}

function jsonByteLength(value: unknown): number {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength
}

/**
 * Validate the envelope against the source-neutral contract. Returns a list
 * of problems (empty = valid). No channel/source-specific field is ever
 * required — validation covers only the contract invariants: schema version,
 * stable source identity, event type, occurred_at, provenance ownership and
 * the bounded-payload ceiling.
 */
export function validateIntakeMessage(
  message: CanonicalIntakeMessage,
): string[] {
  const problems: string[] = []
  if (message.schemaVersion !== INTAKE_MESSAGE_SCHEMA_VERSION) {
    problems.push(
      `schemaVersion must be ${INTAKE_MESSAGE_SCHEMA_VERSION}`,
    )
  }
  if (message.acquisitionLane !== 'realtime' && message.acquisitionLane !== 'batch') {
    problems.push(`acquisitionLane must be 'realtime' or 'batch'`)
  }
  if (!message.source.system || message.source.system.trim().length === 0) {
    problems.push('source.system is required (stable source system)')
  }
  if (!message.source.itemId || message.source.itemId.trim().length === 0) {
    problems.push('source.itemId is required (stable source item/event id)')
  }
  if (!message.eventType || message.eventType.trim().length === 0) {
    problems.push('eventType is required')
  }
  if (!message.occurredAt || Number.isNaN(new Date(message.occurredAt).getTime())) {
    problems.push('occurredAt must be a valid ISO timestamp')
  }
  if (!message.provenance?.adapter || message.provenance.adapter.trim().length === 0) {
    problems.push('provenance.adapter is required (raw-source ownership)')
  }
  if (
    !message.provenance?.adapterVersion ||
    message.provenance.adapterVersion.trim().length === 0
  ) {
    problems.push('provenance.adapterVersion is required (raw-source ownership)')
  }
  if (
    message.sourcePayload !== undefined &&
    jsonByteLength(message.sourcePayload) > INTAKE_SOURCE_PAYLOAD_MAX_BYTES
  ) {
    problems.push(
      `sourcePayload exceeds the ${INTAKE_SOURCE_PAYLOAD_MAX_BYTES}-byte bound`,
    )
  }
  return problems
}

/** Validate and throw on the first contract violation (adapter-side guard). */
export function assertValidIntakeMessage(message: CanonicalIntakeMessage): void {
  const problems = validateIntakeMessage(message)
  if (problems.length > 0) {
    throw new Error(`Invalid intake message: ${problems.join('; ')}`)
  }
}
