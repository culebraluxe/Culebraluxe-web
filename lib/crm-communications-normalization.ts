import {
  normalizePhone,
  sanitizeRawMetadata,
} from './crm-intake-normalization'
import type { JsonObject } from './crm-types'
import type {
  AcceptedCommunicationsEvent,
  CallDisposition,
  CommunicationsAdapterConfiguration,
  CommunicationsAdapterResult,
  CommunicationsAssurance,
  CommunicationsDirection,
  CommunicationsEndpoint,
  CommunicationsProviderEvent,
  CommunicationsTransport,
} from './crm-communications-types'

const SOURCE_TOKEN = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/
const URI_PREFIX = /^[a-z][a-z0-9+.-]*:\/\//i
const TRANSPORTS = new Set<CommunicationsTransport>(['call', 'sms', 'imessage'])
const DIRECTIONS = new Set<CommunicationsDirection>(['inbound', 'outbound'])
const ASSURANCES = new Set<CommunicationsAssurance>([
  'transport_observed',
  'ownership_verified',
  'authenticated_actor',
])
const DISPOSITIONS = new Set<CallDisposition>([
  'connected',
  'no_answer',
  'busy',
  'failed',
  'canceled',
  'voicemail',
])
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

function systemEndpoint(value: string) {
  if (typeof value !== 'string') throw new Error('System endpoint must be a string.')
  const normalized = value.normalize('NFKC').trim()
  if (!normalized || CONTROL_CHARACTERS.test(normalized)) {
    throw new Error('System endpoint must be a nonempty transport address.')
  }
  return normalized
}

function normalizedConfiguration(configuration: CommunicationsAdapterConfiguration) {
  const owned = new Map<string, CommunicationsAdapterConfiguration['ownedLines'][number]>()
  for (const line of configuration.ownedLines) {
    const phone = normalizePhone(line.phone)
    if (line.creationRole && !ROLES.has(line.creationRole)) {
      throw new Error('Owned-line creation role is invalid.')
    }
    const prior = owned.get(phone)
    if (prior) {
      if (prior.creationRole !== line.creationRole) {
        throw new Error('Owned-line configuration has conflicting roles.')
      }
      throw new Error('Owned-line configuration contains a duplicate phone.')
    }
    owned.set(phone, { ...line, phone })
  }

  const shared = new Set((configuration.sharedExternalPhones ?? []).map(normalizePhone))
  const systems = new Set((configuration.systemEndpoints ?? []).map(systemEndpoint))
  for (const phone of owned.keys()) {
    if (shared.has(phone) || systems.has(phone)) {
      throw new Error('Communications endpoint appears in conflicting categories.')
    }
  }
  for (const phone of shared) {
    if (systems.has(phone)) {
      throw new Error('Communications endpoint appears in conflicting categories.')
    }
  }
  return { owned, shared, systems }
}

type EndpointClassification =
  | { kind: 'owned'; value: string }
  | { kind: 'external'; value: string }
  | { kind: 'shared'; value: string }
  | { kind: 'system'; value: string }
  | { kind: 'withheld' }

function classifyEndpoint(
  endpoint: CommunicationsEndpoint,
  configuration: ReturnType<typeof normalizedConfiguration>,
): EndpointClassification {
  if (!endpoint || typeof endpoint !== 'object') {
    throw new Error('Endpoint must be an address or withheld endpoint.')
  }
  if (endpoint.kind === 'withheld') return { kind: 'withheld' }
  if (endpoint.kind !== 'address' || typeof endpoint.value !== 'string') {
    throw new Error('Endpoint must be an address or withheld endpoint.')
  }
  const exact = systemEndpoint(endpoint.value)
  if (configuration.systems.has(exact)) return { kind: 'system', value: exact }
  let phone: string
  try {
    phone = normalizePhone(exact)
  } catch {
    throw new Error('Unconfigured person endpoint must be strict E.164.')
  }
  if (configuration.owned.has(phone)) return { kind: 'owned', value: phone }
  if (configuration.shared.has(phone)) return { kind: 'shared', value: phone }
  return { kind: 'external', value: phone }
}

