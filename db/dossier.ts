import { sql } from './client'

// Read-only relationship dossier projection (CRM-09C). Composes canonical
// person, identity, property-interest, interaction, task, and deal data that
// already exists. No edits, no actions, no fuzzy identity logic — everything
// is joined through existing canonical relationships (person_id, deal_id,
// property_id).

export type DossierIdentity = {
  type: string
  value: string
  primary: boolean
}

export type DossierInterest = {
  id: string
  propertyId: string
  propertyName: string
  location: string | null
  price: number | null
  status: string
}

export type DossierInteraction = {
  id: string
  channel: string
  direction: string | null
  occurredAtLabel: string
  title: string | null
  summary: string | null
  propertyName: string | null
  dealPropertyName: string | null
}

export type DossierTask = {
  id: string
  title: string
  detail: string | null
  dueAt: string | null
  dueAtLabel: string | null
  isOverdue: boolean
  propertyName: string | null
  dealPropertyName: string | null
}

export type DossierDeal = {
  id: string
  stage: string
  propertyName: string
  propertyLocation: string | null
  listPrice: number | null
  offerPrice: number | null
  ownerName: string | null
  closingDateLabel: string | null
}

export type DossierShowing = {
  id: string
  propertyId: string | null
  propertyName: string | null
  dealId: string | null
  dealPropertyName: string | null
  status: string
  requestedAtLabel: string
  scheduledAtLabel: string | null
  completedAtLabel: string | null
  feedback: string | null
}

export type DossierOffer = {
  id: string
  dealId: string
  dealPropertyName: string
  amount: number
  status: string
  submittedAtLabel: string
  respondedAtLabel: string | null
  isCounter: boolean
}

export type RelationshipDossier = {
  person: {
    id: string
    displayName: string
    role: string
    status: string
    location: string | null
    budgetMin: number | null
    budgetMax: number | null
    timeline: string | null
    notes: string | null
    assignedAgent: string | null
  } | null
  identities: DossierIdentity[]
  interests: DossierInterest[]
  interactions: DossierInteraction[]
  openTasks: DossierTask[]
  deals: DossierDeal[]
  showings: DossierShowing[]
  offers: DossierOffer[]
}

type PersonRow = {
  id: string
  display_name: string
  role: string
  status: string
  location: string | null
  budget_min: string | null
  budget_max: string | null
  timeline: string | null
  notes: string | null
  assigned_user_name: string | null
}

type IdentityRow = {
  identity_type: string
  identity_value: string
  is_primary: boolean
}

type InterestRow = {
  id: string
  property_id: string
  property_name: string
  property_location: string | null
  list_price: string | null
  status: string
}

type InteractionRow = {
  id: string
  channel: string
  direction: string | null
  occurred_at_label: string
  title: string | null
  summary: string | null
  property_name: string | null
  deal_property_name: string | null
}

type TaskRow = {
  id: string
  title: string
  detail: string | null
  due_at: string | null
  due_at_label: string | null
  property_name: string | null
  deal_property_name: string | null
}

type DealRow = {
  id: string
  stage: string
  property_name: string
  property_location: string | null
  list_price: string | null
  offer_price: string | null
  owner_name: string | null
  closing_date_label: string | null
}

type ShowingRow = {
  id: string
  property_id: string | null
  property_name: string | null
  deal_id: string | null
  deal_property_name: string | null
  status: string
  requested_at_label: string
  scheduled_at_label: string | null
  completed_at_label: string | null
  feedback: string | null
}

