import { normalizePhone, sanitizeRawMetadata } from './crm-intake-normalization'
import type { JsonObject } from './crm-types'
import type { InboundEvent } from './crm-intake-types'
import type {
  AcceptedWhatsAppEvent,
  WhatsAppAdapterConfiguration,
  WhatsAppAdapterResult,
  WhatsAppAssurance,
  WhatsAppDirection,
  WhatsAppEndpoint,
  WhatsAppMessageClass,
  WhatsAppProviderEvent,
} from './crm-whatsapp-types'

const SOURCE_TOKEN = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/
const URI_PREFIX = /^[a-z][a-z0-9+.-]*:\/\//i
const DIRECTIONS = new Set<WhatsAppDirection>(['inbound', 'outbound'])
const ASSURANCES = new Set<WhatsAppAssurance>(['transport_observed'])
const MESSAGE_CLASSES = new Set<WhatsAppMessageClass>([
  'free_form',
  'template',
  'service',
  'system',
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
  if (length < 1 || length > 4000) {
    throw new Error('Message text must contain 1-4,000 Unicode code points.')
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

function normalizedConfiguration(configuration: WhatsAppAdapterConfiguration) {
  const owned = new Map<string, WhatsAppAdapterConfiguration['ownedLines'][number]>()
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
      throw new Error('WhatsApp endpoint appears in conflicting categories.')
    }
  }
  for (const phone of shared) {
    if (systems.has(phone)) {
      throw new Error('WhatsApp endpoint appears in conflicting categories.')
    }
  }
  return { owned, shared, systems }
}

type Classification =
  | { kind: 'owned'; value: string }
  | { kind: 'external'; value: string }
  | { kind: 'shared'; value: string }
  | { kind: 'system'; value: string }
  | { kind: 'withheld' }

function classifyEndpoint(
  endpoint: WhatsAppEndpoint,
  configuration: ReturnType<typeof normalizedConfiguration>,
): Classification {
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

function roleForOwnedLines(
  ownedPhones: string[],
  configuration: ReturnType<typeof normalizedConfiguration>,
) {
  const roles = unique(
    ownedPhones.map((phone) => configuration.owned.get(phone)?.creationRole),
  )
  return roles.length === 1 ? roles[0] : undefined
}

/**
 * Pure provider-neutral WhatsApp adapter. Translates a neutral WhatsApp
 * message into a canonical `InboundEvent` (channel `whatsapp`) or an explicit
 * excluded / resolution-required / rejected outcome.
 *
 * Assurance is `transport_observed` only: a signed webhook proves delivery
 * integrity, never ownership of the external sender/recipient, so identity
 * evidence is always `user_supplied` and person auto-creation is never
 * enabled here. Source idempotency uses the existing convention
 * `communications:<provider>:<accountNamespace>` + `whatsapp:<messageId>`.
 * No requested action is inferred, so no task noise is derived.
 */
export function adaptWhatsAppEvent(
  event: WhatsAppProviderEvent,
  rawConfiguration: WhatsAppAdapterConfiguration,
): WhatsAppAdapterResult {
  try {
    const provider = sourceToken(event.provider, 'provider')
    const accountNamespace = sourceToken(
      event.accountNamespace,
      'accountNamespace',
    )
    if (!ASSURANCES.has(event.actorAssurance)) {
      throw new Error('Actor assurance is invalid.')
    }
    if (!MESSAGE_CLASSES.has(event.contentClass)) {
      throw new Error('Message class is invalid.')
    }
    if (event.trustedDirection && !DIRECTIONS.has(event.trustedDirection)) {
      throw new Error('Trusted direction is invalid.')
    }
    const providerMessageId = opaqueId(
      event.providerMessageId,
      'providerMessageId',
    )
    const correlationId =
      event.correlationId === undefined
        ? undefined
        : opaqueId(event.correlationId, 'correlationId')
    if (
      !Array.isArray(event.from) ||
      !Array.isArray(event.to) ||
      !event.from.length ||
      !event.to.length
    ) {
      throw new Error('from and to endpoints are required.')
    }
    const configuration = normalizedConfiguration(rawConfiguration)
    const from = event.from.map((endpoint) =>
      classifyEndpoint(endpoint, configuration),
    )
    const to = event.to.map((endpoint) =>
      classifyEndpoint(endpoint, configuration),
    )
    const fromOwned = unique(
      from.flatMap((endpoint) =>
        endpoint.kind === 'owned' ? [endpoint.value] : [],
      ),
    )
    const toOwned = unique(
      to.flatMap((endpoint) =>
        endpoint.kind === 'owned' ? [endpoint.value] : [],
      ),
    )

    if (fromOwned.length && toOwned.length) {
      return from.every((endpoint) => endpoint.kind === 'owned') &&
        to.every((endpoint) => endpoint.kind === 'owned')
        ? { status: 'excluded', reason: 'internal_only' }
        : { status: 'resolution_required', reason: 'ambiguous_group' }
    }
    if (!fromOwned.length && !toOwned.length) {
      return [...from, ...to].some((endpoint) => endpoint.kind === 'system')
        ? { status: 'excluded', reason: 'configured_system_endpoint' }
        : { status: 'rejected', reason: 'missing_owned_line' }
    }

    const direction: WhatsAppDirection = fromOwned.length
      ? 'outbound'
      : 'inbound'
    if (event.trustedDirection && event.trustedDirection !== direction) {
      return { status: 'rejected', reason: 'conflicting_trusted_direction' }
    }
    // Template messages are outbound only; an inbound template is contradictory.
    if (event.contentClass === 'template' && direction === 'inbound') {
      return { status: 'rejected', reason: 'inbound_template' }
    }
    if (event.contentClass === 'system') {
      return { status: 'excluded', reason: 'system_message' }
    }

    const actors = direction === 'inbound' ? from : to
    const internalSide = direction === 'inbound' ? to : from
    const ownedPhones = direction === 'inbound' ? toOwned : fromOwned
    if (internalSide.some((endpoint) => endpoint.kind !== 'owned')) {
      return { status: 'resolution_required', reason: 'ambiguous_group' }
    }
    if (actors.some((endpoint) => endpoint.kind === 'system')) {
      return { status: 'excluded', reason: 'configured_system_endpoint' }
    }
    if (actors.some((endpoint) => endpoint.kind === 'withheld')) {
      return { status: 'resolution_required', reason: 'withheld_actor' }
    }
    if (actors.some((endpoint) => endpoint.kind === 'shared')) {
      return { status: 'resolution_required', reason: 'shared_external_actor' }
    }
    const externalPhones = unique(
      actors.flatMap((endpoint) =>
        endpoint.kind === 'external' ? [endpoint.value] : [],
      ),
    )
    if (externalPhones.length !== 1 || actors.some((endpoint) => endpoint.kind === 'owned')) {
      return {
        status: externalPhones.length > 1 ? 'resolution_required' : 'rejected',
        reason: externalPhones.length > 1 ? 'multiple_external_actors' : 'ambiguous_actor',
      }
    }
    const actorPhone = externalPhones[0]

    // Free-form messages require text; template/service messages may carry
    // optional text (templates additionally require a template reference).
    let summary: string | undefined
    if (event.contentClass === 'free_form') {
      summary = normalizeMessage(event.plainText)
    } else {
      if (event.contentClass === 'template' && event.templateId === undefined) {
        return { status: 'rejected', reason: 'missing_template_id' }
      }
      if (event.plainText !== undefined) summary = normalizeMessage(event.plainText)
    }

    const displayName = displayNameHint(event.displayNameHint)
    const occurredAt = new Date(event.occurredAt)
    if (Number.isNaN(occurredAt.getTime())) throw new Error('occurredAt is invalid.')

    const metadata: JsonObject = {
      transport: 'whatsapp',
      contentClass: event.contentClass,
    }
    if (event.templateId) metadata.templateId = opaqueId(event.templateId, 'templateId')
    if (correlationId) metadata.correlationId = correlationId

    const eventType =
      direction === 'inbound' ? 'whatsapp_received' : 'whatsapp_sent'

    const inboundEvent: InboundEvent = {
      source: {
        system: `communications:${provider}:${accountNamespace}`,
        externalId: `whatsapp:${providerMessageId}`,
      },
      occurredAt: occurredAt.toISOString(),
      channel: 'whatsapp',
      eventType,
      direction,
      actor: {
        identityHints: [
          { kind: 'phone', value: actorPhone, evidence: 'user_supplied' },
        ],
        ...(displayName ? { displayNameHint: displayName } : {}),
      },
      content: {
        ...(summary ? { summary } : {}),
      },
      ...(event.trustedContext ? { context: event.trustedContext } : {}),
      rawMetadata: sanitizeRawMetadata(metadata),
    }

    return {
      status: 'accepted',
      direction,
      actorPhone,
      applicableCreationRole: roleForOwnedLines(ownedPhones, configuration),
      inboundEvent,
    }
  } catch (error) {
    return {
      status: 'rejected',
      reason: error instanceof Error ? error.message : 'unknown',
    }
  }
}

export type AcceptedWhatsAppEventResult = AcceptedWhatsAppEvent