function unique<T>(values: T[]) {
  return [...new Set(values)]
}

function roleForOwnedLines(
  ownedPhones: string[],
  configuration: ReturnType<typeof normalizedConfiguration>,
) {
  const roles = unique(ownedPhones.map((phone) => configuration.owned.get(phone)?.creationRole))
  return roles.length === 1 ? roles[0] : undefined
}

function normalizeMessage(value: string | undefined) {
  if (typeof value !== 'string') throw new Error('Message text is required.')
  const normalized = value
    .normalize('NFKC')
    .replace(/\r\n?/g, '\n')
    .trim()
  if (!normalized || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(normalized)) {
    throw new Error('Message text contains disallowed content.')
  }
  const length = [...normalized].length
  if (length < 1 || length > 4000) throw new Error('Message text must contain 1-4,000 Unicode code points.')
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

function callFacts(
  direction: CommunicationsDirection,
  disposition: CallDisposition | undefined,
  durationSeconds: number | undefined,
) {
  if (!disposition || !DISPOSITIONS.has(disposition)) {
    throw new Error('Call disposition is invalid.')
  }
  const requiresDuration =
    disposition === 'connected' ||
    (direction === 'outbound' && disposition === 'voicemail')
  if (requiresDuration) {
    if (!Number.isSafeInteger(durationSeconds) || durationSeconds! < 0) {
      throw new Error('This call outcome requires a non-negative safe-integer duration.')
    }
  } else if (durationSeconds !== undefined) {
    throw new Error('This call outcome forbids duration.')
  }
  if (direction === 'inbound' && (disposition === 'failed' || disposition === 'canceled')) {
    throw new Error('Inbound transport failure is not a person interaction.')
  }
  return {
    eventType:
      direction === 'outbound'
        ? 'call_placed'
        : disposition === 'connected'
          ? 'call_received'
          : 'call_missed',
    durationSeconds,
    voicemail: disposition === 'voicemail',
  }
}

export function adaptCommunicationsEvent(
  event: CommunicationsProviderEvent,
  rawConfiguration: CommunicationsAdapterConfiguration,
): CommunicationsAdapterResult {
  try {
    const provider = sourceToken(event.provider, 'provider')
    const accountNamespace = sourceToken(event.accountNamespace, 'accountNamespace')
    if (!TRANSPORTS.has(event.transport)) throw new Error('Transport is invalid.')
    if (!ASSURANCES.has(event.actorAssurance)) throw new Error('Actor assurance is invalid.')
    if (event.trustedDirection && !DIRECTIONS.has(event.trustedDirection)) {
      throw new Error('Trusted direction is invalid.')
    }
    const providerEventId = opaqueId(event.providerEventId, 'providerEventId')
    const correlationId = event.correlationId === undefined
      ? undefined
      : opaqueId(event.correlationId, 'correlationId')
    const configuration = normalizedConfiguration(rawConfiguration)
    if (!Array.isArray(event.from) || !Array.isArray(event.to) || !event.from.length || !event.to.length) {
      throw new Error('from and to endpoints are required.')
    }
    const from = event.from.map((endpoint) => classifyEndpoint(endpoint, configuration))
    const to = event.to.map((endpoint) => classifyEndpoint(endpoint, configuration))
    const fromOwned = unique(from.flatMap((endpoint) => endpoint.kind === 'owned' ? [endpoint.value] : []))
    const toOwned = unique(to.flatMap((endpoint) => endpoint.kind === 'owned' ? [endpoint.value] : []))

    if (fromOwned.length && toOwned.length) {
      return from.every((endpoint) => endpoint.kind === 'owned') &&
        to.every((endpoint) => endpoint.kind === 'owned')
        ? { status: 'excluded', reason: 'internal_only' }
        : { status: 'resolution_required', reason: 'ambiguous_group_or_conference' }
    }
    if (!fromOwned.length && !toOwned.length) {
      return [...from, ...to].some((endpoint) => endpoint.kind === 'system')
        ? { status: 'excluded', reason: 'configured_system_endpoint' }
        : { status: 'rejected', reason: 'missing_owned_endpoint' }
    }
    const direction: CommunicationsDirection = fromOwned.length ? 'outbound' : 'inbound'
    if (event.trustedDirection && event.trustedDirection !== direction) {
      return { status: 'rejected', reason: 'conflicting_trusted_direction' }
    }
    const actors = direction === 'inbound' ? from : to
    const internalSide = direction === 'inbound' ? to : from
    const ownedPhones = direction === 'inbound' ? toOwned : fromOwned
    if (internalSide.some((endpoint) => endpoint.kind !== 'owned')) {
      return { status: 'resolution_required', reason: 'ambiguous_group_or_conference' }
    }
    if (actors.some((actor) => actor.kind === 'system')) {
      return { status: 'excluded', reason: 'configured_system_endpoint' }
    }
    if (actors.some((actor) => actor.kind === 'withheld')) {
      return { status: 'resolution_required', reason: 'withheld_actor' }
    }
    if (actors.some((actor) => actor.kind === 'shared')) {
      return { status: 'resolution_required', reason: 'shared_external_actor' }
    }
    const externalPhones = unique(actors.flatMap((actor) => actor.kind === 'external' ? [actor.value] : []))
    if (externalPhones.length !== 1 || actors.some((actor) => actor.kind === 'owned')) {
      return {
        status: externalPhones.length > 1
          ? 'resolution_required'
          : 'rejected',
        reason: externalPhones.length > 1 ? 'multiple_external_actors' : 'ambiguous_actor',
      }
    }

    if (event.transport !== 'call' && (event.callDisposition !== undefined || event.durationSeconds !== undefined)) {
      throw new Error('Message events cannot contain call outcome fields.')
    }
    if (event.transport === 'call' && event.plainText !== undefined) {
      throw new Error('Call events cannot contain message text.')
    }

    const metadata: JsonObject = { transport: event.transport }
    if (correlationId) metadata.correlationId = correlationId
    let eventType: string
    let summary: string | undefined
    let durationSeconds: number | undefined
    if (event.transport === 'call') {
      const facts = callFacts(direction, event.callDisposition, event.durationSeconds)
      eventType = facts.eventType
      durationSeconds = facts.durationSeconds
      metadata.callDisposition = event.callDisposition!
      if (durationSeconds !== undefined) metadata.durationProvenance = 'provider_reported'
      if (facts.voicemail) metadata.voicemail = true
      summary = eventType === 'call_missed' ? 'Missed call' : eventType === 'call_received' ? 'Call received' : 'Call placed'
    } else {
      summary = normalizeMessage(event.plainText)
      eventType = `${event.transport}_${direction === 'inbound' ? 'received' : 'sent'}`
    }

    const evidence = event.actorAssurance === 'authenticated_actor'
      ? 'authenticated'
      : event.actorAssurance === 'ownership_verified'
        ? 'provider_asserted'
        : 'user_supplied'
    const applicableCreationRole = roleForOwnedLines(ownedPhones, configuration)
    const occurredAt = new Date(event.occurredAt)
    if (Number.isNaN(occurredAt.getTime())) throw new Error('occurredAt is invalid.')

    const result: AcceptedCommunicationsEvent = {
      status: 'accepted',
      direction,
      actorPhone: externalPhones[0],
      applicableCreationRole,
      inboundEvent: {
        source: {
          system: `communications:${provider}:${accountNamespace}`,
          externalId: `${event.transport}:${providerEventId}`,
        },
        occurredAt: occurredAt.toISOString(),
        channel: event.transport,
        eventType,
        direction,
        actor: {
          identityHints: [{ kind: 'phone', value: externalPhones[0], evidence }],
          displayNameHint: displayNameHint(event.displayNameHint),
          roleHint: applicableCreationRole,
        },
        content: { summary, durationSeconds },
        context: event.trustedContext,
        rawMetadata: sanitizeRawMetadata(metadata),
      },
    }
    return result
  } catch (error) {
    return { status: 'rejected', reason: error instanceof Error ? error.message : 'Invalid communications event.' }
  }
}
