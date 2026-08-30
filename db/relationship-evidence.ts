import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  ReconcileDecision,
  RelationshipEvidence,
  ReviewState,
} from '../lib/relationship-intel/contracts'
import { reconcileEvidence } from '../lib/relationship-intel/reconcile'
import { createInMemoryPersonLookup } from '../lib/relationship-intel/inmemory-lookup'
import { mergeReconcileDecision } from '../lib/relationship-intel/link-safety'

// ---------------------------------------------------------------------------
// REL-INTEL — relationship-evidence repository (migration 074).
//
// Server-only access to the source-neutral ODS evidence seam. Reads are
// bounded (per canonical Person or a filtered OPPS review page); nothing here
// ever writes canonical person / person_identity. Human-approved promotion
// must go through the existing canonical command/receipt seam.
// ---------------------------------------------------------------------------

export type RelationshipEvidenceRow = RelationshipEvidence & {
  id: string
  reviewState: ReviewState
  matchMethod: string | null
  matchConfidence: string | null
  canonicalPersonId: string | null
  matchReason: string | null
  ruleVersion: string | null
  evidenceFingerprint: string
  updatedAt: string
}

type EvidenceRow = {
  id: string
  source: string
  source_account: string
  source_identity_key: string
  source_label: string | null
  display_name: string | null
  organization: string | null
  emails: unknown
  phones: unknown
  first_observed_at: string | null
  last_observed_at: string | null
  last_inbound_at: string | null
  last_outbound_at: string | null
  inbound_count: number | null
  outbound_count: number | null
  is_two_way: boolean | null
  is_owner_initiated: boolean | null
  is_automated_or_bulk: boolean | null
  is_organization_or_service: boolean | null
  known_apple_contact: boolean | null
  has_email: boolean
  has_phone: boolean
  coverage_note: string | null
  canonical_person_id: string | null
  match_method: string | null
  match_confidence: string | null
  review_state: string
  match_reason: string | null
  rule_version: string | null
  evidence_fingerprint: string
  updated_at: string
}

/** Safe ISO normalization for nullable timestamptz values. The Neon driver may
 *  return these as JS Date objects; repositories must emit ISO strings (or null)
 *  so downstream projections/comparators (e.g. `.localeCompare`) never receive a
 *  Date. */
function toIsoOrNull(value: string | Date | null | undefined): string | null {
  if (value === null || value === undefined) return null
  const d = value instanceof Date ? value : new Date(value)
  return Number.isNaN(d.getTime()) ? null : d.toISOString()
}

function mapRow(row: EvidenceRow): RelationshipEvidenceRow {
  return {
    id: row.id,
    source: row.source as RelationshipEvidence['source'],
    sourceAccount: row.source_account,
    sourceIdentityKey: row.source_identity_key,
    sourceLabel: row.source_label,
    displayName: row.display_name,
    organization: row.organization,
    emails: Array.isArray(row.emails) ? row.emails : [],
    phones: Array.isArray(row.phones) ? row.phones : [],
    firstObservedAt: toIsoOrNull(row.first_observed_at),
    lastObservedAt: toIsoOrNull(row.last_observed_at),
    lastInboundAt: toIsoOrNull(row.last_inbound_at),
    lastOutboundAt: toIsoOrNull(row.last_outbound_at),
    inboundCount: row.inbound_count,
    outboundCount: row.outbound_count,
    isTwoWay: row.is_two_way,
    isOwnerInitiated: row.is_owner_initiated,
    isAutomatedOrBulk: row.is_automated_or_bulk,
    isOrganizationOrService: row.is_organization_or_service,
    knownAppleContact: row.known_apple_contact,
    hasEmail: row.has_email,
    hasPhone: row.has_phone,
    coverageNote: row.coverage_note,
    reviewState: row.review_state as ReviewState,
    matchMethod: row.match_method,
    matchConfidence: row.match_confidence,
    canonicalPersonId: row.canonical_person_id,
    matchReason: row.match_reason,
    ruleVersion: row.rule_version,
    evidenceFingerprint: row.evidence_fingerprint,
    updatedAt: toIsoOrNull(row.updated_at) ?? '',
  }
}

