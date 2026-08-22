// ---------------------------------------------------------------------------
// CRM-23 — Integration Inbox mapper.
//
// MAPPER RESPONSIBILITY (architect brief): translate source-neutral external
// activity into the existing canonical interaction/contact intake contracts;
// perform identity/contact candidate resolution; choose the appropriate
// application intake path.
//
// This module maps the neutral ExternalActivityEvent into the EXISTING
// channel intake contracts (CalendarProviderEvent, EmailProviderMessage,
// CommunicationsProviderEvent, WhatsAppProviderEvent) and the generic
// InboundEvent (contacts). It REUSES the existing CRM intake stubs
// (lib/crm-calendar-intake.ts, lib/crm-email-intake.ts,
// lib/crm-communications-intake.ts, lib/crm-whatsapp-intake.ts,
// lib/crm-intake.ts) — no parallel canonical intake tables or parallel
// normalization. Source-specific business rules (assurance, creation policy,
// exclusion) stay inside those existing stubs; the mapper only translates.
//
// No business decisions are made here: the mapper never decides that an email
// creates a task or a calendar change triggers a workflow. It lowers facts to
// the neutral contracts and lets the intake stubs + domain layer decide.
// ---------------------------------------------------------------------------

import type { InboundEvent } from '../crm-intake-types'
import type { CalendarProviderEvent } from '../crm-calendar-types'
import type { EmailProviderMessage } from '../crm-email-types'
import type { CommunicationsProviderEvent } from '../crm-communications-types'
import type { WhatsAppProviderEvent } from '../crm-whatsapp-types'
import type { ExternalActivityEvent, ExternalIdentity } from '../mac-observer/contracts'

/** The canonical source token the existing intake contracts require
 *  (lowercase, 1-64 chars, [a-z0-9_-]). Mac-observed events belong to the
 *  `macos` provider namespace within each channel. */
export const MAC_OBSERVER_PROVIDER = 'macos'

/**
 * Deterministic, collision-safe token for a macOS account namespace
 * ('iCloud:acct' -> 'icloud-acct'), because the channel intake contracts
 * require a bounded source token.
 */
export function tokenizeAccountNamespace(value: string): string {
  const token = value
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return token.slice(0, 63) || 'mac'
}

/** Source identity convention shared by every Mac-observed channel:
 *  `<channel>:macos:<accountNamespace>`. */
export function macSourceSystem(
  source: ExternalActivityEvent['source'],
  sourceAccount: string,
): string {
  return `${source}:${MAC_OBSERVER_PROVIDER}:${tokenizeAccountNamespace(sourceAccount)}`
}

function externalIdentitiesOf(
  event: ExternalActivityEvent,
  role: ExternalActivityEvent['participants'][number]['role'],
): ExternalIdentity[] {
  return event.participants
    .filter((p) => p.role === role)
    .map(({ kind, value, displayName }) => ({ kind, value, displayName }))
}

