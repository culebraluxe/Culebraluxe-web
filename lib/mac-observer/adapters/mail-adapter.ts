// ---------------------------------------------------------------------------
// CRM-23 — MailObserver adapter (macOS Mail).
//
// Access mechanism: macOS Mail message content is NOT readable without either
// the Mail app running with Full Disk Access, AppleScript (requires Mail
// automation consent), or an IMAP connection with app-specific credentials —
// none of which is proven in this environment. The capability is declared
// HONESTLY as 'unproven' (criterion 8): the adapter contract is complete and
// compiles, but the observer emits no facts until access is configured and
// reviewed. When access is proven, a bounded macOS observer process lowers
// Mail change facts into RawObservation JSON and this adapter lowers that
// JSON into the neutral ExternalActivityEvent — same seam as Contacts/Calendar.
// ---------------------------------------------------------------------------

import type { JsonObject } from '../../crm-types'
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

export const MAIL_OBSERVER_SOURCE = 'mail'
export const MAIL_ADAPTER_VERSION = 'mail.v1'

export const mailCapability: SourceCapability = {
  status: 'unproven',
  reason:
    'Mail message content requires Mail + Full Disk Access, AppleScript automation consent, or IMAP app-specific credentials. None is configured/proven; the adapter emits no facts until access is proven and reviewed. No fake semantics are hidden above the adapter.',
  requiredAccess: [
    'mail-full-disk-access',
    'apple-script-mail-automation',
    'imap-app-specific-credentials',
  ],
  supportedAppleFrameworks: ['Message.framework', 'Mail AppleScript'],
}

export type MailAttachmentRawPayload = {
  referenceId?: string
  filename?: string
  mimeType?: string
  sizeBytes?: number
}

export type MailRawPayload = {
  eventType: 'mail.message_received' | 'mail.message_sent'
  /** Stable source message id (RFC 822 Message-ID or provider id). */
  messageId: string
  /** When the message was sent/received at the source. */
  occurredAt: string
  threadId?: string
  inReplyTo?: string
  from?: { kind?: string; value?: string; displayName?: string }
  to?: Array<{ kind?: string; value?: string; displayName?: string }>
  cc?: Array<{ kind?: string; value?: string; displayName?: string }>
  bcc?: Array<{ kind?: string; value?: string; displayName?: string }>
  subject?: string
  /** Bounded summary only — never the raw body. */
  summary?: string
  /** Reference to the raw body artifact (IMAP uid, file reference). */
  contentReference?: string
  attachments?: MailAttachmentRawPayload[]
  senderAuthentication?: 'unverified' | 'authenticated_pass'
  category?:
    | 'human_correspondence'
    | 'system_notification'
    | 'delivery_status'
    | 'auto_reply'
    | 'bulk_list'
  correlationId?: string
  /** Bounded reference to the raw observation artifact (never the payload). */
  rawReference?: string
}

function attachmentsFromPayload(payload: JsonObject) {
  const entries = payload.attachments
  if (!Array.isArray(entries)) return undefined
  const out: ExternalActivityEvent['attachments'] = []
  for (const entry of entries) {
    if (!entry || typeof entry !== 'object') continue
    const referenceId = optionalText((entry as { referenceId?: unknown }).referenceId)
    if (!referenceId) continue
    out.push({
      referenceId,
      filename: optionalText((entry as { filename?: unknown }).filename),
      mimeType: optionalText((entry as { mimeType?: unknown }).mimeType),
      sizeBytes:
        typeof (entry as { sizeBytes?: unknown }).sizeBytes === 'number'
          ? (entry as { sizeBytes?: number }).sizeBytes
          : undefined,
    })
  }
  return out.length > 0 ? out : undefined
}

/** Lower one raw Mail observation into the neutral ExternalActivityEvent. */
export function lowerMailObservation(
  raw: RawObservation,
): ExternalActivityEvent {
  assertRawObservation(raw, MAIL_OBSERVER_SOURCE)
  const payload = raw.payload as MailRawPayload
  const eventType = requireText(payload.eventType, 'eventType')
  if (eventType !== 'mail.message_received' && eventType !== 'mail.message_sent') {
    throw new Error(`Unsupported mail event type: ${eventType}`)
  }
  const messageId = requireText(payload.messageId, 'messageId')
  const occurredAt = requireIsoTimestamp(payload.occurredAt, 'occurredAt')
  const direction = eventType === 'mail.message_received' ? 'inbound' : 'outbound'

  const sender = identityFromPayload(payload, 'from')
  const recipients = [
    ...identitiesFromPayload(payload, 'to'),
    ...identitiesFromPayload(payload, 'cc'),
    ...identitiesFromPayload(payload, 'bcc'),
  ]

  return buildExternalActivityEvent({
    raw,
    adapter: MAIL_ADAPTER_VERSION,
    adapterVersion: MAIL_ADAPTER_VERSION,
    eventType,
    occurredAt,
    direction,
    participants: [
      ...(sender ? [participant(sender, 'sender')] : []),
      ...recipients.map((identity) => participant(identity, 'recipient')),
    ],
    contactCandidates: (() => {
      const candidates = [...(sender ? [sender] : []), ...recipients]
      return candidates.length > 0 ? candidates : undefined
    })(),
    thread: {
      ...(optionalText(payload.threadId) ? { id: payload.threadId } : {}),
      ...(optionalText(payload.inReplyTo)
        ? { inReplyTo: payload.inReplyTo }
        : {}),
    },
    content: {
      ...(optionalText(payload.subject)
        ? { subject: payload.subject }
        : {}),
      ...(optionalText(payload.summary) ? { summary: payload.summary } : {}),
      ...(optionalText(payload.contentReference)
        ? { contentReference: payload.contentReference }
        : {}),
    },
    attachments: attachmentsFromPayload(payload as unknown as JsonObject),
    correlationId: optionalText(payload.correlationId),
    rawReference: optionalText(payload.rawReference),
  })
}

/**
 * A MailObserver bound to one macOS account namespace. Capability is
 * 'unproven': observe() returns [] — no fabricated mail facts.
 */
export function createMailObserver(accountNamespace: string): MacSourceObserver {
  return {
    source: MAIL_OBSERVER_SOURCE,
    accountNamespace,
    capability: mailCapability,
    observe: async () => [],
  }
}