/** Upsert one neutral evidence row (replay-safe via the identity unique key). */
export async function upsertRelationshipEvidence(
  evidence: RelationshipEvidence,
  fingerprint: string,
  intakeBatchId?: string,
  execute: QueryExecutor = sql,
): Promise<string> {
  const rows = (await execute`
    insert into integration_relationship_evidence (
      integration_intake_batch_id,
      source, source_account, source_identity_key, source_label,
      display_name, organization, emails, phones,
      first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, is_two_way, is_owner_initiated,
      is_automated_or_bulk, is_organization_or_service, known_apple_contact,
      has_email, has_phone, coverage_note, evidence_fingerprint
    ) values (
      ${intakeBatchId ?? null},
      ${evidence.source}, ${evidence.sourceAccount}, ${evidence.sourceIdentityKey},
      ${evidence.sourceLabel ?? null}, ${evidence.displayName ?? null},
      ${evidence.organization ?? null}, ${JSON.stringify(evidence.emails)},
      ${JSON.stringify(evidence.phones)},
      ${evidence.firstObservedAt ?? null}, ${evidence.lastObservedAt ?? null},
      ${evidence.lastInboundAt ?? null}, ${evidence.lastOutboundAt ?? null},
      ${evidence.inboundCount ?? null}, ${evidence.outboundCount ?? null},
      ${evidence.isTwoWay ?? null}, ${evidence.isOwnerInitiated ?? null},
      ${evidence.isAutomatedOrBulk ?? null}, ${evidence.isOrganizationOrService ?? null},
      ${evidence.knownAppleContact ?? null}, ${evidence.hasEmail},
      ${evidence.hasPhone}, ${evidence.coverageNote ?? null}, ${fingerprint}
    )
    on conflict (source, source_account, source_identity_key) do update set
      integration_intake_batch_id = excluded.integration_intake_batch_id,
      source_label = excluded.source_label,
      display_name = excluded.display_name,
      organization = excluded.organization,
      emails = excluded.emails,
      phones = excluded.phones,
      first_observed_at = excluded.first_observed_at,
      last_observed_at = excluded.last_observed_at,
      last_inbound_at = excluded.last_inbound_at,
      last_outbound_at = excluded.last_outbound_at,
      inbound_count = excluded.inbound_count,
      outbound_count = excluded.outbound_count,
      is_two_way = excluded.is_two_way,
      is_owner_initiated = excluded.is_owner_initiated,
      is_automated_or_bulk = excluded.is_automated_or_bulk,
      is_organization_or_service = excluded.is_organization_or_service,
      known_apple_contact = excluded.known_apple_contact,
      has_email = excluded.has_email,
      has_phone = excluded.has_phone,
      coverage_note = excluded.coverage_note,
      evidence_fingerprint = excluded.evidence_fingerprint,
      updated_at = now()
    returning id
  `) as { id: string }[]
  return rows[0]?.id ?? ''
}

/** Record a reconciliation decision for one evidence row. */
export async function recordReconcileDecision(
  id: string,
  decision: ReconcileDecision,
  execute: QueryExecutor = sql,
): Promise<void> {
  // Read the current durable link first: an established source->Person link is
  // owned for good. mergeReconcileDecision enforces match-once/enrich-forever so
  // automated reconciliation can NEVER clear, redirect, or merge an established
  // canonical relationship. It only surfaces a conflict truthfully.
  const current = (await execute`
    select canonical_person_id
    from integration_relationship_evidence
    where id = ${id}
    limit 1
  `) as { canonical_person_id: string | null }[]

  const write = mergeReconcileDecision(current[0]?.canonical_person_id ?? null, decision)

  await execute`
    update integration_relationship_evidence
      set canonical_person_id = ${write.canonicalPersonId},
          match_method = ${write.matchMethod},
          match_confidence = ${write.matchConfidence},
          review_state = ${write.reviewState},
          match_reason = ${write.reason},
          rule_version = ${decision.ruleVersion},
          updated_at = now()
    where id = ${id}
  `
}

