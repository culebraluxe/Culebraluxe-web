import { sql } from '../db/client'
import { financingApplicableFromType } from './financing'

// Canonical DealWorkflowFacts projection for transaction-close-v1.
//
// Only the facts workflow decisions require. Canonical data only — no mock
// data, no full row dumps. Financing applicability derives from the canonical
// deal.financing_type fact, never from lender participant presence.

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
  /** true = financed, false = cash, null = unknown. */
  financingApplicable: boolean | null
}

export { financingApplicableFromType }

type DealRow = {
  id: string
  stage: string
  list_price: string | null
  offer_price: string | null
  closing_date: string | null
  financing_type: string | null
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
      p.id as property_id,
      p.name as property_name,
      p.property_type,
      p.status as property_status,
      person.id as client_id,
      person.display_name as client_name
    from deal d
    join property p on p.id = d.property_id
    left join person on person.id = d.client_person_id
    where d.id = ${dealId}
    limit 1
  `
  const deal = dealRows[0] as DealRow | undefined
  if (!deal) return null

  const [offerRows, showingRows, taskRows, participantRows] = await Promise.all([
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
  ])

  return {
    dealId: deal.id,
    stage: deal.stage,
    listPrice: deal.list_price === null ? null : Number(deal.list_price),
    offerPrice: deal.offer_price === null ? null : Number(deal.offer_price),
    closingDate: deal.closing_date,
    financingApplicable: financingApplicableFromType(deal.financing_type),
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
