// ---------------------------------------------------------------------------
// REL-INTEL — Apple Contacts -> neutral evidence projection (PURE).
//
// Pure, deterministic mapper with no database imports so it is unit-testable
// without a live Neon connection (matching the repository's pure-function test
// convention). The DB orchestrator lives in apple-evidence.ts.
//
// It never converts Apple contacts into canonical Clients; classification is
// left to the reconciliation engine.
// ---------------------------------------------------------------------------

import type { IdentityEvidence, RelationshipEvidence } from './contracts'
import { fingerprint } from './normalize'

export const APPLE_SOURCE = 'apple_contacts' as const

export interface ApplePersonInput {
  id: string
  sourceAccount: string
  sourceContactId: string
  displayName: string | null
  organization: string | null
  emails: IdentityEvidence[]
  phones: IdentityEvidence[]
  /** Organization/service-only contact (no person name) — never a canonical Person. */
  isOrganizationOrService?: boolean
}

/** Pure, deterministic projection of one Apple load row into neutral evidence. */
export function projectApplePersonToEvidence(input: ApplePersonInput): {
  evidence: RelationshipEvidence
  fingerprint: string
} {
  const evidence: RelationshipEvidence = {
    source: APPLE_SOURCE,
    sourceAccount: input.sourceAccount,
    sourceIdentityKey: input.sourceContactId,
    sourceLabel: null,
    displayName: input.displayName,
    organization: input.organization,
    emails: input.emails,
    phones: input.phones,
    firstObservedAt: null,
    lastObservedAt: null,
    lastInboundAt: null,
    lastOutboundAt: null,
    isTwoWay: null,
    isOwnerInitiated: null,
    isAutomatedOrBulk: null,
    isOrganizationOrService: input.isOrganizationOrService ?? null,
    knownAppleContact: true,
    hasEmail: input.emails.length > 0,
    hasPhone: input.phones.length > 0,
    coverageNote: null,
  }
  const fp = fingerprint(
    JSON.stringify({
      source: evidence.source,
      sourceAccount: evidence.sourceAccount,
      sourceIdentityKey: evidence.sourceIdentityKey,
      displayName: evidence.displayName,
      organization: evidence.organization,
      emails: evidence.emails.map((e) => `${e.normalized ?? e.value}|${e.label ?? ''}`).join(','),
      phones: evidence.phones.map((p) => `${p.normalized ?? p.value}|${p.label ?? ''}`).join(','),
    }),
  )
  return { evidence, fingerprint: fp }
}
