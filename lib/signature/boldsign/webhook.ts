// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: webhook signature verification + payload
// normalization.
//
// Faithful to BoldSign's documented webhook security model
// (developers.boldsign.com/webhooks/verify-webhook-events):
//   - every webhook carries an `X-BoldSign-Signature` header:
//         t=<epoch-seconds>, s0=<hmac-hex>[, s1=<hmac-hex>]
//     (s1 is only present while an old secret is still valid during a roll);
//   - the signed payload is `${t}.${rawBody}` — the RAW request body, exact
//     bytes, so verification REQUIRES the raw body string (never a
//     reformatted JSON object);
//   - the HMAC is SHA-256 keyed with the webhook signing secret, hex-encoded,
//     and compared with a CONSTANT-TIME comparison (timingSafeEqual);
//   - the timestamp is checked against a tolerance window to reject replays
//     of old events.
//
// The normalized form of a payload is provider-internal (event id, event type,
// envelope/document id, document status). The adapter maps it to a NEUTRAL
// event at the seam; provider vocabulary never crosses into canonical models.
// ---------------------------------------------------------------------------

import { createHmac, timingSafeEqual } from 'node:crypto'

export const BOLD_SIGN_SIGNATURE_HEADER = 'x-boldsign-signature'

export type BoldSignWebhookEvent = {
  /** The BoldSign webhook event id — the webhook replay dedupe key. */
  providerEventId: string
  /** Raw BoldSign event type (Sent, Viewed, Signed, Completed, ...). */
  eventType: string
  /** The BoldSign envelope/document id the event concerns. */
  envelopeId: string
  /** The document's status at event time (data.status), when present. */
  documentStatus: string | null
}

/**
 * Build a BoldSign-style signature header for a raw body (used by tests and
 * by the fake BoldSign server; mirrors BoldSign's server-side computation).
 */
export function signBoldSignWebhook(
  body: string,
  secret: string,
  timestampSeconds: number,
): string {
  const hmac = createHmac('sha256', secret)
    .update(`${timestampSeconds}.${body}`, 'utf8')
    .digest('hex')
  return `t=${timestampSeconds}, s0=${hmac}`
}

function parseSignatureHeader(header: string): {
  timestamp: number
  signatures: string[]
} {
  const parts = header.split(',')
  let timestamp: number | null = null
  const signatures: string[] = []
  for (const part of parts) {
    const eq = part.indexOf('=')
    if (eq <= 0) continue
    const key = part.slice(0, eq).trim()
    const value = part.slice(eq + 1).trim()
    if (key === 't') {
      const parsed = Number.parseInt(value, 10)
      if (Number.isInteger(parsed)) timestamp = parsed
    } else if (key.startsWith('s')) {
      signatures.push(value)
    }
  }
  if (timestamp === null || signatures.length === 0) {
    throw new Error('BoldSign webhook signature header is malformed (expected t=<ts>, s0=<hmac>).')
  }
  return { timestamp, signatures }
}

function constantTimeEqualHex(expectedHex: string, receivedHex: string): boolean {
  if (!/^[0-9a-f]+$/i.test(receivedHex)) return false
  const expected = Buffer.from(expectedHex, 'hex')
  const received = Buffer.from(receivedHex, 'hex')
  if (expected.length !== received.length) return false
  return timingSafeEqual(expected, received)
}

/**
 * Verify a BoldSign webhook signature over the RAW body. Throws on any
 * failure: malformed header, unknown timestamp, stale timestamp (outside the
 * tolerance window — replay protection), or an HMAC that matches none of the
 * header's signatures. Never logs the secret.
 */
export function verifyBoldSignWebhookSignature(
  rawBody: string,
  signatureHeader: string,
  secret: string,
  nowSeconds: number,
  toleranceSeconds: number,
): void {
  const { timestamp, signatures } = parseSignatureHeader(signatureHeader)
  const skew = Math.abs(nowSeconds - timestamp)
  if (skew > toleranceSeconds) {
    throw new Error(
      `BoldSign webhook signature timestamp is outside the allowed tolerance window (skew ${skew}s > ${toleranceSeconds}s).`,
    )
  }
  const expected = createHmac('sha256', secret)
    .update(`${timestamp}.${rawBody}`, 'utf8')
    .digest('hex')
  const matched = signatures.some((received) => constantTimeEqualHex(expected, received))
  if (!matched) {
    throw new Error('BoldSign webhook signature is invalid (HMAC mismatch).')
  }
}

/**
 * Normalize a raw BoldSign webhook body to the provider-internal event shape.
 * FAILS CLOSED on malformed payloads or a missing envelope/document id.
 */
export function parseBoldSignWebhookPayload(rawBody: string): BoldSignWebhookEvent {
  let parsed: unknown
  try {
    parsed = JSON.parse(rawBody)
  } catch {
    throw new Error('BoldSign webhook payload is not valid JSON.')
  }
  const body = parsed as {
    event?: { id?: unknown; eventType?: unknown }
    data?: { documentId?: unknown; status?: unknown }
  }
  const eventId = body?.event?.id
  const eventType = body?.event?.eventType
  const envelopeId = body?.data?.documentId
  const documentStatus = body?.data?.status
  if (typeof eventId !== 'string' || eventId.trim() === '') {
    throw new Error('BoldSign webhook payload is missing event.id.')
  }
  if (typeof eventType !== 'string' || eventType.trim() === '') {
    throw new Error('BoldSign webhook payload is missing event.eventType.')
  }
  if (typeof envelopeId !== 'string' || envelopeId.trim() === '') {
    throw new Error('BoldSign webhook payload is missing data.documentId.')
  }
  return {
    providerEventId: eventId,
    eventType,
    envelopeId,
    documentStatus: typeof documentStatus === 'string' && documentStatus.trim() !== ''
      ? documentStatus
      : null,
  }
}
