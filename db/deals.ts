import { sql } from "./client"
import type { Deal, DealStage } from "@/lib/portal/types"

type DealRow = {
  id: string

  property_id: string
  property_name: string
  property_location: string | null
  property_type: string | null
  bedrooms: string | null

  client_id: string
  client_name: string

  stage: DealStage

  list_price: string | null
  offer_price: string | null

  owner_name: string | null

  closing_date: string | null

  next_milestone: string | null
  next_milestone_at: string | null

  last_activity: string | null
  last_activity_at: string | null
}

function toNumber(value: string | null) {
  return value === null ? undefined : Number(value)
}

function propertyDescriptor(row: DealRow) {
  const parts: string[] = []

  if (row.bedrooms !== null) {
    const bedrooms = Number(row.bedrooms)

    parts.push(
      `${Number.isInteger(bedrooms) ? bedrooms : bedrooms} bedrooms`
    )
  }

  if (row.property_type) {
    parts.push(row.property_type)
  }

  return parts.length > 0 ? parts.join(" · ") : undefined
}

export async function getDeals(): Promise<Deal[]> {
  const rows = await sql`
    select
      d.id,

      p.id as property_id,
      p.name as property_name,
      p.location as property_location,
      p.property_type,
      p.bedrooms,

      person.id as client_id,
      person.display_name as client_name,

      d.stage,

      d.list_price,
      d.offer_price,

      owner.display_name as owner_name,

      case
        when d.closing_date is not null
        then to_char(d.closing_date, 'Mon FMDD, YYYY')
        else null
      end as closing_date,

      next_task.title as next_milestone,

      case
        when next_task.due_at is not null
        then to_char(
          next_task.due_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        )
        else null
      end as next_milestone_at,

      last_interaction.title as last_activity,

      case
        when last_interaction.occurred_at is not null
        then to_char(
          last_interaction.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        )
        else null
      end as last_activity_at

    from deal d

    join property p
      on p.id = d.property_id

    join person
      on person.id = d.client_person_id

    left join app_user owner
      on owner.id = d.owner_user_id

    left join lateral (
      select
        t.title,
        t.due_at
      from task t
      where t.deal_id = d.id
        and t.status = 'open'
      order by
        t.due_at asc nulls last,
        t.created_at asc
      limit 1
    ) next_task on true

    left join lateral (
      select
        i.title,
        i.summary,
        i.occurred_at
      from interaction i
      where i.deal_id = d.id
      order by i.occurred_at desc
      limit 1
    ) last_interaction on true

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
  `

  return (rows as DealRow[]).map((row) => ({
    id: row.id,

    propertyId: row.property_id,
    propertyName: row.property_name,
    propertyLocation: row.property_location ?? "Culebra, Puerto Rico",
    propertyDescriptor: propertyDescriptor(row),

    clientId: row.client_id,
    clientName: row.client_name,

    stage: row.stage,

    listPrice: toNumber(row.list_price),
    offerPrice: toNumber(row.offer_price),

    owner: row.owner_name ?? "Unassigned",

    closingDate: row.closing_date ?? undefined,

    nextMilestone: row.next_milestone ?? undefined,
    nextMilestoneAt: row.next_milestone_at ?? undefined,

    lastActivity: row.last_activity ?? undefined,
    lastActivityAt: row.last_activity_at ?? undefined,
  }))
}