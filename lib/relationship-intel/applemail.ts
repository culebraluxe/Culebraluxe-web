import type { ICloudMailObservation } from './icloud-mail'
import { boundedEmailSubject, normalizeMailbox } from './icloud-mail'

// ---------------------------------------------------------------------------
// REL-INTEL — landed Apple Mail row -> neutral observation. PURE; no database.
//
// The intake writes source evidence into l_applemail. This turns a landed row
// back into the observation the evidence builder consumes, so promotion never
// re-reads Mail and never repeats the classification rules in two places.
//
// The rules are the ones the intake already applied:
//   * inbox -> inbound; the external sender is the counterparty
//   * sent  -> outbound; exactly ONE external recipient or it is ambiguous
//   * internal-only, unaddressed, or timestamp-less rows are skipped, never
//     invented. The landed row stays the source of truth either way.
// ---------------------------------------------------------------------------

export type MailAddress = { address?: string | null; name?: string | null }

/** The columns the promoter selects. Driver-native types are normalized on read. */
export type LandedAppleMailRow = {
  source_account: string
  source_message_id: string
  mailbox_kind: string | null
  mailbox_name: string | null
  local_id: number | string | null
  message_id: string | null
  occurred_at: string | Date | null
  sender: string | null
  to_recipients: MailAddress[] | null
  cc_recipients: MailAddress[] | null
  bcc_recipients: MailAddress[] | null
  subject: string | null
}

export type ObservationSkipReason =
  | 'no_timestamp'
  | 'unaddressed'
  | 'internal_only'
  | 'ambiguous'
  | 'duplicate'

export type LandedMailNormalization = {
  observations: ICloudMailObservation[]
  skipped: Record<ObservationSkipReason, number>
}

/** "Dana <dana@example.com>" -> { address, name }; a bare address also works. */
export function parseSenderAddress(
  value: string | null,
): { address: string; name: string | null } | null {
  if (!value) return null
  const bracketed = value.match(/^(.*?)<([^<>]+)>\s*$/)
  const address = normalizeMailbox(bracketed?.[2] ?? value.match(/[^\s<>]+@[^\s<>]+/)?.[0] ?? '')
  if (!address) return null
  // Trim FIRST, then strip the quotes: `"Dana Q" ` ends in a space, so anchoring a
  // closing-quote strip before the trim leaves a stray quote on the name.
  const rawName = bracketed?.[1]?.trim() ?? ''
  const name = rawName.replace(/^["']+/, '').replace(/["']+$/, '').trim() || null
  return { address, name }
}

function toIso(value: string | Date | null): string | null {
  if (!value) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

function externalRecipients(row: LandedAppleMailRow, internal: ReadonlySet<string>) {
  const external = new Map<string, string | null>()
  const all = [...(row.to_recipients ?? []), ...(row.cc_recipients ?? []), ...(row.bcc_recipients ?? [])]
  for (const recipient of all) {
    const address = recipient.address ? normalizeMailbox(recipient.address) : null
    if (!address || internal.has(address) || external.has(address)) continue
    external.set(address, recipient.name?.trim() || null)
  }
  return external
}

export function normalizeLandedMail(
  rows: readonly LandedAppleMailRow[],
  internalAddresses: ReadonlySet<string>,
): LandedMailNormalization {
  const skipped: Record<ObservationSkipReason, number> = {
    no_timestamp: 0,
    unaddressed: 0,
    internal_only: 0,
    ambiguous: 0,
    duplicate: 0,
  }
  const seen = new Set<string>()
  const observations: ICloudMailObservation[] = []

  for (const row of rows) {
    const occurredAt = toIso(row.occurred_at)
    if (!occurredAt) {
      skipped.no_timestamp += 1
      continue
    }

    const kind = (row.mailbox_kind ?? '').trim().toLowerCase()
    let direction: 'inbound' | 'outbound'
    let externalEmail: string
    let displayName: string | null

    if (kind === 'inbox' || kind === 'received') {
      const sender = parseSenderAddress(row.sender)
      if (!sender) {
        skipped.unaddressed += 1
        continue
      }
      if (internalAddresses.has(sender.address)) {
        skipped.internal_only += 1
        continue
      }
      direction = 'inbound'
      externalEmail = sender.address
      displayName = sender.name
    } else {
      const external = externalRecipients(row, internalAddresses)
      if (external.size === 0) {
        skipped.internal_only += 1
        continue
      }
      if (external.size > 1) {
        skipped.ambiguous += 1
        continue
      }
      const [address, name] = external.entries().next().value as [string, string | null]
      direction = 'outbound'
      externalEmail = address
      displayName = name
    }

    // One message can surface in two mailboxes (an inbox and a sent copy); the
    // landed source_message_id is the identity, so the second is a duplicate.
    if (seen.has(row.source_message_id)) {
      skipped.duplicate += 1
      continue
    }
    seen.add(row.source_message_id)

    observations.push({
      sourceExternalId: row.source_message_id,
      sourceAccount: row.source_account,
      mailbox: row.mailbox_name ?? kind,
      uid: row.local_id == null ? null : Number(row.local_id),
      uidValidity: 'apple-mail-local',
      occurredAt,
      direction,
      externalEmail,
      displayName,
      subject: boundedEmailSubject(row.subject ?? undefined),
    })
  }

  return { observations, skipped }
}
