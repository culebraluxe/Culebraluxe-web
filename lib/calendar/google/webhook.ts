// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: webhook (push notification) verification.
//
// Google Calendar push notifications (developers.google.com/calendar/push)
// carry NO signed payload: the notification body is empty and the trust
// anchors are the X-Goog-* headers, most importantly X-Goog-Channel-Token —
// an arbitrary string we set when the channel is created, acting as a shared
// secret. Verification here:
//   - requires the raw request body string (never a reformatted object);
//   - requires a bounded channel id, resource id, and a positive message
//     number;
//   - requires a known resource state (sync | exists | updated | deleted);
//   - compares the channel token with a CONSTANT-TIME comparison
//     (timingSafeEqual) and fails closed when no channel token is configured.
//
// A verified notification is neutralized at the seam to
// CalendarWebhookVerification { resourceState } — provider semantics never
// cross into CRM. Replay dedupe of the actual data lives in the receipt
// (unique source identity), not here.
// ---------------------------------------------------------------------------

import { timingSafeEqual } from 'node:crypto'
import type { CalendarWebhookVerification } from '../contracts'
import { CALENDAR_WEBHOOK_RESOURCE_STATES } from '../contracts'

function headerValue(
  headers: Record<string, string | string[] | undefined>,
  name: string,
): string | undefined {
  const value = headers[name] ?? headers[name.toLowerCase()] ?? headers[name.toUpperCase()]
  if (value === undefined || value === null) return undefined
  return Array.isArray(value) ? value[0] : value
}

function bounded(value: string | undefined, field: string): string | null {
  if (value === undefined || value.trim() === '') return null
  const normalized = value.trim()
  if (normalized.length > 256 || /[\u0000-\u001f\u007f-\u009f]/.test(normalized)) {
    throw new Error(`Google Calendar webhook ${field} is invalid.`)
  }
  return normalized
}

function constantTimeTokenEqual(expected: string, received: string): boolean {
  const a = Buffer.from(expected, 'utf8')
  const b = Buffer.from(received, 'utf8')
  if (a.length !== b.length) return false
  return timingSafeEqual(a, b)
}

/**
 * Verify a Google Calendar push notification. Throws on any failure:
 * non-string payload, malformed headers, unknown resource state, missing
 * configured channel token, or a channel-token mismatch. Returns the NEUTRAL
 * resource state.
 */
export function verifyGoogleCalendarWebhook(
  payload: unknown,
  headers: Record<string, string | string[] | undefined>,
  configuredChannelToken: string | null,
): CalendarWebhookVerification {
  // Google notifications have an empty body, but the seam contract requires
  // the RAW body string (never a reformatted JSON object) so signature
  // material can never be confused with a parsed payload.
  if (typeof payload !== 'string') {
    throw new Error('Google Calendar webhook payload must be the raw request body string.')
  }

  const channelId = bounded(headerValue(headers, 'x-goog-channel-id'), 'channel id')
  if (!channelId) {
    throw new Error('Google Calendar webhook is missing X-Goog-Channel-ID.')
  }
  const resourceId = bounded(headerValue(headers, 'x-goog-resource-id'), 'resource id')
  if (!resourceId) {
    throw new Error('Google Calendar webhook is missing X-Goog-Resource-ID.')
  }

  const resourceState = bounded(
    headerValue(headers, 'x-goog-resource-state'),
    'resource state',
  )
  if (
    !resourceState ||
    !(CALENDAR_WEBHOOK_RESOURCE_STATES as readonly string[]).includes(resourceState)
  ) {
    throw new Error('Google Calendar webhook has an invalid X-Goog-Resource-State.')
  }

  const messageNumber = headerValue(headers, 'x-goog-message-number')
  const messageNumberValue = messageNumber === undefined ? NaN : Number(messageNumber)
  if (!Number.isInteger(messageNumberValue) || messageNumberValue <= 0) {
    throw new Error('Google Calendar webhook has an invalid X-Goog-Message-Number.')
  }

  if (configuredChannelToken === null) {
    throw new Error('Google Calendar webhook verification is not configured (channel token missing).')
  }
  const receivedToken = headerValue(headers, 'x-goog-channel-token')
  if (receivedToken === undefined || !constantTimeTokenEqual(configuredChannelToken, receivedToken)) {
    throw new Error('Google Calendar webhook channel token is invalid.')
  }

  return {
    verified: true,
    resourceState: resourceState as CalendarWebhookVerification['resourceState'],
    channelId,
  }
}
