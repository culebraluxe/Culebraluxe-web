// ---------------------------------------------------------------------------
// CRM-23 — ContactsObserver adapter (macOS Contacts).
//
// Supported Apple framework: Contacts.framework / EventKit-adjacent TCC
// consent. Least privilege: observe ONLY contact change facts (add/update/
// delete + changed identities), never the whole address book. The payload
// carries identity candidates; canonical CRM resolves them onto the existing
// person spine and stores only normalized business data.
//
// Capability: 'available' when the macOS Contacts observer process is
// configured with TCC consent. The node-side adapter never reads the address
// book itself — a bounded macOS observer process lowers Contacts change facts
// into RawObservation JSON; this adapter lowers that JSON into the neutral
// ExternalActivityEvent.
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

export const CONTACTS_OBSERVER_SOURCE = 'contacts'
export const CONTACTS_ADAPTER_VERSION = 'contacts.v1'

export const contactsCapability: SourceCapability = {
  status: 'available',
  reason:
    'macOS Contacts.framework is observable with user TCC consent. The observer persists only change facts, not the address book.',
  requiredAccess: ['tcc:contacts', 'contacts-change-observation-process'],
  supportedAppleFrameworks: ['Contacts.framework'],
}

/** The raw Contacts change payload contract (produced by the macOS observer). */
export type ContactsRawPayload = {
  eventType: 'contact.added' | 'contact.updated' | 'contact.deleted'
  /** Stable source contact id (the inbox dedupe key component). */
  contactId: string
  /** When the change happened at the source. */
  changedAt: string
  /** Primary identity (email/phone) with display name. */
  identity?: { kind?: string; value?: string; displayName?: string }
  /** Additional identities on the contact. */
  additionalIdentities?: Array<{
    kind?: string
    value?: string
    displayName?: string
  }>
  displayName?: string
  /** Bounded reference to the raw observation artifact (never the payload). */
  rawReference?: string
}

/**
 * Lower one raw Contacts observation into the neutral ExternalActivityEvent.
 * The contact's identities become participants (role 'contact') AND contact
 * candidates — the canonical contact spine. No CRM intent is derived here.
 */
export function lowerContactsObservation(
  raw: RawObservation,
): ExternalActivityEvent {
  assertRawObservation(raw, CONTACTS_OBSERVER_SOURCE)
  const payload = raw.payload as ContactsRawPayload
  const eventType = requireText(payload.eventType, 'eventType')
  if (
    eventType !== 'contact.added' &&
    eventType !== 'contact.updated' &&
    eventType !== 'contact.deleted'
  ) {
    throw new Error(`Unsupported contacts event type: ${eventType}`)
  }
  const contactId = requireText(payload.contactId, 'contactId')
  const changedAt = requireIsoTimestamp(payload.changedAt, 'changedAt')

  const primary = identityFromPayload(payload, 'identity')
  const additional = identitiesFromPayload(payload, 'additionalIdentities')
  const identities = [
    ...(primary ? [primary] : []),
    ...additional,
  ]

  const candidates = identities.length > 0 ? identities : undefined

  return buildExternalActivityEvent({
    raw,
    adapter: CONTACTS_ADAPTER_VERSION,
    adapterVersion: CONTACTS_ADAPTER_VERSION,
    eventType,
    occurredAt: changedAt,
    participants: [
      participant(
        { kind: 'contact', value: contactId, displayName: optionalText(payload.displayName) },
        'contact',
      ),
      ...identities.map((identity) => participant(identity, 'contact')),
    ],
    contactCandidates: candidates,
    content: optionalText(payload.displayName)
      ? { subject: optionalText(payload.displayName) }
      : undefined,
    rawReference: optionalText(payload.rawReference),
  })
}

/** A ContactsObserver bound to one macOS account namespace. */
export function createContactsObserver(accountNamespace: string): MacSourceObserver {
  return {
    source: CONTACTS_OBSERVER_SOURCE,
    accountNamespace,
    capability: contactsCapability,
    // The node-side observer does not contact the framework: a bounded macOS
    // observer process pushes RawObservation JSON. Acquisition is injected by
    // the caller (poll endpoint / file drop) — this contract stays honest.
    observe: async () => [],
  }
}
