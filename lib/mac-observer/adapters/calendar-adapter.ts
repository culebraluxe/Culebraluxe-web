// ---------------------------------------------------------------------------
// CRM-23 — CalendarObserver adapter (macOS Calendar via EventKit).
//
// Supported Apple framework: EventKit with TCC consent. The macOS observer
// process lowers Calendar change facts (created/updated/deleted) into
// RawObservation JSON; this adapter lowers that JSON into the neutral
// ExternalActivityEvent. Attendee identity is organizer-supplied and never
// ownership-verified — the existing calendar intake rule (allowCreation stays
// false) is preserved by the mapper, never overridden here.
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
  optionalText,
  participant,
  requireIsoTimestamp,
  requireText,
} from './shared'

export const CALENDAR_OBSERVER_SOURCE = 'calendar'
export const CALENDAR_ADAPTER_VERSION = 'calendar.v1'

export const calendarCapability: SourceCapability = {
  status: 'available',
  reason:
    'macOS EventKit is observable with user TCC consent. The observer persists only calendar change facts, never the whole calendar database.',
  requiredAccess: ['tcc:calendar', 'eventkit-change-observation'],
  supportedAppleFrameworks: ['EventKit.framework'],
}

export type CalendarRawPayload = {
  eventType:
    | 'calendar.event_created'
    | 'calendar.event_updated'
    | 'calendar.event_deleted'
  /** Stable source calendar event id (the inbox dedupe key component). */
  calendarEventId: string
  /** When the change happened at the source. */
  changedAt: string
  /** The appointment time (the CRM timeline fact time). */
  eventStartAt: string
  organizer: 'owned' | 'external'
  attendees?: Array<{ kind?: string; value?: string; displayName?: string }>
  title?: string
  description?: string
  displayNameHint?: string
  correlationId?: string
  /** Recurring event series / thread reference where available. */
  threadId?: string
  /** Bounded reference to the raw observation artifact (never the payload). */
  rawReference?: string
}

/** Lower one raw Calendar observation into the neutral ExternalActivityEvent. */
export function lowerCalendarObservation(
  raw: RawObservation,
): ExternalActivityEvent {
  assertRawObservation(raw, CALENDAR_OBSERVER_SOURCE)
  const payload = raw.payload as CalendarRawPayload
  const eventType = requireText(payload.eventType, 'eventType')
  if (
    eventType !== 'calendar.event_created' &&
    eventType !== 'calendar.event_updated' &&
    eventType !== 'calendar.event_deleted'
  ) {
    throw new Error(`Unsupported calendar event type: ${eventType}`)
  }
  const calendarEventId = requireText(
    payload.calendarEventId,
    'calendarEventId',
  )
  requireIsoTimestamp(payload.changedAt, 'changedAt')
  const eventStartAt = requireIsoTimestamp(payload.eventStartAt, 'eventStartAt')
  const organizer = requireText(payload.organizer, 'organizer')
  if (organizer !== 'owned' && organizer !== 'external') {
    throw new Error('organizer must be owned or external.')
  }

  const attendees = identitiesFromPayload(payload, 'attendees')

  return buildExternalActivityEvent({
    raw,
    adapter: CALENDAR_ADAPTER_VERSION,
    adapterVersion: CALENDAR_ADAPTER_VERSION,
    eventType,
    occurredAt: eventStartAt,
    // The organizer flag maps onto the neutral transport direction (owned
    // organizes -> outbound; external organizes -> inbound); attendees keep
    // the neutral 'attendee' role. The mapper derives the channel-specific
    // organizer from the direction — no calendar concept leaks outward.
    direction: organizer === 'owned' ? 'outbound' : 'inbound',
    participants: attendees.map((identity) => participant(identity, 'attendee')),
    contactCandidates: attendees.length > 0 ? attendees : undefined,
    thread: optionalText(payload.threadId)
      ? { id: payload.threadId }
      : undefined,
    content: {
      ...(optionalText(payload.title)
        ? { subject: payload.title }
        : {}),
      ...(optionalText(payload.description)
        ? { summary: payload.description }
        : {}),
    },
    correlationId: optionalText(payload.correlationId),
    rawReference: optionalText(payload.rawReference),
  })
}

/** A CalendarObserver bound to one macOS account namespace. */
export function createCalendarObserver(accountNamespace: string): MacSourceObserver {
  return {
    source: CALENDAR_OBSERVER_SOURCE,
    accountNamespace,
    capability: calendarCapability,
    observe: async () => [],
  }
}
