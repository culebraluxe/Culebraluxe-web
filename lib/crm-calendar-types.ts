import type { InboundEvent, NormalizedIntakeResult } from './crm-intake-types'
import type { PersonCreationResult, PersonRole } from './crm-person-types'

export type CalendarDirection = 'inbound' | 'outbound'
export type CalendarAssurance = 'transport_observed'
export type CalendarAttendeeKind = 'email' | 'phone'
export type CalendarOrganizer = 'owned' | 'external'

export interface CalendarAttendee {
  kind: CalendarAttendeeKind
  value: string
}

/**
 * Provider-neutral calendar appointment facts, ready for a future connector
 * to lower Google/Apple calendar payloads into. Only normalized transport
 * facts cross into CRM; provider SDK objects and credentials never do.
 *
 * This is CRM-08 readiness scaffolding: it produces canonical in-memory
 * inputs only. No interaction, task, interest, or person write is reachable,
 * and no calendar provider is contacted.
 *
 * Provider authentication authenticates the owned business calendar
 * account/transport — not ownership of an external attendee email or phone.
 * Attendee identity is always `user_supplied`; stronger attendee assurance
 * would require a separately reviewed future capability.
 */
export interface CalendarProviderEvent {
  provider: string
  accountNamespace: string
  providerEventId: string
  occurredAt: string | Date
  organizer: CalendarOrganizer
  attendees: CalendarAttendee[]
  actorAssurance: CalendarAssurance
  title?: string
  description?: string
  displayNameHint?: string
  correlationId?: string
  trustedDirection?: CalendarDirection
  trustedContext?: InboundEvent['context']
}

export interface OwnedCalendarAccount {
  email: string
  creationRole?: PersonRole
}

export interface CalendarAdapterConfiguration {
  ownedCalendarEmails: OwnedCalendarAccount[]
  sharedExternalEmails?: string[]
  systemEmails?: string[]
}

export type AcceptedCalendarEvent = {
  status: 'accepted'
  direction: CalendarDirection
  actorIdentityHint: InboundEvent['actor']['identityHints'][number]
  applicableCreationRole?: PersonRole
  inboundEvent: InboundEvent
}

export type CalendarAdapterResult =
  | AcceptedCalendarEvent
  | { status: 'excluded'; reason: string }
  | { status: 'resolution_required'; reason: string }
  | { status: 'rejected'; reason: string }

export type CalendarIntakeResult =
  | { status: 'excluded' | 'rejected'; reason: string }
  | {
      status: 'resolution_required'
      reason: string
      personResult?: PersonCreationResult
    }
  | { status: 'duplicate'; existingInteractionId: string }
  | {
      status: 'ready'
      personResult: PersonCreationResult
      intakeResult: NormalizedIntakeResult
    }
