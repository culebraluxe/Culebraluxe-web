// ---------------------------------------------------------------------------
// CRM-23 — Fake Mac observer (contract formalization fixture).
//
// Implementation sequence step 1: formalize the event/inbox contracts using a
// FAKE observer before wiring any real source. This fake produces
// deterministic RawObservation fixtures for the defined adapters (contacts,
// calendar, mail) so the neutral contract and the durable inbox can be proven
// end-to-end without any macOS access.
//
// Honesty rules:
//   - By default the fake declares the SAME capability as the real adapter
//     (mail stays 'unproven', messages/whatsapp stay 'unsupported') so the
//     fixture can never accidentally pretend access exists.
//   - Tests that need to exercise a non-'available' adapter end-to-end pass an
//     EXPLICIT capability override labeled 'fake-for-test' — a visible test
//     double, never hidden semantics.
// ---------------------------------------------------------------------------

import type {
  ExternalActivitySource,
  MacSourceObserver,
  RawObservation,
  SourceCapability,
} from './contracts'

export type FakeObservationFixture = {
  source: ExternalActivitySource
  sourceAccount: string
  rawEventId: string
  observedAt: string
  payload: Record<string, unknown>
}

export const FAKE_OBSERVER_ADAPTER_LABEL = 'fake-for-test'

function rawFromFixture(fixture: FakeObservationFixture): RawObservation {
  return {
    source: fixture.source,
    sourceAccount: fixture.sourceAccount,
    rawEventId: fixture.rawEventId,
    observedAt: fixture.observedAt,
    payload: fixture.payload as RawObservation['payload'],
  }
}

/**
 * Deterministic contacts observation fixture (contact.updated).
 * `idSuffix` keeps fixtures unique across tests/instances.
 */
export function contactsFixture(idSuffix = '1'): FakeObservationFixture {
  return {
    source: 'contacts',
    sourceAccount: 'iCloud:acct',
    rawEventId: `contact-${idSuffix}`,
    observedAt: '2026-08-22T10:05:00.000Z',
    payload: {
      eventType: 'contact.updated',
      contactId: `ABPerson-${idSuffix}`,
      changedAt: '2026-08-22T10:04:00.000Z',
      identity: {
        kind: 'email',
        value: `buyer${idSuffix}@example.com`,
        displayName: `Buyer ${idSuffix}`,
      },
      additionalIdentities: [
        { kind: 'phone', value: '+1-555-010-000' + idSuffix },
      ],
      displayName: `Buyer ${idSuffix}`,
      rawReference: `contacts-raw-${idSuffix}.json`,
    },
  }
}

/** Deterministic calendar observation fixture (calendar.event_created). */
export function calendarFixture(idSuffix = '1'): FakeObservationFixture {
  return {
    source: 'calendar',
    sourceAccount: 'iCloud:acct',
    rawEventId: `ek-event-${idSuffix}`,
    observedAt: '2026-08-22T10:10:00.000Z',
    payload: {
      eventType: 'calendar.event_created',
      calendarEventId: `EKEvent-${idSuffix}`,
      changedAt: '2026-08-22T10:09:00.000Z',
      eventStartAt: '2026-08-22T14:00:00.000Z',
      organizer: 'external',
      attendees: [
        { kind: 'email', value: 'buyer1@example.com', displayName: 'Buyer 1' },
        { kind: 'email', value: 'agent@culebraluxe.example' },
      ],
      title: 'Property viewing',
      threadId: `series-${idSuffix}`,
      rawReference: `calendar-raw-${idSuffix}.json`,
    },
  }
}

/** Deterministic mail observation fixture (mail.message_received). */
export function mailFixture(idSuffix = '1'): FakeObservationFixture {
  return {
    source: 'mail',
    sourceAccount: 'IMAP:acct',
    rawEventId: `msg-${idSuffix}`,
    observedAt: '2026-08-22T10:15:00.000Z',
    payload: {
      eventType: 'mail.message_received',
      messageId: `Message-ID-${idSuffix}`,
      occurredAt: '2026-08-22T10:14:00.000Z',
      threadId: `thread-${idSuffix}`,
      inReplyTo: `prev-${idSuffix}`,
      from: {
        kind: 'email',
        value: 'buyer1@example.com',
        displayName: 'Buyer 1',
      },
      to: [{ kind: 'email', value: 'agent@culebraluxe.example' }],
      subject: 'Property inquiry',
      summary: 'Interested in Casa Luar.',
      contentReference: `imap-uid-${idSuffix}`,
      attachments: [
        { referenceId: `att-${idSuffix}`, filename: 'plans.pdf', mimeType: 'application/pdf', sizeBytes: 2048 },
      ],
      senderAuthentication: 'unverified',
      category: 'human_correspondence',
      correlationId: `corr-mail-${idSuffix}`,
      rawReference: `mail-raw-${idSuffix}.eml`,
    },
  }
}

/**
 * The fake observer: deterministic fixture observations, honest capabilities.
 * `capabilityOverride` (label 'fake-for-test') lets a test exercise an
 * otherwise non-'available' adapter through the full inbox pipeline — an
 * explicit test double, never hidden semantics.
 */
export function createFakeMacObserver(
  fixtures: FakeObservationFixture[],
  options: { capabilityOverride?: SourceCapability } = {},
): MacSourceObserver {
  const first = fixtures[0]
  if (!first) throw new Error('A fake observer needs at least one fixture.')
  const capabilityBySource: Record<string, SourceCapability> = {
    contacts: {
      status: 'available',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
    calendar: {
      status: 'available',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
    mail: {
      status: 'unproven',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
    messages: {
      status: 'unsupported',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
    whatsapp: {
      status: 'unsupported',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
  }
  return {
    source: first.source,
    accountNamespace: first.sourceAccount,
    capability: options.capabilityOverride ?? capabilityBySource[first.source] ?? {
      status: 'unsupported',
      reason: 'fake-for-test',
      requiredAccess: [],
      supportedAppleFrameworks: [],
    },
    observe: async () => fixtures.map(rawFromFixture),
  }
}
