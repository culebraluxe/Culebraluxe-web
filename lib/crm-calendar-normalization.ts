import {
  normalizeEmail,
  normalizePhone,
  sanitizeRawMetadata,
} from './crm-intake-normalization'
import type { JsonObject } from './crm-types'
import type { InboundEvent } from './crm-intake-types'
import type {
  AcceptedCalendarEvent,
  CalendarAdapterConfiguration,
  CalendarAdapterResult,
  CalendarAssurance,
  CalendarAttendee,
  CalendarDirection,
  CalendarOrganizer,
  CalendarProviderEvent,
} from './crm-calendar-types'

const SOURCE_TOKEN = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/
const URI_PREFIX = /^[a-z][a-z0-9+.-]*:\/\//i
const DIRECTIONS = new Set<CalendarDirection>(['inbound', 'outbound'])
const ASSURANCES = new Set<CalendarAssurance>(['transport_observed'])
const ORGANIZERS = new Set<CalendarOrganizer>(['owned', 'external'])
const ROLES = new Set(['buyer', 'seller', 'both'])

function sourceToken(value: string, field: string) {
  if (typeof value !== 'string') throw new Error(`${field} must be a string.`)
  const normalized = value.normalize('NFKC').trim().toLowerCase()
  if (!SOURCE_TOKEN.test(normalized)) {
    throw new Error(`${field} must be a valid 1-64 character source token.`)
  }
  return normalized
}

function opaqueId(value: string, field: string) {
  if (
    typeof value !== 'string' ||
    value.length < 1 ||
    value.length > 512 ||
    value.trim() !== value ||
    CONTROL_CHARACTERS.test(value) ||
    value.includes(':') ||
    URI_PREFIX.test(value)
  ) {
    throw new Error(`${field} must be a bounded opaque identifier.`)
  }
  return value
}

function boundedText(value: string | undefined, maximum: number, field: string) {
  if (value === undefined) return undefined
  if (typeof value !== 'string') throw new Error(`${field} must be a string.`)
  const normalized = value.normalize('NFKC').trim()
  if (!normalized) return undefined
  if (normalized.length > maximum) {
    throw new Error(`${field} exceeds ${maximum} characters.`)
  }
  return normalized
}

function displayNameHint(value: string | undefined) {
  if (value === undefined) return undefined
  if (typeof value !== 'string') throw new Error('displayNameHint must be a string.')
  const normalized = value.normalize('NFKC').trim().replace(/\s+/g, ' ')
  if (!normalized) return undefined
  if (normalized.length > 200) throw new Error('displayNameHint exceeds 200 characters.')
  return normalized
}

function unique<T>(values: T[]) {
  return [...new Set(values)]
}

function normalizedConfiguration(configuration: CalendarAdapterConfiguration) {
  const owned = new Map<string, CalendarAdapterConfiguration['ownedCalendarEmails'][number]>()
  for (const account of configuration.ownedCalendarEmails) {
    const email = normalizeEmail(account.email)
    if (account.creationRole && !ROLES.has(account.creationRole)) {
      throw new Error('Owned calendar creation role is invalid.')
    }
    const prior = owned.get(email)
    if (prior) {
      if (prior.creationRole !== account.creationRole) {
        throw new Error('Owned calendar configuration has conflicting roles.')
      }
      throw new Error('Owned calendar configuration contains a duplicate email.')
    }
    owned.set(email, { ...account, email })
  }
  const shared = new Set((configuration.sharedExternalEmails ?? []).map(normalizeEmail))
  const systems = new Set((configuration.systemEmails ?? []).map(normalizeEmail))
  for (const email of owned.keys()) {
    if (shared.has(email) || systems.has(email)) {
      throw new Error('Calendar endpoint appears in conflicting categories.')
    }
  }
  for (const email of shared) {
    if (systems.has(email)) {
      throw new Error('Calendar endpoint appears in conflicting categories.')
    }
  }
  return { owned, shared, systems }
}

type Classification =
  | { kind: 'owned'; value: string }
  | { kind: 'shared'; value: string }
  | { kind: 'system'; value: string }
  | { kind: 'external'; value: string }
  | { kind: 'external_phone'; value: string }

function classifyAttendee(
  attendee: CalendarAttendee,
  configuration: ReturnType<typeof normalizedConfiguration>,
): Classification {
  if (
    !attendee ||
    typeof attendee !== 'object' ||
    !attendee.kind ||
    typeof attendee.value !== 'string'
  ) {
    throw new Error('Attendee must be an address object.')
  }
  if (attendee.kind === 'email') {
    const email = normalizeEmail(attendee.value)
    if (configuration.owned.has(email)) return { kind: 'owned', value: email }
    if (configuration.shared.has(email)) return { kind: 'shared', value: email }
    if (configuration.systems.has(email)) return { kind: 'system', value: email }
    return { kind: 'external', value: email }
  }
  if (attendee.kind === 'phone') {
    return { kind: 'external_phone', value: normalizePhone(attendee.value) }
  }
  throw new Error('Attendee kind must be email or phone.')
}

function roleForOwnedAccounts(
  ownedEmails: string[],
  configuration: ReturnType<typeof normalizedConfiguration>,
) {
  const roles = unique(
    ownedEmails.map((email) => configuration.owned.get(email)?.creationRole),
  )
  return roles.length === 1 ? roles[0] : undefined
}

