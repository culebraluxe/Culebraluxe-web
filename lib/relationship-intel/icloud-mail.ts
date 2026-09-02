import { createHash } from 'node:crypto'
import type { MessageAddressObject, MessageEnvelopeObject } from 'imapflow'
import type { CreateInteractionInput } from '../crm-types'
import type { RelationshipEvidence } from './contracts'

export const ICLOUD_MAIL_SOURCE = 'icloud_mail' as const
export const EMAIL_SUBJECT_MAX_LENGTH = 500

export type ICloudMailObservation = {
  sourceExternalId: string
  sourceAccount: string
  mailbox: string
  uid: number
  uidValidity: string
  occurredAt: string
  direction: 'inbound' | 'outbound'
  externalEmail: string
  displayName: string | null
  subject: string | null
}

export type EnvelopeClassification =
  | { ok: true; direction: 'inbound' | 'outbound'; externalEmail: string; displayName: string | null }
  | { ok: false; reason: 'internal_only' | 'unrelated' | 'ambiguous' }

export function normalizeMailbox(value: string): string | null {
  const normalized = value.trim().toLowerCase()
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalized) ? normalized : null
}

function normalizedAddresses(addresses: MessageAddressObject[] | undefined) {
  const seen = new Map<string, string | null>()
  for (const address of addresses ?? []) {
    const email = address.address ? normalizeMailbox(address.address) : null
    if (!email || seen.has(email)) continue
    const name = address.name?.normalize('NFKC').replace(/\s+/g, ' ').trim() || null
    seen.set(email, name)
  }
  return seen
}

export function classifyEnvelope(
  envelope: MessageEnvelopeObject,
  internalAddresses: ReadonlySet<string>,
): EnvelopeClassification {
  const senders = normalizedAddresses(envelope.from)
  const recipients = new Map([
    ...normalizedAddresses(envelope.to),
    ...normalizedAddresses(envelope.cc),
    ...normalizedAddresses(envelope.bcc),
  ])
  const senderEmails = [...senders.keys()]
  const recipientEmails = [...recipients.keys()]
  if (senderEmails.length !== 1) return { ok: false, reason: 'ambiguous' }

  const sender = senderEmails[0]
  if (internalAddresses.has(sender)) {
    const external = recipientEmails.filter((email) => !internalAddresses.has(email))
    if (external.length === 0) return { ok: false, reason: 'internal_only' }
    if (external.length !== 1) return { ok: false, reason: 'ambiguous' }
    return {
      ok: true,
      direction: 'outbound',
      externalEmail: external[0],
      displayName: recipients.get(external[0]) ?? null,
    }
  }

  if (!recipientEmails.some((email) => internalAddresses.has(email))) {
    return { ok: false, reason: 'unrelated' }
  }
  return {
    ok: true,
    direction: 'inbound',
    externalEmail: sender,
    displayName: senders.get(sender) ?? null,
  }
}

export function boundedEmailSubject(value: string | undefined): string | null {
  if (!value) return null
  const normalized = value
    .normalize('NFKC')
    .replace(/[\u0000-\u001f\u007f-\u009f]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  if (!normalized) return null
  return normalized.length <= EMAIL_SUBJECT_MAX_LENGTH
    ? normalized
    : `${normalized.slice(0, EMAIL_SUBJECT_MAX_LENGTH - 1).trimEnd()}…`
}

export function observationToInteraction(
  observation: ICloudMailObservation,
  canonicalPersonId: string,
): CreateInteractionInput {
  return {
    personId: canonicalPersonId,
    channel: 'email',
    eventType: observation.direction === 'inbound' ? 'email_received' : 'email_sent',
    direction: observation.direction,
    occurredAt: observation.occurredAt,
    title: observation.subject ?? undefined,
    sourceSystem: ICLOUD_MAIL_SOURCE,
    sourceExternalId: observation.sourceExternalId,
    sourceMetadata: {
      sourceAccount: observation.sourceAccount,
      mailbox: observation.mailbox,
      uid: observation.uid,
      uidValidity: observation.uidValidity,
      metadataOnly: true,
    },
  }
}

export function buildICloudMailEvidence(observations: ICloudMailObservation[]) {
  const groups = new Map<string, ICloudMailObservation[]>()
  for (const observation of observations) {
    const rows = groups.get(observation.externalEmail) ?? []
    rows.push(observation)
    groups.set(observation.externalEmail, rows)
  }

  return [...groups.entries()].map(([email, rows]) => {
    rows.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt))
    const inbound = rows.filter((row) => row.direction === 'inbound')
    const outbound = rows.filter((row) => row.direction === 'outbound')
    const evidence: RelationshipEvidence = {
      source: ICLOUD_MAIL_SOURCE,
      sourceAccount: rows[0].sourceAccount,
      sourceIdentityKey: email,
      sourceLabel: 'Apple-hosted work email',
      displayName: rows.find((row) => row.displayName)?.displayName ?? null,
      organization: null,
      emails: [{ value: email, normalized: email, label: 'Email' }],
      phones: [],
      firstObservedAt: rows[0].occurredAt,
      lastObservedAt: rows.at(-1)!.occurredAt,
      lastInboundAt: inbound.at(-1)?.occurredAt ?? null,
      lastOutboundAt: outbound.at(-1)?.occurredAt ?? null,
      inboundCount: inbound.length,
      outboundCount: outbound.length,
      isTwoWay: inbound.length > 0 && outbound.length > 0,
      isOwnerInitiated: outbound.length > 0,
      isAutomatedOrBulk: null,
      isOrganizationOrService: null,
      knownAppleContact: null,
      hasEmail: true,
      hasPhone: false,
      coverageNote: 'Apple iCloud Mail envelope metadata only; bodies, snippets, attachments, and raw MIME omitted.',
    }
    const fingerprint = createHash('sha256')
      .update(JSON.stringify({
        source: evidence.source,
        sourceAccount: evidence.sourceAccount,
        email,
        ids: rows.map((row) => row.sourceExternalId).sort(),
      }))
      .digest('hex')
    return { evidence, fingerprint }
  })
}
