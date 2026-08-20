import { sql } from './client'

// Read-only unified activity feed (CRM-09D). Surfaces canonical interaction
// records ordered across all channels by occurred_at. Person, property, and
// deal context are joined only through existing canonical FKs (person_id,
// property_id, deal_id). No event synthesis, no AI summaries, no inferred
// links.

export type ActivityFeedEntry = {
  id: string
  personId: string | null
  dealId: string | null
  channel: string
  direction: string | null
  occurredAt: string
  occurredAtLabel: string
  title: string | null
  summary: string | null
  personName: string | null
  propertyName: string | null
  dealPropertyName: string | null
}

type ActivityFeedRow = {
  id: string
  person_id: string | null
  deal_id: string | null
  channel: string
  direction: string | null
  occurred_at: string
  occurred_at_label: string
  title: string | null
  summary: string | null
  person_name: string | null
  property_name: string | null
  deal_property_name: string | null
}

export async function getActivityFeed(
  limit = 50,
): Promise<ActivityFeedEntry[]> {
  const rows = await sql`
    select
      i.id,
      person.id as person_id,
      deal.id as deal_id,
      i.channel,
      i.direction,
      i.occurred_at,
      to_char(
        i.occurred_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
      ) as occurred_at_label,
      i.title,
      i.summary,
      person.display_name as person_name,
      property.name as property_name,
      deal_property.name as deal_property_name
    from interaction i
    join person
      on person.id = i.person_id
    left join property
      on property.id = i.property_id
    left join deal
      on deal.id = i.deal_id
    left join property deal_property
      on deal_property.id = deal.property_id
    order by i.occurred_at desc
    limit ${limit}
  `

  return (rows as ActivityFeedRow[]).map((row) => ({
    id: row.id,
    personId: row.person_id ?? null,
    dealId: row.deal_id ?? null,
    channel: row.channel,
    direction: row.direction ?? null,
    occurredAt: row.occurred_at,
    occurredAtLabel: row.occurred_at_label,
    title: row.title ?? null,
    summary: row.summary ?? null,
    personName: row.person_name ?? null,
    propertyName: row.property_name ?? null,
    dealPropertyName: row.deal_property_name ?? null,
  }))
}
