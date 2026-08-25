// ---------------------------------------------------------------------------
// Apple Messages — pure adapter that maps a local export package into the
// EXISTING source-neutral REL-INTEL evidence shape (migration 074). It does
// NOT create a parallel intake system: each Messages handle becomes one
// `integration_relationship_evidence` row via the canonical upsert, and
// message timestamps/directions become communication evidence.
//
// Handle identities stay Apple-source values here; normalization into phone /
// email belongs to the existing neutral normalization + reconciliation.
// ---------------------------------------------------------------------------
import type { IdentityEvidence, RelationshipEvidence } from './contracts'
import { normalizeEmail, normalizePhone, fingerprint } from './normalize'

export const APPLE_MESSAGES_SOURCE = 'apple_messages' as const

export type AppleMessagesHandle = {
  rowid: number
  id: string
  country: string | null
  service: string | null
  uncanonicalizedId: string | null
  personCentricId: string | null
}

export type AppleMessagesMessage = {
  rowid: number
  guid: string
  chatGuid: string | null
  handleId: number | null
  handleValue: string | null
  service: string | null
  date: number | null
  dateISO: string | null
  isFromMe: number | null
  text: string | null
  hasAttachments: number | null
}

export type AppleMessagesExport = {
  sourceAccount: string
  handles: AppleMessagesHandle[]
  messages: AppleMessagesMessage[]
}

/** Split an Apple handle id into neutral email/phone identity evidence. */
export function handleToIdentities(handleId: string): {
  emails: IdentityEvidence[]
  phones: IdentityEvidence[]
} {
  const emails: IdentityEvidence[] = []
  const phones: IdentityEvidence[] = []
  const raw = handleId.trim()
  if (!raw) return { emails, phones }
  const email = normalizeEmail(raw)
  if (email.ok) {
    emails.push({ value: raw, normalized: email.value, label: null })
    return { emails, phones }
  }
  const phone = normalizePhone(raw)
  if (phone.ok) {
    phones.push({ value: raw, normalized: phone.value, label: null })
    return { emails, phones }
  }
  // Unclassifiable (group id, urn:biz, short name, etc.) -> NO fabricated
  // identity. It stays hasEmail=false / hasPhone=false (reconcile -> deferred).
  return { emails, phones }
}

/**
 * Aggregate the bounded message set into one neutral RelationshipEvidence row
 * per handle, with its deterministic replay fingerprint. Communication
 * evidence is derived ONLY from real message timestamps/directions; it is
 * never invented.
 */
export function buildMessagesRelationshipEvidence(exportData: AppleMessagesExport): Array<{
  evidence: RelationshipEvidence
  fingerprint: string
}> {
  const byHandle = new Map<number, AppleMessagesMessage[]>()
  for (const m of exportData.messages) {
    if (m.handleId == null) continue
    ;(byHandle.get(m.handleId) ?? byHandle.set(m.handleId, []).get(m.handleId)!).push(m)
  }

  const out: Array<{ evidence: RelationshipEvidence; fingerprint: string }> = []
  for (const handle of exportData.handles) {
    const messages = byHandle.get(handle.rowid) ?? []
    const { emails, phones } = handleToIdentities(handle.id)

    let firstObservedAt: string | null = null
    let lastObservedAt: string | null = null
    let lastInboundAt: string | null = null
    let lastOutboundAt: string | null = null
    let inboundCount = 0
    let outboundCount = 0
    // Counts derive from EVERY real message (direction is independent of a
    // usable timestamp). Dates only drive observed windows.
    for (const m of messages) {
      if (m.isFromMe === 1) outboundCount += 1
      else inboundCount += 1
      if (m.dateISO) {
        const iso = m.dateISO
        if (!firstObservedAt || iso < firstObservedAt) firstObservedAt = iso
        if (!lastObservedAt || iso > lastObservedAt) lastObservedAt = iso
        if (m.isFromMe === 1) {
          if (!lastOutboundAt || iso > lastOutboundAt) lastOutboundAt = iso
        } else {
          if (!lastInboundAt || iso > lastInboundAt) lastInboundAt = iso
        }
      }
    }
    const isTwoWay = inboundCount > 0 && outboundCount > 0

    const evidence: RelationshipEvidence = {
      source: APPLE_MESSAGES_SOURCE,
      sourceAccount: exportData.sourceAccount,
      sourceIdentityKey: handle.id,
      sourceLabel: handle.service ?? null,
      displayName: null,
      organization: null,
      emails,
      phones,
      firstObservedAt,
      lastObservedAt,
      lastInboundAt,
      lastOutboundAt,
      inboundCount,
      outboundCount,
      isTwoWay: isTwoWay ? true : (inboundCount + outboundCount > 0 ? false : null),
      isOwnerInitiated: null,
      isAutomatedOrBulk: null,
      isOrganizationOrService: null,
      knownAppleContact: false,
      hasEmail: emails.length > 0,
      hasPhone: phones.length > 0,
      coverageNote: null,
    }
    const fp = fingerprint(
      JSON.stringify({
        source: evidence.source,
        sourceAccount: evidence.sourceAccount,
        sourceIdentityKey: evidence.sourceIdentityKey,
        emails: evidence.emails.map((e) => e.normalized).join(','),
        phones: evidence.phones.map((p) => p.normalized).join(','),
        firstObservedAt,
        lastObservedAt,
        lastInboundAt,
        lastOutboundAt,
        inboundCount,
        outboundCount,
      }),
    )
    out.push({ evidence, fingerprint: fp })
  }
  return out
}

