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

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getRelationshipDossier(
  personId: string,
): Promise<RelationshipDossier> {
  const [personRows, identityRows, interestRows, interactionRows, taskRows, dealRows] =
    await Promise.all([
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
        left join app_user owner
          on owner.id = d.owner_user_id
        where d.client_person_id = ${personId}
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
  }
}
