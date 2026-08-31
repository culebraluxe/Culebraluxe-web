// ---------------------------------------------------------------------------
// REL-INTEL — source-neutral relationship evidence contracts.
//
// One shared vocabulary for source-neutral relationship evidence so CORE
// surfaces (Catch-Up, Clients) can read neutral facts without knowing which
// source produced them. Provenance keeps every fact traceable to a source
// system, batch/event boundary, and immutable source identity.
// ---------------------------------------------------------------------------

export type RelationshipEvidenceSource =
  | 'apple_contacts'
  | 'gmail_contacts'
  | 'apple_messages'
  | 'whatsapp'

export type ReviewState =
  | 'unresolved'
  | 'exact_linked'
  | 'review_required'
  | 'ambiguous'
  | 'unmatched'
  | 'rejected'
  | 'non_person'
  | 'deferred'

export type MatchMethod =
  | 'exact_email'
  | 'exact_phone'
  | 'source_link'
  | 'review_candidate'
  | 'unmatched'
  | 'rejected'

export type MatchConfidence = 'exact' | 'probable' | 'ambiguous' | 'none'

export interface IdentityEvidence {
  /** original value exactly as it arrived from the source */
  value: string
  /** deterministic normalized value (email lowercased, phone digit-normalized) */
  normalized: string | null
  /** source label (e.g. Work / Mobile) when provided */
  label: string | null
}

export interface RelationshipEvidence {
  source: RelationshipEvidenceSource
  sourceAccount: string
  /** stable source relationship identity (contact id, address, or event-source Person key) */
  sourceIdentityKey: string
  sourceLabel?: string | null
  displayName?: string | null
  organization?: string | null
  emails: IdentityEvidence[]
  phones: IdentityEvidence[]

  // Communication evidence — never invented; null when the source cannot prove it.
  firstObservedAt?: string | null
  lastObservedAt?: string | null
  lastInboundAt?: string | null
  lastOutboundAt?: string | null
  inboundCount?: number | null
  outboundCount?: number | null
  isTwoWay?: boolean | null
  isOwnerInitiated?: boolean | null
  isAutomatedOrBulk?: boolean | null
  isOrganizationOrService?: boolean | null
  knownAppleContact?: boolean | null
  hasEmail: boolean
  hasPhone: boolean
  coverageNote?: string | null
}

export interface ReconcileDecision {
  reviewState: ReviewState
  matchMethod: MatchMethod
  matchConfidence: MatchConfidence
  canonicalPersonId?: string | null
  reason: string
  ruleVersion: string
}
