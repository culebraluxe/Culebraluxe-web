// ---------------------------------------------------------------------------
// CRM-23 — Shared adapter lowering helpers.
//
// Every Mac source adapter lowers RawObservation into the neutral
// ExternalActivityEvent. This module owns the common validation + assembly so
// adapters stay thin and the neutral contract stays identical across sources.
// Nothing here knows CRM intents, tasks, deals or workflows — acquisition
// only.
// ---------------------------------------------------------------------------

import type { JsonObject } from '../../crm-types'
import type {
  ExternalActivityEvent,
  ExternalIdentity,
  ExternalParticipant,
  ExternalProvenance,
  RawObservation,
} from '../contracts'

export function requireText(value: unknown, field: string): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(`Raw observation field '${field}' is required.`)
  }
  return value.trim()
}

export function requireIsoTimestamp(value: unknown, field: string): string {
  const text = requireText(value, field)
  const date = new Date(text)
  if (Number.isNaN(date.getTime())) {
    throw new Error(`Raw observation field '${field}' must be a valid timestamp.`)
  }
  return date.toISOString()
}

export function optionalText(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed || undefined
}

/** Validate the transport envelope shared by every raw observation. */
export function assertRawObservation(
  raw: RawObservation,
  expectedSource: string,
): void {
  if (raw.source !== expectedSource) {
    throw new Error(
      `Adapter '${expectedSource}' cannot lower a raw observation from source '${raw.source}'.`,
    )
  }
  requireText(raw.sourceAccount, 'sourceAccount')
  requireText(raw.rawEventId, 'rawEventId')
  requireIsoTimestamp(raw.observedAt, 'observedAt')
  if (
    !raw.payload ||
    Array.isArray(raw.payload) ||
    typeof raw.payload !== 'object'
  ) {
    throw new Error("Raw observation field 'payload' must be a JSON object.")
  }
}

function participant(
  identity: ExternalIdentity,
  role: ExternalParticipant['role'],
): ExternalParticipant {
  return { ...identity, role }
}

/** Lower a raw payload identity entry into an ExternalIdentity. */
export function identityFromPayload(
  payload: JsonObject,
  field: string,
): ExternalIdentity | undefined {
  const entry = payload[field] as
    | { kind?: unknown; value?: unknown; displayName?: unknown }
    | undefined
  if (!entry || typeof entry !== 'object') return undefined
  const value = optionalText(entry.value)
  if (!value) return undefined
  return {
    kind: (optionalText(entry.kind) ?? 'external') as ExternalIdentity['kind'],
    value,
    displayName: optionalText(entry.displayName),
  }
}

/** Lower a payload identities array into ExternalIdentity[]. */
export function identitiesFromPayload(
  payload: JsonObject,
  field: string,
): ExternalIdentity[] {
  const entries = payload[field]
  if (!Array.isArray(entries)) return []
  const out: ExternalIdentity[] = []
  for (const entry of entries) {
    if (!entry || typeof entry !== 'object') continue
    const value = optionalText((entry as { value?: unknown }).value)
    if (!value) continue
    out.push({
      kind: (optionalText((entry as { kind?: unknown }).kind) ??
        'external') as ExternalIdentity['kind'],
      value,
      displayName: optionalText((entry as { displayName?: unknown }).displayName),
    })
  }
  return out
}

export type BuildExternalActivityEventInput = {
  raw: RawObservation
  adapter: string
  adapterVersion: string
  eventType: string
  occurredAt: string
  direction?: ExternalActivityEvent['direction']
  participants: ExternalParticipant[]
  contactCandidates?: ExternalIdentity[]
  thread?: ExternalActivityEvent['thread']
  content?: ExternalActivityEvent['content']
  attachments?: ExternalActivityEvent['attachments']
  correlationId?: string
  context?: ExternalActivityEvent['context']
  rawReference?: string
}

/**
 * Assemble the neutral ExternalActivityEvent from a lowered observation.
 * `occurredAt` may differ from `observedAt` (a fact can pre-date acquisition);
 * `observedAt` always records acquisition time. Provenance references the raw
 * artifact instead of duplicating it (privacy/retention criterion 10).
 */
export function buildExternalActivityEvent(
  input: BuildExternalActivityEventInput,
): ExternalActivityEvent {
  const provenance: ExternalProvenance = {
    rawReference: input.rawReference,
    adapter: input.adapter,
    adapterVersion: input.adapterVersion,
  }
  return {
    schemaVersion: 1,
    source: input.raw.source,
    sourceAccount: input.raw.sourceAccount,
    externalEventId: input.raw.rawEventId,
    eventType: input.eventType,
    occurredAt: input.occurredAt,
    observedAt: new Date(input.raw.observedAt).toISOString(),
    direction: input.direction,
    participants: input.participants,
    contactCandidates: input.contactCandidates,
    thread: input.thread,
    content: input.content,
    attachments: input.attachments,
    correlationId: input.correlationId,
    context: input.context,
    provenance,
  }
}

export { participant }
