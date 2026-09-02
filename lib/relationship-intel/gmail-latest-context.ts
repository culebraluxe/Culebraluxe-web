import type { CreateInteractionInput } from '../crm-types'

export const GMAIL_CONTEXT_SOURCE = 'gmail_contacts'
export const GMAIL_SUBJECT_MAX_LENGTH = 500

export type GmailMetadataMessage = {
  id: string
  threadId?: string
  internalDate?: string
  payload?: {
    headers?: Array<{ name?: string; value?: string }>
  }
}

export type GmailContextResult =
  | { ok: true; interaction: CreateInteractionInput }
  | {
      ok: false
      reason:
        | 'invalid_message'
        | 'missing_subject'
        | 'not_target_correspondence'
        | 'ambiguous_outbound'
    }

function normalizeEmail(value: string): string | null {
  const normalized = value.trim().toLowerCase()
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalized) ? normalized : null
}

/** Parse the email addresses from a bounded RFC-style address header. */
export function headerEmails(value: string | undefined): string[] {
  if (!value) return []
  const seen = new Set<string>()
  const matches = value.match(/[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi) ?? []
  for (const match of matches) {
    const email = normalizeEmail(match)
    if (email) seen.add(email)
  }
  return [...seen]
}

function headerMap(message: GmailMetadataMessage): Map<string, string> {
  const map = new Map<string, string>()
  for (const header of message.payload?.headers ?? []) {
    const name = header.name?.trim().toLowerCase()
    if (!name || typeof header.value !== 'string') continue
    map.set(name, header.value)
  }
  return map
}

function boundedSubject(value: string | undefined): string | null {
  if (!value) return null
  const subject = value.normalize('NFKC').replace(/[\u0000-\u001f\u007f-\u009f]+/g, ' ').replace(/\s+/g, ' ').trim()
  if (!subject) return null
  return subject.length <= GMAIL_SUBJECT_MAX_LENGTH
    ? subject
    : `${subject.slice(0, GMAIL_SUBJECT_MAX_LENGTH - 1).trimEnd()}…`
}

/**
 * Convert one Gmail metadata response into a context-only canonical interaction.
 * The target Person is already authoritatively linked by exact Gmail evidence.
 * Bodies, snippets, attachment descriptors, and raw MIME never enter this seam.
 */
export function gmailMetadataToContext(
  message: GmailMetadataMessage,
  targetEmailRaw: string,
  internalEmailRaw: string,
  canonicalPersonId: string,
): GmailContextResult {
  const targetEmail = normalizeEmail(targetEmailRaw)
  const internalEmail = normalizeEmail(internalEmailRaw)
  const occurredMs = Number(message.internalDate)
  if (!message.id?.trim() || !targetEmail || !internalEmail || !Number.isFinite(occurredMs)) {
    return { ok: false, reason: 'invalid_message' }
  }

  const headers = headerMap(message)
  const subject = boundedSubject(headers.get('subject'))
  if (!subject) return { ok: false, reason: 'missing_subject' }

  const from = headerEmails(headers.get('from'))
  const recipients = [
    ...headerEmails(headers.get('to')),
    ...headerEmails(headers.get('cc')),
    ...headerEmails(headers.get('bcc')),
  ]
  const uniqueRecipients = [...new Set(recipients)]

  let direction: 'inbound' | 'outbound'
  if (from.length === 1 && from[0] === targetEmail && uniqueRecipients.includes(internalEmail)) {
    direction = 'inbound'
  } else if (from.length === 1 && from[0] === internalEmail && uniqueRecipients.includes(targetEmail)) {
    const externalRecipients = uniqueRecipients.filter((email) => email !== internalEmail)
    if (externalRecipients.length !== 1 || externalRecipients[0] !== targetEmail) {
      return { ok: false, reason: 'ambiguous_outbound' }
    }
    direction = 'outbound'
  } else {
    return { ok: false, reason: 'not_target_correspondence' }
  }

  return {
    ok: true,
    interaction: {
      personId: canonicalPersonId,
      channel: 'email',
      eventType: direction === 'inbound' ? 'email_received' : 'email_sent',
      direction,
      occurredAt: new Date(occurredMs).toISOString(),
      title: subject,
      sourceSystem: GMAIL_CONTEXT_SOURCE,
      sourceExternalId: message.id,
      sourceMetadata: {
        sourceAccount: internalEmail,
        threadId: message.threadId ?? null,
        metadataOnly: true,
      },
    },
  }
}
