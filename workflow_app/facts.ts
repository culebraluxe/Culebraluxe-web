import { sql } from '../db/client'
import { financingApplicableFromType } from './financing'
import { appraisalApplicableFromRequired } from './appraisal'
import { lenderClearToCloseFromFact } from './lender-clearance'
import { listTransactionDocumentsByDeal } from '../db/transaction-document'
import { deriveClosingDocumentReadiness, type PacketFacts } from './transaction-packet'
import { CULEBRA_JURISDICTION_CONFIG } from './configuration'

// ---------------------------------------------------------------------------
// Canonical DealWorkflowFacts projection for the RE_supermodel.
//
// Only the facts workflow decisions require. Canonical data only — no mock
// data, no full row dumps.
//
// Story 134 classification:
//   A — derived from canonical data:
//       financingApplicable (deal.financing_type), closingDate /
//       closingDateScheduled (deal.closing_date), appraisalApplicable
//       (deal.appraisal_required), lenderClearToClose
//       (deal.lender_clear_to_close), closingDocumentsReady (CRM-21 — derived
//       from the packet catalog + canonical transaction_document rows; never
//       stored, never invented)
//   B — CulebraLuxe configuration default (configuration.ts):
//       closingAgentRole, requiresNotario, requiresTitleCompany,
//       requiresCrimClearance, requiresRegistryFollowup,
//       inspectionApplicable, insuranceApplicable, requiresSurvey,
//       requiresHoaClearance, closingConfirmationRequired
//   C — unresolved (never invented; requires human/application resolution):
//       none for V1 (appraisal moved from Class C to Class A in CRM-19: the
//       durable deal-level source deal.appraisal_required is resolved by the
//       explicit application command deal.set_appraisal_required; lender
//       clear-to-close moved from Class C to Class A in CRM-20: the durable
//       deal-level source deal.lender_clear_to_close is resolved by the
//       explicit application command deal.set_lender_clear_to_close)
//   D — not yet necessary for V1: none
// ---------------------------------------------------------------------------

export type DealWorkflowFacts = {
  dealId: string
  stage: string
  listPrice: number | null
  offerPrice: number | null
  closingDate: string | null
  property: {
    id: string
    name: string
    propertyType: string | null
    status: string
  } | null
  client: { id: string; name: string } | null
  offers: Array<{
    id: string
    amount: number
    status: string
    parentOfferId: string | null
  }>
  showings: Array<{ id: string; status: string }>
  openTasks: Array<{ id: string; title: string; dueAt: string | null }>
  participants: Array<{
    id: string
    roleCategory: string
    roleLabel: string | null
    personId: string | null
    userId: string | null
  }>

  // Class A (derived)
  /** true = financed, false = cash, null = unknown. */
  financingApplicable: boolean | null
  /** true when a canonical target closing date exists. */
  closingDateScheduled: boolean
  /**
   * Appraisal branch active. Canonical deal.appraisal_required fact
   * (CRM-19): true = appraisal required, false = not required,
   * null = unresolved (never coerced to a boolean).
   */
  appraisalApplicable: boolean | null
  /**
   * Lender clear-to-close (CRM-20). Canonical deal.lender_clear_to_close
   * fact: true = lender cleared the transaction to close, false = not
   * cleared, null = unresolved. Only consumed when financingApplicable is
   * true (financed deals); cash/non-financed deals are unaffected.
   */
  lenderClearToClose: boolean | null
  /**
   * Closing-document readiness (CRM-21). DERIVED, never stored: true only
   * when the packet's required closing package (closing documents + closing
   * statement) is complete AND every required item is in a final (signed)
   * state of the DOC-01 draft -> ready -> sent -> signed lineage. Sourced
   * from the packet rule catalog + canonical transaction_document rows; the
   * fact never invents a required document.
   */
  closingDocumentsReady: boolean

  // Class B (CulebraLuxe configuration defaults)
  closingAgentRole: string
  requiresNotario: boolean
  requiresTitleCompany: boolean
  requiresCrimClearance: boolean
  requiresRegistryFollowup: boolean
  inspectionApplicable: boolean
  insuranceApplicable: boolean
  requiresSurvey: boolean
  requiresHoaClearance: boolean
  closingConfirmationRequired: boolean
}

export { financingApplicableFromType }

type DealRow = {
  id: string
  stage: string
  list_price: string | null
  offer_price: string | null
  closing_date: string | null
  financing_type: string | null
  appraisal_required: boolean | null
  lender_clear_to_close: boolean | null
  property_id: string
  property_name: string
  property_type: string | null
  property_status: string
  client_id: string | null
  client_name: string | null
}