/** Relationship evidence for one canonical Person (CORE read model). */
export async function getRelationshipEvidenceForPerson(
  personId: string,
  execute: QueryExecutor = sql,
): Promise<RelationshipEvidenceRow[]> {
  const rows = (await execute`
    select
      id, source, source_account, source_identity_key, source_label,
      display_name, organization, emails, phones,
      first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, is_two_way, is_owner_initiated,
      is_automated_or_bulk, is_organization_or_service, known_apple_contact,
      has_email, has_phone, coverage_note,
      canonical_person_id, match_method, match_confidence, review_state,
      match_reason, rule_version, evidence_fingerprint, updated_at
    from integration_relationship_evidence
    where canonical_person_id = ${personId}
    order by coalesce(last_observed_at, created_at) desc nulls last
  `) as EvidenceRow[]
  return rows.map(mapRow)
}

/**
 * All relationship evidence rows for a source (bounded stewardship/load tooling).
 * Used by the DEV load/reconcile scripts and the OPPS "rerun" path to re-read the
 * neutral evidence and re-apply the deterministic reconciliation pass.
 */
export async function getRelationshipEvidenceRows(
  source?: string,
  execute: QueryExecutor = sql,
): Promise<RelationshipEvidenceRow[]> {
  const rows = source
    ? ((await execute`
        select
          id, source, source_account, source_identity_key, source_label,
          display_name, organization, emails, phones,
          first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
          inbound_count, outbound_count, is_two_way, is_owner_initiated,
          is_automated_or_bulk, is_organization_or_service, known_apple_contact,
          has_email, has_phone, coverage_note,
          canonical_person_id, match_method, match_confidence, review_state,
          match_reason, rule_version, evidence_fingerprint, updated_at
        from integration_relationship_evidence
        where source = ${source}
        order by coalesce(last_observed_at, created_at) desc nulls last
      `) as EvidenceRow[])
    : ((await execute`
        select
          id, source, source_account, source_identity_key, source_label,
          display_name, organization, emails, phones,
          first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
          inbound_count, outbound_count, is_two_way, is_owner_initiated,
          is_automated_or_bulk, is_organization_or_service, known_apple_contact,
          has_email, has_phone, coverage_note,
          canonical_person_id, match_method, match_confidence, review_state,
          match_reason, rule_version, evidence_fingerprint, updated_at
        from integration_relationship_evidence
        order by coalesce(last_observed_at, created_at) desc nulls last
      `) as EvidenceRow[])
  return rows.map(mapRow)
}

/**
 * OPPS review page — filter by reconciliation outcome + optional search.
 * Filters are applied server-side over a bounded recent window (the evidence
 * set is small and occasional stewardship only); the returned page is bounded.
 */
export async function getRelationshipEvidenceReview(
  opts: {
    reviewState?: ReviewState | 'all'
    search?: string
    limit?: number
    offset?: number
  },
  execute: QueryExecutor = sql,
): Promise<{ rows: RelationshipEvidenceRow[]; total: number }> {
  const reviewState = opts.reviewState ?? 'all'
  const needle = (opts.search ?? '').trim().toLowerCase()
  const limit = Math.max(1, Math.min(100, opts.limit ?? 50))
  const offset = Math.max(0, opts.offset ?? 0)

  const rows = (await execute`
    select
      id, source, source_account, source_identity_key, source_label,
      display_name, organization, emails, phones,
      first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, is_two_way, is_owner_initiated,
      is_automated_or_bulk, is_organization_or_service, known_apple_contact,
      has_email, has_phone, coverage_note,
      canonical_person_id, match_method, match_confidence, review_state,
      match_reason, rule_version, evidence_fingerprint, updated_at
    from integration_relationship_evidence
    order by coalesce(last_observed_at, created_at) desc nulls last
    limit 10000
  `) as EvidenceRow[]

  const mapped = rows.map(mapRow)
  const filtered = mapped.filter((row) => {
    if (reviewState !== 'all' && row.reviewState !== reviewState) return false
    if (needle) {
      const haystack = [
        row.displayName,
        row.organization,
        row.sourceIdentityKey,
      ]
        .filter(Boolean)
        .join(' ')
        .toLowerCase()
      if (!haystack.includes(needle)) return false
    }
    return true
  })

  return { rows: filtered.slice(offset, offset + limit), total: filtered.length }
}

