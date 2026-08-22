// ---------------------------------------------------------------------------
// INTAKE-01 — Realtime lane adapter output.
//
// The real-time acquisition lane (mac-observer + provider webhooks) lowers
// its neutral observed fact (ExternalActivityEvent) into the canonical intake
// message. This is the ONLY realtime edge transformation: everything after
// this function (durable inbox, identity resolution, Business Command layer)
// consumes the canonical envelope. The raw observation payload stays behind
// the adapter boundary — provenance references it, it is never forwarded.
// ---------------------------------------------------------------------------

import type { ExternalActivityEvent } from '../mac-observer/contracts'
import type { CanonicalIntakeMessage } from './contracts'

/**
 * Lower a neutral real-time observed fact into the canonical intake message.
 * Lossless for every neutral field the durable inbox needs; the raw payload
 * is never forwarded (referenced via provenance.rawReference only).
 */
export function lowerExternalActivityEventToIntakeMessage(
  event: ExternalActivityEvent,
): CanonicalIntakeMessage {
  return {
    schemaVersion: 1,
    acquisitionLane: 'realtime',
    source: {
      system: event.source,
      itemId: event.externalEventId,
      account: event.sourceAccount,
    },
    eventType: event.eventType,
    occurredAt: event.occurredAt,
    observedAt: event.observedAt,
    direction: event.direction,
    participants: event.participants,
    contactCandidates: event.contactCandidates,
    thread: event.thread,
    content: event.content,
    attachments: event.attachments,
    context: event.context,
    correlationId: event.correlationId,
    provenance: event.provenance,
  }
}
