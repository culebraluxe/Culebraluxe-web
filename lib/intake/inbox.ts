// ---------------------------------------------------------------------------
// INTAKE-01 — The single projection into the durable intake transport.
//
// `toInboxInsert` is the ONE transformation from the canonical intake message
// into the durable integration inbox (migration 044). Both lanes route
// through it, so the durable transport — and therefore identity resolution
// and the Business Command layer downstream — sees one neutral shape and
// never needs source-specific parsing rules. No new canonical CRM state model
// is created: the inbox table is the existing one and the projection is pure.
//
// The projection deliberately drops the envelope's sourcePayload: the inbox
// stores only neutral business facts + provenance references (CRM-23 privacy
// / retention policy — raw payloads never persist).
// ---------------------------------------------------------------------------

import type { InsertIntegrationInboxInput } from '../integration-inbox/contracts'
import type { CanonicalIntakeMessage } from './contracts'
import { intakeSourceIdentity } from './contracts'

/**
 * Project the canonical intake message into the durable inbox insert. The
 * dedupe key is derived from the envelope via intakeSourceIdentity: realtime
 * scopes by account namespace, batch leaves it empty (the durable key equals
 * the canonical (system, itemId) — re-imports replay, never duplicate).
 * `maxAttempts` is inbox configuration (bounded retry), not source data.
 */
export function toInboxInsert(
  message: CanonicalIntakeMessage,
  maxAttempts: number,
): InsertIntegrationInboxInput {
  const identity = intakeSourceIdentity(message)
  return {
    source: identity.source,
    sourceAccount: identity.sourceAccount,
    externalEventId: identity.externalEventId,
    eventType: message.eventType,
    occurredAt: message.occurredAt,
    observedAt: message.observedAt ?? message.occurredAt,
    direction: message.direction ?? null,
    correlationId: message.correlationId ?? null,
    threadId: message.thread?.id ?? message.thread?.conversationId ?? null,
    subject: message.content?.subject ?? null,
    summary: message.content?.summary ?? null,
    contentReference: message.content?.contentReference ?? null,
    provenanceReference: message.provenance.rawReference ?? null,
    participantIdentities: message.participants.map(
      ({ kind, value, displayName }) => ({
        kind,
        value,
        ...(displayName ? { displayName } : {}),
      }),
    ),
    contactCandidates: message.contactCandidates ?? null,
    attachmentMetadata: (message.attachments ?? []).map((a) => ({
      referenceId: a.referenceId,
      filename: a.filename,
      mimeType: a.mimeType,
      sizeBytes: a.sizeBytes,
    })),
    maxAttempts,
  }
}