function identityHintsFromCandidates(
  candidates: ExternalIdentity[] | undefined,
): InboundEvent['actor']['identityHints'] {
  const hints: InboundEvent['actor']['identityHints'] = []
  for (const candidate of candidates ?? []) {
    if (candidate.kind === 'email' || candidate.kind === 'phone') {
      hints.push({
        kind: candidate.kind,
        value: candidate.value,
        // The Mac observer observes the record/transport, never ownership of
        // the external identity — evidence is always 'user_supplied' here,
        // exactly like the calendar attendee rule. Person auto-creation stays
        // off (allowCreation = false in the processor).
        evidence: 'user_supplied',
      })
    } else {
      hints.push({
        kind: 'external',
        value: candidate.value,
        sourceSystem: candidate.kind === 'whatsapp' ? 'whatsapp' : candidate.kind,
        evidence: 'user_supplied',
      })
    }
  }
  return hints
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

/** Lower a neutral calendar fact into the existing CalendarProviderEvent. */
export function mapCalendarEvent(
  event: ExternalActivityEvent,
): CalendarProviderEvent {
  const attendees = externalIdentitiesOf(event, 'attendee')
    .filter((i) => i.kind === 'email' || i.kind === 'phone')
    .map((i) => ({ kind: i.kind as 'email' | 'phone', value: i.value }))
  if (attendees.length === 0) {
    throw new Error('Calendar observation carries no attendee identities.')
  }
  return {
    provider: MAC_OBSERVER_PROVIDER,
    accountNamespace: tokenizeAccountNamespace(event.sourceAccount),
    providerEventId: event.externalEventId,
    occurredAt: event.occurredAt,
    organizer: event.direction === 'outbound' ? 'owned' : 'external',
    attendees,
    actorAssurance: 'transport_observed',
    title: event.content?.subject,
    description: event.content?.summary,
    displayNameHint: event.participants.find((p) => p.displayName)?.displayName,
    correlationId: event.correlationId,
    trustedDirection: event.direction,
  }
}

// ---------------------------------------------------------------------------
// Mail
// ---------------------------------------------------------------------------

/** Lower a neutral mail fact into the existing EmailProviderMessage. */
export function mapMailEvent(event: ExternalActivityEvent): EmailProviderMessage {
  const sender = externalIdentitiesOf(event, 'sender')[0]
  const recipients = externalIdentitiesOf(event, 'recipient').filter(
    (i) => i.kind === 'email',
  )
  if (!sender || sender.kind !== 'email') {
    throw new Error('Mail observation carries no email sender.')
  }
  return {
    provider: MAC_OBSERVER_PROVIDER,
    accountNamespace: tokenizeAccountNamespace(event.sourceAccount),
    messageId: event.externalEventId,
    threadId: event.thread?.id,
    inReplyToMessageId: event.thread?.inReplyTo,
    occurredAt: event.occurredAt,
    senders: [{ email: sender.value, displayName: sender.displayName }],
    // The neutral event preserves recipient identities (to/cc/bcc flatten to
    // 'recipient'); the existing email intake classifies on the combined
    // recipient set, which is preserved exactly.
    to: recipients.map((i) => ({ email: i.value })),
    trustedDirection: event.direction,
    // The Mac observer does not (yet) authenticate senders or classify
    // content — honest defaults, never fabricated verdicts.
    senderAuthentication: 'unverified',
    category: 'human_correspondence',
    subject: event.content?.subject,
    plainText: event.content?.summary,
    // The existing email intake requires complete attachment descriptors; only
    // attachments with a filename, MIME type and size cross the mapper.
    attachments: (event.attachments ?? []).flatMap((a) =>
      a.filename && a.mimeType && Number.isSafeInteger(a.sizeBytes)
        ? [{
            providerAttachmentId: a.referenceId,
            filename: a.filename,
            mimeType: a.mimeType,
            sizeBytes: a.sizeBytes as number,
          }]
        : [],
    ),
  }
}

// ---------------------------------------------------------------------------
// Messages / iMessage (communications intake)
// ---------------------------------------------------------------------------

/** Lower a neutral messages fact into the existing CommunicationsProviderEvent. */
export function mapMessagesEvent(
  event: ExternalActivityEvent,
): CommunicationsProviderEvent {
  const sender = externalIdentitiesOf(event, 'sender')[0]
  const recipients = externalIdentitiesOf(event, 'recipient')
  const endpoints = (identities: ExternalIdentity[]) =>
    identities
      .filter((i) => i.kind === 'phone' || i.kind === 'email' || i.kind === 'whatsapp')
      .map((i) => ({ kind: 'address' as const, value: i.value }))
  return {
    provider: MAC_OBSERVER_PROVIDER,
    accountNamespace: tokenizeAccountNamespace(event.sourceAccount),
    transport: 'imessage',
    providerEventId: event.externalEventId,
    occurredAt: event.occurredAt,
    from: sender ? endpoints([sender]) : [],
    to: endpoints(recipients),
    trustedDirection: event.direction,
    actorAssurance: 'transport_observed',
    plainText: event.content?.summary,
    correlationId: event.correlationId,
    displayNameHint:
      sender?.displayName ?? event.participants.find((p) => p.displayName)?.displayName,
  }
}

// ---------------------------------------------------------------------------
// WhatsApp
// ---------------------------------------------------------------------------

/** Lower a neutral whatsapp fact into the existing WhatsAppProviderEvent. */
export function mapWhatsAppEvent(
  event: ExternalActivityEvent,
): WhatsAppProviderEvent {
  const sender = externalIdentitiesOf(event, 'sender')[0]
  const recipients = externalIdentitiesOf(event, 'recipient')
  const endpoints = (identities: ExternalIdentity[]) =>
    identities
      .filter((i) => i.kind === 'phone' || i.kind === 'whatsapp')
      .map((i) => ({ kind: 'address' as const, value: i.value }))
  return {
    provider: MAC_OBSERVER_PROVIDER,
    accountNamespace: tokenizeAccountNamespace(event.sourceAccount),
    providerMessageId: event.externalEventId,
    occurredAt: event.occurredAt,
    from: sender ? endpoints([sender]) : [],
    to: endpoints(recipients),
    trustedDirection: event.direction,
    actorAssurance: 'transport_observed',
    contentClass: 'free_form',
    plainText: event.content?.summary,
    correlationId: event.correlationId,
    displayNameHint:
      sender?.displayName ?? event.participants.find((p) => p.displayName)?.displayName,
  }
}

// ---------------------------------------------------------------------------
// Contacts (the spine: identity/contact resolution before any mutation)
// ---------------------------------------------------------------------------

/**
 * Lower a neutral contacts fact into the generic InboundEvent. The contact
 * candidates become identity hints (evidence 'user_supplied'); the processor
 * resolves them onto the canonical person BEFORE any mutation (criterion 5)
 * and never auto-creates a person from an observed address-book record.
 */
export function mapContactsEvent(event: ExternalActivityEvent): InboundEvent {
  const candidates =
    event.contactCandidates && event.contactCandidates.length > 0
      ? event.contactCandidates
      : event.participants
          .filter((p) => p.role === 'contact')
          .map(({ kind, value, displayName }) => ({ kind, value, displayName }))

  const identityHints = identityHintsFromCandidates(candidates)
  if (identityHints.length === 0) {
    throw new Error('Contacts observation carries no resolvable identity candidates.')
  }

  const primary = candidates.find((c) => c.kind === 'email' || c.kind === 'phone')
  const displayName = primary?.displayName

  return {
    source: {
      system: macSourceSystem(event.source, event.sourceAccount),
      externalId: event.externalEventId,
    },
    occurredAt: event.occurredAt,
    channel: 'note',
    eventType: event.eventType,
    actor: {
      identityHints,
      ...(displayName ? { displayNameHint: displayName } : {}),
    },
    content: event.content
      ? {
          subject: event.content.subject,
          summary: event.content.summary,
        }
      : undefined,
    context: event.context,
    rawMetadata: {
      ...(event.correlationId ? { correlationId: event.correlationId } : {}),
      ...(event.provenance.rawReference
        ? { provenanceReference: event.provenance.rawReference }
        : {}),
    },
  }
}
