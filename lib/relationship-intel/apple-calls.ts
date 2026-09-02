import type { RelationshipEvidence } from './contracts'
import { fingerprint } from './normalize'
import { handleToIdentities } from './apple-messages'

export const APPLE_CALLS_SOURCE = 'apple_calls' as const
export const APPLE_FACETIME_SOURCE = 'apple_facetime' as const

export type AppleCallRecord = {
  rowid: number
  uniqueId: string
  address: string | null
  dateRaw: number | null
  dateISO: string | null
  duration: number | null
  originated: number | boolean | null
  answered: number | boolean | null
  callType: string | number | null
  serviceProvider: string | null
  countryCode: string | null
}

export function isFaceTimeCall(call: AppleCallRecord): boolean {
  const provider = String(call.serviceProvider ?? '').toLowerCase()
  const callType = String(call.callType ?? '').toLowerCase()
  return provider.includes('facetime') || callType.includes('facetime')
}

export function callSource(call: AppleCallRecord) {
  return isFaceTimeCall(call) ? APPLE_FACETIME_SOURCE : APPLE_CALLS_SOURCE
}

export function callDirection(call: AppleCallRecord): 'inbound' | 'outbound' {
  return call.originated === true || call.originated === 1 ? 'outbound' : 'inbound'
}

export function buildAppleCallRelationshipEvidence(
  calls: AppleCallRecord[],
  sourceAccount = 'apple_call_history_local',
): Array<{ evidence: RelationshipEvidence; fingerprint: string }> {
  const groups = new Map<string, AppleCallRecord[]>()
  for (const call of calls) {
    const address = call.address?.trim()
    if (!address) continue
    const source = callSource(call)
    const key = `${source}\u0000${address}`
    const rows = groups.get(key) ?? []
    rows.push(call)
    groups.set(key, rows)
  }

  const out: Array<{ evidence: RelationshipEvidence; fingerprint: string }> = []
  for (const [key, rows] of groups) {
    const [source, address] = key.split('\u0000') as [typeof APPLE_CALLS_SOURCE | typeof APPLE_FACETIME_SOURCE, string]
    const { emails, phones } = handleToIdentities(address)
    let firstObservedAt: string | null = null
    let lastObservedAt: string | null = null
    let lastInboundAt: string | null = null
    let lastOutboundAt: string | null = null
    let inboundCount = 0
    let outboundCount = 0

    for (const row of rows) {
      const direction = callDirection(row)
      if (direction === 'outbound') outboundCount += 1
      else inboundCount += 1
      if (!row.dateISO) continue
      if (!firstObservedAt || row.dateISO < firstObservedAt) firstObservedAt = row.dateISO
      if (!lastObservedAt || row.dateISO > lastObservedAt) lastObservedAt = row.dateISO
      if (direction === 'outbound') {
        if (!lastOutboundAt || row.dateISO > lastOutboundAt) lastOutboundAt = row.dateISO
      } else if (!lastInboundAt || row.dateISO > lastInboundAt) {
        lastInboundAt = row.dateISO
      }
    }

    const evidence: RelationshipEvidence = {
      source,
      sourceAccount,
      sourceIdentityKey: address,
      sourceLabel: source === APPLE_FACETIME_SOURCE ? 'FaceTime' : 'Phone',
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
      isTwoWay: inboundCount > 0 && outboundCount > 0 ? true : rows.length > 0 ? false : null,
      isOwnerInitiated: outboundCount > 0 ? true : null,
      isAutomatedOrBulk: null,
      isOrganizationOrService: null,
      knownAppleContact: false,
      hasEmail: emails.length > 0,
      hasPhone: phones.length > 0,
      coverageNote: 'Apple CallHistoryDB read-only export',
    }

    out.push({
      evidence,
      fingerprint: fingerprint(JSON.stringify({
        source,
        sourceAccount,
        address,
        firstObservedAt,
        lastObservedAt,
        lastInboundAt,
        lastOutboundAt,
        inboundCount,
        outboundCount,
        ids: rows.map((r) => r.uniqueId).sort(),
      })),
    })
  }
  return out
}
