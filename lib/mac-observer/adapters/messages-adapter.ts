// ---------------------------------------------------------------------------
// CRM-23 — MessagesObserver adapter (macOS Messages / iMessage).
//
// HONEST CAPABILITY: iMessage message content is NOT accessible through any
// public macOS API. Reading the Messages database requires Full Disk Access
// AND disabling System Integrity Protection, or private/undocumented
// frameworks — neither is acceptable to enable, and no access is proven in
// this environment. This adapter therefore declares capability 'unsupported':
// the lower function keeps the neutral contract complete (so a future proven
// mechanism can slot in below the observer boundary), but the observer emits
// NO facts and the integration-inbox processor refuses 'unsupported' sources.
// No fake semantics or brittle coupling is hidden above the adapter.
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

export const MESSAGES_OBSERVER_SOURCE = 'messages'
export const MESSAGES_ADAPTER_VERSION = 'messages.v1'

export const messagesCapability: SourceCapability = {
  status: 'unsupported',
  reason:
    'iMessage content has no public macOS API. Reading the Messages database would require Full Disk Access plus SIP bypass or private frameworks — unacceptable and unproven. The observer emits no facts; the contract stays complete so a future, reviewed mechanism can slot in below the observer boundary.',
  requiredAccess: [
    'messages-full-disk-access (insufficient alone)',
    'sip-disable (unacceptable)',
  ],
  supportedAppleFrameworks: [],
}

export type MessagesRawPayload = {
  eventType: 'messages.message_received' | 'messages.message_sent'
  /** Stable source message id (Messages db row / GUID). */
  messageId: string
  occurredAt: string
  conversationId?: string
  from?: { kind?: string; value?: string; displayName?: string }
  to?: Array<{ kind?: string; value?: string; displayName?: string }>
  summary?: string
  correlationId?: string
  rawReference?: string
}

/**
 * Contract-complete lowerer for a future, proven iMessage access mechanism.
 * Today it is reachable only if a caller fabricates access — the processor
 * guards against that by refusing 'unsupported' capabilities (defense in
 * depth; see lib/integration-inbox/processor.ts).
 */
export function lowerMessagesObservation(
  raw: RawObservation,
): ExternalActivityEvent {
  assertRawObservation(raw, MESSAGES_OBSERVER_SOURCE)
  const payload = raw.payload as MessagesRawPayload
  const eventType = requireText(payload.eventType, 'eventType')
  if (
    eventType !== 'messages.message_received' &&
    eventType !== 'messages.message_sent'
  ) {
    throw new Error(`Unsupported messages event type: ${eventType}`)
  }
  const messageId = requireText(payload.messageId, 'messageId')
  const occurredAt = requireIsoTimestamp(payload.occurredAt, 'occurredAt')

  const sender = identityFromPayload(payload, 'from')
  const recipients = identitiesFromPayload(payload, 'to')

  return buildExternalActivityEvent({
    raw,
    adapter: MESSAGES_ADAPTER_VERSION,
    adapterVersion: MESSAGES_ADAPTER_VERSION,
    eventType,
    occurredAt,
    direction: eventType === 'messages.message_received' ? 'inbound' : 'outbound',
    participants: [
      ...(sender ? [participant(sender, 'sender')] : []),
      ...recipients.map((identity) => participant(identity, 'recipient')),
    ],
    contactCandidates: sender ? [sender] : undefined,
    thread: optionalText(payload.conversationId)
      ? { conversationId: payload.conversationId }
      : undefined,
    content: optionalText(payload.summary)
      ? { summary: payload.summary }
      : undefined,
    correlationId: optionalText(payload.correlationId),
    rawReference: optionalText(payload.rawReference),
  })
}

/** A MessagesObserver — capability 'unsupported': emits no facts, honestly. */
export function createMessagesObserver(accountNamespace: string): MacSourceObserver {
  return {
    source: MESSAGES_OBSERVER_SOURCE,
    accountNamespace,
    capability: messagesCapability,
    observe: async () => [],
  }
}