type OfferRow = {
  id: string
  deal_id: string
  deal_property_name: string
  parent_offer_id: string | null
  amount: string
  status: string
  submitted_at_label: string
  responded_at_label: string | null
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getRelationshipDossier(
  personId: string,
): Promise<RelationshipDossier> {
  const [
    personRows,
    identityRows,
    interestRows,
    interactionRows,
    taskRows,
    dealRows,
    showingRows,
    offerRows,
  ] = await Promise.all([
      sql`
        select
          p.id,
          p.display_name,
          p.role,
          p.status,
          p.location,
          p.budget_min,
          p.budget_max,
          p.timeline,
          p.notes,
          u.display_name as assigned_user_name
        from person p
        left join app_user u
          on u.id = p.assigned_user_id
        where p.id = ${personId}
          and p.archived_at is null
        limit 1
      `,
      sql`
        select
          identity_type,
          identity_value,
          is_primary
        from person_identity
        where person_id = ${personId}
        order by is_primary desc, created_at asc
      `,
      sql`
        select
          pi.id,
          pi.property_id,
          pi.status,
          property.name as property_name,
          property.location as property_location,
          property.list_price::text as list_price
        from property_interest pi
        join property
          on property.id = pi.property_id
        where pi.person_id = ${personId}
          and property.archived_at is null
        order by pi.ranking asc nulls last, pi.created_at desc
      `,
      sql`
        select
          i.id,
          i.channel,
          i.direction,
          to_char(
            i.occurred_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as occurred_at_label,
          i.title,
          i.summary,
          property.name as property_name,
          deal_property.name as deal_property_name
        from interaction i
        left join property
          on property.id = i.property_id
        left join deal
          on deal.id = i.deal_id
        left join property deal_property
          on deal_property.id = deal.property_id
        where i.person_id = ${personId}
        order by i.occurred_at desc
        limit 25
      `,
      sql`
        select
          t.id,
          t.title,
          t.detail,
          t.due_at,
          to_char(
            t.due_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as due_at_label,
          property.name as property_name,
          deal_property.name as deal_property_name
        from task t
        left join property
          on property.id = t.property_id
        left join deal
          on deal.id = t.deal_id
        left join property deal_property
          on deal_property.id = deal.property_id
        where t.person_id = ${personId}
          and t.status = 'open'
        order by t.due_at asc nulls last, t.created_at asc
      `,
      sql`
        select
          d.id,
          d.stage,
          d.list_price,
          d.offer_price,
          to_char(
            d.closing_date at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY'
          ) as closing_date_label,
          property.name as property_name,
          property.location as property_location,
          owner.display_name as owner_name
        from deal d
        join property
          on property.id = d.property_id
        left join lateral (
          select
            app_user.display_name
          from deal_participant dp
          join app_user
            on app_user.id = dp.user_id
          where dp.deal_id = d.id
            and dp.role = 'owner'
            and dp.active = true
          order by dp.created_at asc
          limit 1
        ) owner on true
        where exists (
          select 1
          from deal_participant dp_client
          where dp_client.deal_id = d.id
            and dp_client.person_id = ${personId}
            and dp_client.role = 'client'
        )
        order by
          case d.stage
            when 'under_contract' then 1
            when 'offer' then 2
            when 'showing' then 3
            when 'qualified' then 4
            when 'new_lead' then 5
            when 'closed' then 6
            else 7
          end,
          d.updated_at desc
      `,
      sql`
        select
          s.id,
          s.property_id,
          property.name as property_name,
          s.deal_id,
          deal_property.name as deal_property_name,
          s.status,
          to_char(
            s.requested_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as requested_at_label,
          to_char(
            s.scheduled_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as scheduled_at_label,
          to_char(
            s.completed_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as completed_at_label,
          s.feedback
        from showing s
        left join property
          on property.id = s.property_id
        left join deal d
          on d.id = s.deal_id
        left join property deal_property
          on deal_property.id = d.property_id
        where s.person_id = ${personId}
        order by s.requested_at desc
        limit 20
      `,
      sql`
        select
          o.id,
          o.deal_id,
          deal_property.name as deal_property_name,
          o.parent_offer_id,
          o.amount,
          o.status,
          to_char(
            o.submitted_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as submitted_at_label,
          to_char(
            o.responded_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as responded_at_label
        from offer o
        join deal d
          on d.id = o.deal_id
        join property deal_property
          on deal_property.id = d.property_id
        where o.person_id = ${personId}
        order by o.submitted_at desc
        limit 20
      `,
    ])

  const personRow = (personRows as PersonRow[])[0]
  const now = Date.now()

  return {
    person: personRow
      ? {
          id: personRow.id,
          displayName: personRow.display_name,
          role: personRow.role,
          status: personRow.status,
          location: personRow.location ?? null,
          budgetMin: toNumber(personRow.budget_min),
          budgetMax: toNumber(personRow.budget_max),
          timeline: personRow.timeline ?? null,
          notes: personRow.notes ?? null,
          assignedAgent: personRow.assigned_user_name ?? null,
        }
      : null,
    identities: (identityRows as IdentityRow[]).map((row) => ({
      type: row.identity_type,
      value: row.identity_value,
      primary: row.is_primary,
    })),
    interests: (interestRows as InterestRow[]).map((row) => ({
      id: row.id,
      propertyId: row.property_id,
      propertyName: row.property_name,
      location: row.property_location ?? null,
      price: toNumber(row.list_price),
      status: row.status,
    })),
    interactions: (interactionRows as InteractionRow[]).map((row) => ({
      id: row.id,
      channel: row.channel,
      direction: row.direction ?? null,
      occurredAtLabel: row.occurred_at_label,
      title: row.title ?? null,
      summary: row.summary ?? null,
      propertyName: row.property_name ?? null,
      dealPropertyName: row.deal_property_name ?? null,
    })),
    openTasks: (taskRows as TaskRow[]).map((row) => ({
      id: row.id,
      title: row.title,
      detail: row.detail ?? null,
      dueAt: row.due_at ?? null,
      dueAtLabel: row.due_at_label ?? null,
      isOverdue:
        row.due_at !== null && new Date(row.due_at).getTime() < now,
      propertyName: row.property_name ?? null,
      dealPropertyName: row.deal_property_name ?? null,
    })),
    deals: (dealRows as DealRow[]).map((row) => ({
      id: row.id,
      stage: row.stage,
      propertyName: row.property_name,
      propertyLocation: row.property_location ?? null,
      listPrice: toNumber(row.list_price),
      offerPrice: toNumber(row.offer_price),
      ownerName: row.owner_name ?? null,
      closingDateLabel: row.closing_date_label ?? null,
    })),
    showings: (showingRows as ShowingRow[]).map((row) => ({
      id: row.id,
      propertyId: row.property_id ?? null,
      propertyName: row.property_name ?? null,
      dealId: row.deal_id ?? null,
      dealPropertyName: row.deal_property_name ?? null,
      status: row.status,
      requestedAtLabel: row.requested_at_label,
      scheduledAtLabel: row.scheduled_at_label ?? null,
      completedAtLabel: row.completed_at_label ?? null,
      feedback: row.feedback ?? null,
    })),
    offers: (offerRows as OfferRow[]).map((row) => ({
      id: row.id,
      dealId: row.deal_id,
      dealPropertyName: row.deal_property_name,
      amount: Number(row.amount),
      status: row.status,
      submittedAtLabel: row.submitted_at_label,
      respondedAtLabel: row.responded_at_label ?? null,
      isCounter: Boolean(row.parent_offer_id),
    })),
  }
}