/** Build a plain RelationshipEvidence from a persisted row (for re-reconciliation). */
function toReconcileEvidence(row: RelationshipEvidenceRow): RelationshipEvidence {
  return {
    source: row.source,
    sourceAccount: row.sourceAccount,
    sourceIdentityKey: row.sourceIdentityKey,
    sourceLabel: row.sourceLabel,
    displayName: row.displayName,
    organization: row.organization,
    emails: row.emails,
    phones: row.phones,
    firstObservedAt: row.firstObservedAt,
    lastObservedAt: row.lastObservedAt,
    lastInboundAt: row.lastInboundAt,
    lastOutboundAt: row.lastOutboundAt,
    inboundCount: row.inboundCount,
    outboundCount: row.outboundCount,
    isTwoWay: row.isTwoWay,
    isOwnerInitiated: row.isOwnerInitiated,
    isAutomatedOrBulk: row.isAutomatedOrBulk,
    isOrganizationOrService: row.isOrganizationOrService,
    knownAppleContact: row.knownAppleContact,
    hasEmail: row.hasEmail,
    hasPhone: row.hasPhone,
    coverageNote: row.coverageNote,
  }
}

/**
 * Relationship evidence for a set of canonical persons (bounded Catch-Up surface).
 * One bounded read for the people in a Catch-Up snapshot.
 */
export async function getRelationshipEvidenceForPersons(
  personIds: string[],
  execute: QueryExecutor = sql,
): Promise<Record<string, RelationshipEvidenceRow[]>> {
  if (personIds.length === 0) return {}
  const rows = (await execute`
    select
      id, source, source_account, source_identity_key, source_label,
      display_name, organization, emails, phones,
      first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, is_two_way, is_owner_initiated,
      is_automated_or_bulk, is_organization_or_service, known_apple_contact,
      has_email, has_phone, coverage_note,
      canonical_person_id, match_method, match_confidence, review_state,
      match_reason, rule_version, evidence_fingerprint, updated_at
    from integration_relationship_evidence
    where canonical_person_id = any (${personIds})
    order by coalesce(last_observed_at, created_at) desc nulls last
  `) as EvidenceRow[]
  const out: Record<string, RelationshipEvidenceRow[]> = {}
  for (const r of rows) {
    const pid = r.canonical_person_id ?? ''
    if (!pid) continue
    ;(out[pid] ??= []).push(mapRow(r))
  }
  return out
}

/** OPPS — fetch a single evidence row by id (inspect match reason / provenance). */
export async function getRelationshipEvidenceById(
  id: string,
  execute: QueryExecutor = sql,
): Promise<RelationshipEvidenceRow | null> {
  const rows = (await execute`
    select
      id, source, source_account, source_identity_key, source_label,
      display_name, organization, emails, phones,
      first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
      inbound_count, outbound_count, is_two_way, is_owner_initiated,
      is_automated_or_bulk, is_organization_or_service, known_apple_contact,
      has_email, has_phone, coverage_note,
      canonical_person_id, match_method, match_confidence, review_state,
      match_reason, rule_version, evidence_fingerprint, updated_at
    from integration_relationship_evidence
    where id = ${id}
    limit 1
  `) as EvidenceRow[]
  return rows[0] ? mapRow(rows[0]) : null
}

/**
 * OPPS — classify an evidence row (mark automated/bulk or organization/service)
 * so an operator can correct a source classification, then re-run reconciliation.
 */
export async function classifyEvidenceRow(
  id: string,
  flags: {
    isAutomatedOrBulk?: boolean | null
    isOrganizationOrService?: boolean | null
  },
  execute: QueryExecutor = sql,
): Promise<void> {
  await execute`
    update integration_relationship_evidence
      set is_automated_or_bulk = coalesce(${flags.isAutomatedOrBulk ?? null}, is_automated_or_bulk),
          is_organization_or_service = coalesce(${flags.isOrganizationOrService ?? null}, is_organization_or_service),
          updated_at = now()
    where id = ${id}
  `
}