/**
 * Pure provider-neutral calendar adapter. Translates a neutral calendar
 * appointment into a canonical `InboundEvent` (channel `calendar`) or an
 * explicit excluded / resolution-required / rejected outcome. No task is
 * derived and no persistence is reachable, so calendar intake cannot create
 * task noise. Source idempotency uses the existing
 * (source_system, source_external_id) convention via
 * `calendar:<provider>:<accountNamespace>` + the provider event id.
 *
 * Calendar readiness scaffolding does not create persons. Provider
 * authentication verifies the owned calendar transport/account, not ownership
 * of an external attendee email/phone; attendee identity evidence is therefore
 * always `user_supplied`, and stronger attendee assurance would require a
 * separately reviewed future capability.
 */
export function adaptCalendarEvent(
  event: CalendarProviderEvent,
  rawConfiguration: CalendarAdapterConfiguration,
): CalendarAdapterResult {
  try {
    const provider = sourceToken(event.provider, 'provider')
    const accountNamespace = sourceToken(
      event.accountNamespace,
      'accountNamespace',
    )
    if (!ASSURANCES.has(event.actorAssurance)) {
      throw new Error('Actor assurance is invalid.')
    }
    if (!ORGANIZERS.has(event.organizer)) {
      throw new Error('Organizer is invalid.')
    }
    if (event.trustedDirection && !DIRECTIONS.has(event.trustedDirection)) {
      throw new Error('Trusted direction is invalid.')
    }
    const providerEventId = opaqueId(event.providerEventId, 'providerEventId')
    const correlationId =
      event.correlationId === undefined
        ? undefined
        : opaqueId(event.correlationId, 'correlationId')
    if (!Array.isArray(event.attendees) || event.attendees.length === 0) {
      throw new Error('At least one attendee is required.')
    }
    const configuration = normalizedConfiguration(rawConfiguration)
    const classifications = event.attendees.map((attendee) =>
      classifyAttendee(attendee, configuration),
    )

    if (classifications.some((classification) => classification.kind === 'system')) {
      return { status: 'excluded', reason: 'configured_system_endpoint' }
    }
    if (!classifications.some((classification) => classification.kind === 'owned')) {
      return { status: 'rejected', reason: 'missing_owned_calendar' }
    }
    if (classifications.some((classification) => classification.kind === 'shared')) {
      return { status: 'resolution_required', reason: 'shared_external_actor' }
    }

    const externalEmails = unique(
      classifications.flatMap((classification) =>
        classification.kind === 'external' ? [classification.value] : [],
      ),
    )
    const externalPhones = unique(
      classifications.flatMap((classification) =>
        classification.kind === 'external_phone' ? [classification.value] : [],
      ),
    )
    const externalTotal = externalEmails.length + externalPhones.length
    if (externalTotal === 0) return { status: 'excluded', reason: 'internal_only' }
    if (externalTotal > 1) {
      return { status: 'resolution_required', reason: 'multiple_external_actors' }
    }

    const direction: CalendarDirection =
      event.organizer === 'owned' ? 'outbound' : 'inbound'
    if (event.trustedDirection && event.trustedDirection !== direction) {
      return { status: 'rejected', reason: 'conflicting_trusted_direction' }
    }

    const ownedEmails = unique(
      classifications.flatMap((classification) =>
        classification.kind === 'owned' ? [classification.value] : [],
      ),
    )
    const actorValue = externalEmails[0] ?? externalPhones[0]
    const actorKind = externalEmails.length === 1 ? 'email' : 'phone'
    // The calendar provider authenticates the owned business account/transport,
    // not the external attendee. Attendee emails/phones are organizer-supplied
    // and never ownership-verified, so calendar identity evidence is always
    // 'user_supplied' — never 'authenticated'.
    const evidence = 'user_supplied' as const
    const actorIdentityHint: InboundEvent['actor']['identityHints'][number] =
      actorKind === 'email'
        ? { kind: 'email', value: actorValue, evidence }
        : { kind: 'phone', value: actorValue, evidence }

    const title = boundedText(event.title, 300, 'Title')
    const summary = boundedText(event.description, 4000, 'Description')
    const displayName = displayNameHint(event.displayNameHint)
    const occurredAt = new Date(event.occurredAt)
    if (Number.isNaN(occurredAt.getTime())) throw new Error('occurredAt is invalid.')

    const metadata: JsonObject = { eventType: 'appointment' }
    if (correlationId) metadata.correlationId = correlationId

    const inboundEvent: InboundEvent = {
      source: {
        system: `calendar:${provider}:${accountNamespace}`,
        externalId: providerEventId,
      },
      occurredAt: occurredAt.toISOString(),
      channel: 'calendar',
      eventType: 'appointment',
      direction,
      actor: {
        identityHints: [actorIdentityHint],
        ...(displayName ? { displayNameHint: displayName } : {}),
      },
      content: {
        ...(title ? { subject: title } : {}),
        ...(summary ? { summary } : {}),
      },
      ...(event.trustedContext ? { context: event.trustedContext } : {}),
      rawMetadata: sanitizeRawMetadata(metadata),
    }

    return {
      status: 'accepted',
      direction,
      actorIdentityHint,
      applicableCreationRole: roleForOwnedAccounts(ownedEmails, configuration),
      inboundEvent,
    }
  } catch (error) {
    return {
      status: 'rejected',
      reason: error instanceof Error ? error.message : 'unknown',
    }
  }
}

export type AcceptedCalendarEventResult = AcceptedCalendarEvent
