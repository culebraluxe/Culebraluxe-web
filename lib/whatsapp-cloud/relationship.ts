import { sql } from '../../db/client'
import {
  recordReconcileDecision,
  upsertRelationshipEvidence,
} from '../../db/relationship-evidence'
import { fingerprint } from '../relationship-intel/normalize'
import { REL_INTEL_RULE_VERSION } from '../relationship-intel/reconcile'
import type { RelationshipEvidence } from '../relationship-intel/contracts'
import type { ExternalActivityEvent } from '../mac-observer/contracts'

const WHATSAPP_SOURCE = 'whatsapp' as const

type AggregateRow = {
  first_observed_at: string | Date | null
  last_observed_at: string | Date | null
  last_inbound_at: string | Date | null
  last_outbound_at: string | Date | null
  inbound_count: number
  outbound_count: number
}

function toIso(value: string | Date | null): string | null {
  if (!value) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

function externalPhone(event: ExternalActivityEvent): string {
  const candidate = event.contactCandidates?.find((item) => item.kind === 'phone')
  if (!candidate?.value) {
    throw new Error(`WhatsApp event ${event.externalEventId} has no external phone candidate.`)
  }
  return candidate.value
}

/**
 * Reduce the durable realtime WhatsApp inbox into the same relationship-evidence
 * seam that feeds the Client Person×Source read model.
 *
 * The aggregate is rebuilt from durable completed inbox receipts rather than
 * incremented from the webhook call. That makes Meta retries and a crash after
 * canonical interaction persistence self-healing: replaying any completed event
 * recomputes the same absolute Person×WhatsApp state without double-counting.
 */
export async function projectWhatsAppRelationship(input: {
  event: ExternalActivityEvent
  personId: string
}): Promise<void> {
  const { event, personId } = input
  const phone = externalPhone(event)

  // The provider identity has durable ownership independently of relationship
  // evidence. Never redirect a phone already owned by another canonical Person.
  await sql`
    insert into integration_source_person_link (
      source, source_account, source_identity_key,
      canonical_person_id, link_method, link_reason
    ) values (
      ${WHATSAPP_SOURCE}, ${event.sourceAccount}, ${phone},
      ${personId}, 'exact_phone', 'resolved_whatsapp_event'
    )
    on conflict (source, source_account, source_identity_key) do nothing
  `

  const ownership = (await sql`
    select canonical_person_id
    from integration_source_person_link
    where source = ${WHATSAPP_SOURCE}
      and source_account = ${event.sourceAccount}
      and source_identity_key = ${phone}
    limit 1
  `) as { canonical_person_id: string }[]
  const owner = ownership[0]?.canonical_person_id
  if (!owner) {
    throw new Error(`WhatsApp source-person link was not persisted for ${phone}.`)
  }
  if (owner !== personId) {
    throw new Error(
      `WhatsApp source-person ownership conflict for ${phone}; existing=${owner} attempted=${personId}`,
    )
  }

  const aggregate = (await sql`
    select
      min(occurred_at) as first_observed_at,
      max(occurred_at) as last_observed_at,
      max(occurred_at) filter (where direction = 'inbound') as last_inbound_at,
      max(occurred_at) filter (where direction = 'outbound') as last_outbound_at,
      count(*) filter (where direction = 'inbound')::int as inbound_count,
      count(*) filter (where direction = 'outbound')::int as outbound_count
    from integration_inbox
    where source = ${WHATSAPP_SOURCE}
      and source_account = ${event.sourceAccount}
      and resolved_person_id = ${personId}
      and status = 'completed'
  `) as AggregateRow[]
  const row = aggregate[0]
  if (!row) return

  const firstObservedAt = toIso(row.first_observed_at)
  const lastObservedAt = toIso(row.last_observed_at)
  const lastInboundAt = toIso(row.last_inbound_at)
  const lastOutboundAt = toIso(row.last_outbound_at)
  const inboundCount = Number(row.inbound_count ?? 0)
  const outboundCount = Number(row.outbound_count ?? 0)

  const linkedPhones = (await sql`
    select source_identity_key
    from integration_source_person_link
    where source = ${WHATSAPP_SOURCE}
      and source_account = ${event.sourceAccount}
      and canonical_person_id = ${personId}
    order by source_identity_key
  `) as { source_identity_key: string }[]
  const phones = linkedPhones.map(({ source_identity_key }) => ({
    value: source_identity_key,
    normalized: source_identity_key,
    label: null,
  }))

  // Event sources reduce to one relationship row per Person×provider account.
  // Source identity ownership remains separately durable per external phone.
  const sourceIdentityKey = `person:${personId}`
  const evidence: RelationshipEvidence = {
    source: WHATSAPP_SOURCE,
    sourceAccount: event.sourceAccount,
    sourceIdentityKey,
    sourceLabel: 'Meta WhatsApp',
    displayName: null,
    organization: null,
    emails: [],
    phones,
    firstObservedAt,
    lastObservedAt,
    lastInboundAt,
    lastOutboundAt,
    inboundCount,
    outboundCount,
    isTwoWay: inboundCount > 0 && outboundCount > 0,
    isOwnerInitiated: outboundCount > 0,
    isAutomatedOrBulk: false,
    isOrganizationOrService: false,
    knownAppleContact: null,
    hasEmail: false,
    hasPhone: phones.length > 0,
    coverageNote: 'Meta WhatsApp Cloud API realtime webhook coverage.',
  }

  const evidenceFingerprint = fingerprint(JSON.stringify({
    source: evidence.source,
    sourceAccount: evidence.sourceAccount,
    sourceIdentityKey: evidence.sourceIdentityKey,
    phones: phones.map((item) => item.normalized),
    firstObservedAt,
    lastObservedAt,
    lastInboundAt,
    lastOutboundAt,
    inboundCount,
    outboundCount,
  }))

  const evidenceId = await upsertRelationshipEvidence(evidence, evidenceFingerprint)
  if (!evidenceId) throw new Error('WhatsApp relationship evidence upsert returned no id.')

  await recordReconcileDecision(evidenceId, {
    reviewState: 'exact_linked',
    matchMethod: 'source_link',
    matchConfidence: 'exact',
    canonicalPersonId: personId,
    reason: 'durable_whatsapp_source_person_link',
    ruleVersion: REL_INTEL_RULE_VERSION,
  })
}
