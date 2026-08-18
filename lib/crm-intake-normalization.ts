import type { JsonObject, JsonValue } from './crm-types'
import type {
  IdentityHint,
  InboundEvent,
  NormalizedIdentityHint,
  NormalizedInboundEvent,
} from './crm-intake-types'

const MAX_METADATA_BYTES = 32 * 1024
const SECRET_KEYS = new Set([
  'accesstoken',
  'authorization',
  'apikey',
  'clientsecret',
  'cookie',
  'cookies',
  'password',
  'passwd',
  'refreshtoken',
  'secret',
])

function requireText(value: string, field: string) {
  const normalized = value.normalize('NFKC').trim()
  if (!normalized) throw new Error(`${field} is required.`)
  return normalized
}

export function normalizeEmail(value: string) {
  const normalized = requireText(value, 'Email').toLowerCase()
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalized)) {
    throw new Error('Email is invalid.')
  }
  return normalized
}

export function normalizePhone(value: string) {
  const trimmed = requireText(value, 'Phone')
  if (!trimmed.startsWith('+')) {
    throw new Error('Phone must include an explicit country code.')
  }

  const normalized = `+${trimmed.slice(1).replace(/[\s().-]/g, '')}`
  if (!/^\+[1-9]\d{7,14}$/.test(normalized)) {
    throw new Error('Phone must be a valid E.164 number.')
  }
  return normalized
}

export function normalizeDisplayName(value: string) {
  return requireText(value, 'Display name').replace(/\s+/g, ' ')
}

export function normalizeOccurredAt(value: string | Date) {
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) {
    throw new Error('occurredAt must be a valid timestamp.')
  }
  return date.toISOString()
}

export function extractRecognizedPropertySlug(value: string) {
  let url: URL
  try {
    url = new URL(value)
  } catch {
    return null
  }

  const host = url.hostname.toLowerCase()
  if (host !== 'culebraluxe.com' && host !== 'www.culebraluxe.com') {
    return null
  }

  const match = url.pathname.match(/^\/properties\/([^/]+)\/?$/)
  if (!match) return null

  try {
    return decodeURIComponent(match[1]).trim().toLowerCase()
  } catch {
    return null
  }
}

export function normalizeIdentityHint(
  hint: IdentityHint,
): NormalizedIdentityHint {
  if (hint.kind === 'email') {
    return { ...hint, normalizedValue: normalizeEmail(hint.value) }
  }

  if (hint.kind === 'phone') {
    return { ...hint, normalizedValue: normalizePhone(hint.value) }
  }

  const sourceSystem = requireText(
    hint.sourceSystem,
    'External identity sourceSystem',
  ).toLowerCase()
  const externalId = requireText(hint.value, 'External identity value')

  return {
    ...hint,
    sourceSystem,
    normalizedValue: `${sourceSystem}:${externalId}`,
  }
}

function assertMetadataSafe(value: JsonValue, path: string) {
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      assertMetadataSafe(item, `${path}[${index}]`),
    )
    return
  }

  if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      const normalizedKey = key.toLowerCase().replace(/[_\-\s]/g, '')
      if (SECRET_KEYS.has(normalizedKey)) {
        throw new Error(`rawMetadata contains prohibited secret field: ${path}.${key}`)
      }
      assertMetadataSafe(child, `${path}.${key}`)
    }
  }
}

export function sanitizeRawMetadata(metadata: JsonObject) {
  if (!metadata || Array.isArray(metadata) || typeof metadata !== 'object') {
    throw new Error('rawMetadata must be a JSON object.')
  }

  assertMetadataSafe(metadata, 'rawMetadata')

  const serialized = JSON.stringify(metadata)
  if (new TextEncoder().encode(serialized).byteLength > MAX_METADATA_BYTES) {
    throw new Error('rawMetadata exceeds the 32 KB limit.')
  }

  return JSON.parse(serialized) as JsonObject
}

export function normalizeInboundEvent(
  event: InboundEvent,
): NormalizedInboundEvent {
  const sourceSystem = requireText(
    event.source.system,
    'source.system',
  ).toLowerCase()
  const sourceExternalId = requireText(
    event.source.externalId,
    'source.externalId',
  )
  const eventType = requireText(event.eventType, 'eventType')

  if (
    event.content?.durationSeconds !== undefined &&
    (!Number.isInteger(event.content.durationSeconds) ||
      event.content.durationSeconds < 0)
  ) {
    throw new Error('durationSeconds must be a non-negative integer.')
  }

  return {
    ...event,
    source: { system: sourceSystem, externalId: sourceExternalId },
    eventType,
    occurredAt: normalizeOccurredAt(event.occurredAt),
    actor: {
      ...event.actor,
      displayNameHint: event.actor.displayNameHint
        ? normalizeDisplayName(event.actor.displayNameHint)
        : undefined,
      identityHints: event.actor.identityHints.map(normalizeIdentityHint),
    },
    content: event.content
      ? {
          ...event.content,
          subject: event.content.subject?.normalize('NFKC').trim() || undefined,
          summary: event.content.summary?.normalize('NFKC').trim() || undefined,
        }
      : undefined,
    context: event.context
      ? {
          propertyId: event.context.propertyId?.trim() || undefined,
          propertySlug:
            event.context.propertySlug?.trim().toLowerCase() || undefined,
          propertyUrl: event.context.propertyUrl?.trim() || undefined,
          dealId: event.context.dealId?.trim() || undefined,
        }
      : undefined,
    rawMetadata: sanitizeRawMetadata(event.rawMetadata),
  }
}