/**
 * OPPS — safely re-run the deterministic reconciliation pass over a bounded
 * subset (by id list, source, and/or review state). Re-reads the neutral
 * evidence, applies the real reconcileEvidence engine with an in-memory lookup
 * preloaded from person_identity, records decisions via the sanctioned seam, and
 * returns the outcome tally. Never writes canonical tables directly.
 */
export async function rerunRelationshipReconciliation(
  opts: {
    ids?: string[]
    source?: string
    reviewState?: ReviewState
    limit?: number
  },
  execute: QueryExecutor = sql,
): Promise<{ rows: RelationshipEvidenceRow[]; tally: Record<ReviewState, number>; canonicalLinked: number }> {
  const limit = Math.max(1, Math.min(500, opts.limit ?? 200))
  const idSet = opts.ids && opts.ids.length > 0 ? new Set(opts.ids) : null
  const all = await getRelationshipEvidenceRows(opts.source, execute)
  let rows = all
  if (opts.reviewState) rows = rows.filter((r) => r.reviewState === opts.reviewState)
  if (idSet) rows = rows.filter((r) => idSet.has(r.id))
  rows = rows.slice(0, limit)

  const { lookup } = await createInMemoryPersonLookup(execute)
  const tally: Record<ReviewState, number> = {
    unresolved: 0, exact_linked: 0, review_required: 0, ambiguous: 0,
    unmatched: 0, rejected: 0, non_person: 0, deferred: 0,
  }
  let canonicalLinked = 0

  for (const row of rows) {
    const decision = await reconcileEvidence(toReconcileEvidence(row), lookup)
    await recordReconcileDecision(row.id, decision, execute)
    tally[decision.reviewState] += 1
    if (decision.reviewState === 'exact_linked' && decision.canonicalPersonId) {
      canonicalLinked += 1
    }
  }

  return { rows, tally, canonicalLinked }
}

export interface IntakeBatchInput {
  source: string
  sourceAccount: string
  externalBatchId: string
  schemaVersion: number
  exportedAt?: string | null
  fileSha256: string
  inputCount: number
  validCount: number
  newProfileCount: number
  replayCount: number
  changedRevisionCount: number
  errorCount: number
}

/**
 * Create (or refresh on replay) one integration_intake_batch row. The row must
 * satisfy the batch balance CHECK: input_count = valid_count + error_count and
 * valid_count = new + replay + changed. Replay with the same external_batch_id
 * refreshes the same row (idempotent).
 */
export async function createIntakeBatch(
  input: IntakeBatchInput,
  execute: QueryExecutor = sql,
): Promise<string> {
  const rows = (await execute`
    insert into integration_intake_batch (
      source, source_account, external_batch_id, schema_version, exported_at,
      received_at, file_sha256, input_count, valid_count, new_profile_count,
      replay_count, changed_revision_count, error_count, load_status
    ) values (
      ${input.source}, ${input.sourceAccount}, ${input.externalBatchId},
      ${input.schemaVersion}, ${input.exportedAt ?? null}, now(),
      ${input.fileSha256}, ${input.inputCount}, ${input.validCount},
      ${input.newProfileCount}, ${input.replayCount},
      ${input.changedRevisionCount}, ${input.errorCount}, 'loaded'
    )
    on conflict (source, source_account, external_batch_id) do update set
      schema_version = excluded.schema_version,
      exported_at = excluded.exported_at,
      received_at = now(),
      file_sha256 = excluded.file_sha256,
      input_count = excluded.input_count,
      valid_count = excluded.valid_count,
      new_profile_count = excluded.new_profile_count,
      replay_count = excluded.replay_count,
      changed_revision_count = excluded.changed_revision_count,
      error_count = excluded.error_count,
      load_status = 'loaded',
      updated_at = now()
    returning id
  `) as { id: string }[]
  return rows[0]?.id ?? ''
}


