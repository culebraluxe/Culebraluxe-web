// ---------------------------------------------------------------------------
// INTAKE — Gmail REST request shaping and metadata -> landing mapping.
//
// Pure and testable: the exact request shape (bounded page size, metadata-only
// format, selected headers) and the metadata -> l_email mapping live here, so the
// privacy boundary is assertable without a network or a database.
//
// Privacy boundary, deliberately enforced at the request shape:
//   * messages.list  -> bounded page (maxResults <= 500), never every id
//   * messages.get   -> format=metadata + named headers ONLY
//   * never full / raw / body / snippet / attachment content
// ---------------------------------------------------------------------------

import type { LandedEmail } from '../../db/landing'
import type { GmailMetadataMessage } from '../relationship-intel/gmail-latest-context'
import { GMAIL_MAX_RESULTS, GMAIL_METADATA_HEADERS } from './mailbox-paging'

/** Authenticated Gmail REST base. `me` is the token's own mailbox. */
export const GMAIL_API_BASE = 'https://gmail.googleapis.com/gmail/v1/users/me'

/** The Gmail mailbox this census intake is scoped to. */
export const DEFAULT_GMAIL_MAILBOX = 'penfield33@gmail.com'

/** messages.list path for ONE bounded page. maxResults is never > 500. */
export function gmailListPath(query: string, pageToken: string | null): string {
  const params = new URLSearchParams({ maxResults: String(GMAIL_MAX_RESULTS), q: query })
  if (pageToken) params.set('pageToken', pageToken)
  return `messages?${params.toString()}`
}

/** messages.get path: metadata format and named headers only. No full/raw/body. */
export function gmailMetadataPath(id: string): string {
  const params = new URLSearchParams({ format: 'metadata' })
  for (const header of GMAIL_METADATA_HEADERS) params.append('metadataHeaders', header)
  return `messages/${encodeURIComponent(id)}?${params.toString()}`
}

/** Case-insensitive header lookup on a metadata response. */
export function gmailHeader(message: GmailMetadataMessage, name: string): string | null {
  const wanted = name.toLowerCase()
  const header = (message.payload?.headers ?? []).find((h) => (h.name ?? '').toLowerCase() === wanted)
  const value = header?.value
  return typeof value === 'string' && value.trim() ? value : null
}

/** Gmail internalDate (epoch ms string) -> ISO, or null when unusable. */
export function gmailSentAt(message: GmailMetadataMessage): string | null {
  const ms = Number(message.internalDate)
  return Number.isFinite(ms) && ms > 0 ? new Date(ms).toISOString() : null
}

/**
 * Map one metadata-only message into a landing row. Returns null when the
 * message carries no usable source id (it could not be replayed safely).
 * `raw` is the untouched metadata response - a landing row is evidence.
 */
export function gmailMetadataToLandedEmail(
  message: GmailMetadataMessage,
  sourceAccount: string,
): LandedEmail | null {
  const id = message.id?.trim()
  if (!id) return null
  return {
    sourceAccount,
    sourceMessageId: id,
    threadId: message.threadId ?? null,
    fromAddress: gmailHeader(message, 'From'),
    toAddress: gmailHeader(message, 'To'),
    subject: gmailHeader(message, 'Subject'),
    sentAt: gmailSentAt(message),
    bodyPreview: null,
    labels: null,
    raw: message,
  }
}

/**
 * Fail closed: the authenticated profile MUST be the expected mailbox. Never
 * silently ingest another Google account.
 */
export function assertGmailAccount(actual: string | null | undefined, expected: string): void {
  const normalizedActual = (actual ?? '').trim().toLowerCase()
  const normalizedExpected = expected.trim().toLowerCase()
  if (!normalizedActual) throw new Error('Gmail profile did not report an email address.')
  if (normalizedActual !== normalizedExpected) {
    throw new Error(
      `Gmail token resolves to ${normalizedActual}, but this intake is scoped to ${normalizedExpected}. Refusing to ingest another mailbox.`,
    )
  }
}