export async function getDealWorkflowFacts(
  dealId: string,
): Promise<DealWorkflowFacts | null> {
  const dealRows = await sql`
    select
      d.id,
      d.stage,
      d.list_price::text as list_price,
      d.offer_price::text as offer_price,
      d.closing_date::text as closing_date,
      d.financing_type,
      d.appraisal_required,
      d.lender_clear_to_close,
      p.id as property_id,
      p.name as property_name,
      p.property_type,
      p.status as property_status,
      client_participant.id as client_id,
      client_participant.display_name as client_name
    from deal d
    join property p on p.id = d.property_id
    left join lateral (
      select person.id, person.display_name
      from deal_participant dp
      join person on person.id = dp.person_id
      where dp.deal_id = d.id
        and dp.role = 'client'
        and dp.active = true
      order by dp.created_at asc
      limit 1
    ) client_participant on true
    where d.id = ${dealId}
    limit 1
  `
  const deal = dealRows[0] as DealRow | undefined
  if (!deal) return null

  const [offerRows, showingRows, taskRows, participantRows, documentRows] =
    await Promise.all([
      sql`
        select id, amount::text as amount, status, parent_offer_id
        from offer where deal_id = ${dealId}
        order by submitted_at asc
      `,
      sql`
        select id, status from showing where deal_id = ${dealId}
        order by requested_at asc
      `,
      sql`
        select id, title, due_at::text as due_at
        from task where deal_id = ${dealId} and status = 'open'
        order by due_at asc nulls last, created_at asc
      `,
      sql`
        select id, role, role_label, person_id, user_id
        from deal_participant
        where deal_id = ${dealId} and active = true
        order by started_at asc
      `,
      listTransactionDocumentsByDeal(dealId),
    ])

  const cfg = CULEBRA_JURISDICTION_CONFIG
  const closingDate = deal.closing_date
  const financingApplicable = financingApplicableFromType(deal.financing_type)
  const appraisalApplicable = appraisalApplicableFromRequired(deal.appraisal_required)

  // CRM-21 — the closing-document readiness fact is DERIVED from the packet
  // rule catalog + the canonical transaction_document rows (never stored,
  // never invented). PacketFacts is a structural subset of this projection.
  const packetFacts: PacketFacts = {
    financingApplicable,
    closingDateScheduled: closingDate !== null,
    appraisalApplicable,
    requiresNotario: cfg.requiresNotario,
    requiresTitleCompany: cfg.requiresTitleCompany,
    requiresCrimClearance: cfg.requiresCrimClearance,
    requiresRegistryFollowup: cfg.requiresRegistryFollowup,
    inspectionApplicable: cfg.inspectionApplicable,
    insuranceApplicable: cfg.insuranceApplicable,
    requiresSurvey: cfg.requiresSurvey,
    requiresHoaClearance: cfg.requiresHoaClearance,
  }
  const closingDocumentReadiness = deriveClosingDocumentReadiness(
    packetFacts,
    documentRows,
  )

  return {
    dealId: deal.id,
    stage: deal.stage,
    listPrice: deal.list_price === null ? null : Number(deal.list_price),
    offerPrice: deal.offer_price === null ? null : Number(deal.offer_price),
    closingDate,
    financingApplicable,
    closingDateScheduled: closingDate !== null,
    // CRM-19 — canonical deal-level source (deal.appraisal_required), resolved
    // by the explicit application command deal.set_appraisal_required. Never
    // invented: null means unresolved, and the XML decision surfaces it
    // explicitly (appraisal_applicability_unresolved) instead of skipping.
    appraisalApplicable,
    // CRM-20 — canonical deal-level source (deal.lender_clear_to_close),
    // resolved by the explicit application command deal.set_lender_clear_to_close.
    // Never invented: null means unresolved, and the XML closing-readiness gate
    // surfaces it explicitly (lender_clearance_resolution / _pending) instead
    // of letting a financed deal appear closing-ready. Cash deals are routed
    // around the fact entirely.
    lenderClearToClose: lenderClearToCloseFromFact(deal.lender_clear_to_close),
    // CRM-21 — derived closing-document readiness (packet catalog + canonical
    // transaction_document rows; signed/final lineage only). Consumed by the
    // XML closing_documents_gate before closing readiness.
    closingDocumentsReady: closingDocumentReadiness.ready,
    // Class B — Culebra operating defaults.
    closingAgentRole: cfg.closingAgentRole,
    requiresNotario: cfg.requiresNotario,
    requiresTitleCompany: cfg.requiresTitleCompany,
    requiresCrimClearance: cfg.requiresCrimClearance,
    requiresRegistryFollowup: cfg.requiresRegistryFollowup,
    inspectionApplicable: cfg.inspectionApplicable,
    insuranceApplicable: cfg.insuranceApplicable,
    requiresSurvey: cfg.requiresSurvey,
    requiresHoaClearance: cfg.requiresHoaClearance,
    closingConfirmationRequired: cfg.closingConfirmationRequired,
    property: {
      id: deal.property_id,
      name: deal.property_name,
      propertyType: deal.property_type,
      status: deal.property_status,
    },
    client:
      deal.client_id && deal.client_name
        ? { id: deal.client_id, name: deal.client_name }
        : null,
    offers: (offerRows as Array<{
      id: string
      amount: string
      status: string
      parent_offer_id: string | null
    }>).map((o) => ({
      id: o.id,
      amount: Number(o.amount),
      status: o.status,
      parentOfferId: o.parent_offer_id,
    })),
    showings: (showingRows as Array<{ id: string; status: string }>).map(
      (s) => ({ id: s.id, status: s.status }),
    ),
    openTasks: (taskRows as Array<{
      id: string
      title: string
      due_at: string | null
    }>).map((t) => ({ id: t.id, title: t.title, dueAt: t.due_at })),
    participants: (participantRows as Array<{
      id: string
      role: string
      role_label: string | null
      person_id: string | null
      user_id: string | null
    }>).map((p) => ({
      id: p.id,
      roleCategory: p.role,
      roleLabel: p.role_label,
      personId: p.person_id,
      userId: p.user_id,
    })),
  }
}
