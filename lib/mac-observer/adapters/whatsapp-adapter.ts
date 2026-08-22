// ---------------------------------------------------------------------------
// CRM-23 — WhatsAppObserver adapter (macOS WhatsApp).
//
// HONEST CAPABILITY: WhatsApp has NO public macOS API. The macOS desktop app
// stores conversations in a proprietary, undocumented local database — reading
// it is unsupported and unacceptable. This adapter declares capability
// 'unsupported': the lower function keeps the neutral contract complete for a
// future, reviewed mechanism (e.g. a provider webhook connector — see
// lib/crm-whatsapp-*, CRM-07 — which lives BELOW this observer boundary, not
// here), but the observer emits NO facts and the integration-inbox processor
// refuses 'unsupported' sources. No fake semantics above the adapter.
// ---------------------------------------------------------------------------

import type {
  ExternalActivityEvent,
  MacSourceObserver,
  RawObservation,
  SourceCapability,
} from '../contracts'
import {
  assertRawObservation,
  buildExternalActivityEvent,
  identitiesFromPayload,
  identityFromPayload,
  optionalText,
  participant,
  requireIsoTimestamp,
  requireText,
} from './shared'

export const WHATSAPP_OBSERVER_SOURCE = 'whatsapp'
export const WHATSAPP_ADAPTER_VERSION = 'whatsapp.v1'

export const whatsappCapability: SourceCapability = {
  status: 'unsupported',
  reason:
    'WhatsApp has no public macOS API; the desktop app database is proprietary and undocumented. Reading it is unsupported. A real connector belongs to the provider webhook seam (CRM-07 / lib/crm-whatsapp-*), below the observer boundary. The observer emits no facts.',
  requiredAccess: ['whatsapp-business-api (provider webhook — not a Mac observer)'],
  supportedAppleFrameworks: [],
}

export type WhatsAppRawPayload = {
  eventType: 'whatsapp.message_received' | 'whatsapp.message_sent'
  /** Stable source message id. */
  messageId: string
  occurredAt: string
  from?: { kind?: string; value?: string; displayName?: string }
  to?: Array<{ kind?: string; value?: string; displayName?: string }>
  summary?: string
  correlationId?: string
  rawReference?: string
}

/** Contract-complete lowerer for a future, proven WhatsApp mechanism. The
 *  processor refuses 'unsupported' capabilities, so this is defense in depth
 *  — never a fabricated path. */
export function lowerWhatsAppObservation(
  raw: RawObservation,
): ExternalActivityEvent {
  assertRawObservation(raw, WHATSAPP_OBSERVER_SOURCE)
  const payload = raw.payload as WhatsAppRawPayload
  const eventType = requireText(payload.eventType, 'eventType')
  if (
    eventType !== 'whatsapp.message_received' &&
    eventType !== 'whatsapp.message_sent'
  ) {
    throw new Error(`Unsupported whatsapp event type: ${eventType}`)
  }
  const messageId = requireText(payload.messageId, 'messageId')
  const occurredAt = requireIsoTimestamp(payload.occurredAt, 'occurredAt')

  const sender = identityFromPayload(payload, 'from')
  const recipients = identitiesFromPayload(payload, 'to')

  return buildExternalActivityEvent({
    raw,
    adapter: WHATSAPP_ADAPTER_VERSION,
    adapterVersion: WHATSAPP_ADAPTER_VERSION,
    eventType,
    occurredAt,
    direction: eventType === 'whatsapp.message_received' ? 'inbound' : 'outbound',
    participants: [
      ...(sender ? [participant(sender, 'sender')] : []),
      ...recipients.map((identity) => participant(identity, 'recipient')),
    ],
    contactCandidates: sender ? [sender] : undefined,
    content: optionalText(payload.summary)
      ? { summary: payload.summary }
      : undefined,
    correlationId: optionalText(payload.correlationId),
    rawReference: optionalText(payload.rawReference),
  })
}

/** A WhatsAppObserver — capability 'unsupported': emits no facts, honestly. */
export function createWhatsAppObserver(accountNamespace: string): MacSourceObserver {
  return {
    source: WHATSAPP_OBSERVER_SOURCE,
    accountNamespace,
    capability: whatsappCapability,
    observe: async () => [],
  }
}
