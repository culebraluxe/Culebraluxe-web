import { sql } from "./client"
import type { Deal, DealStage } from "@/lib/portal/types"
import type { ActingUser } from "@/lib/auth/types"

type DealRow = {
  id: string

  property_id: string
  property_name: string
  property_location: string | null
  property_type: string | null
  bedrooms: string | null

  hero_media_id: string | null

  client_id: string
  client_name: string

  stage: DealStage

  list_price: string | null
  offer_price: string | null

  owner_name: string | null

  closing_date: string | null

  showing_count: number | null
  offer_count: number | null
  participant_count: number | null
  latest_offer_amount: string | null
  latest_offer_status: string | null

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

// AUTH-02 read-scoping rule for deal reads:
//   - internal actors (portal.read / deal.read) → coarse read, all deals.
//   - external actors (external.deal.read_own)  → rows scoped to deals linked
//     to the actor's own person (app_user.person_id) via active deal_participant
//     rows. External accounts never hold portal.read; this is the row-scoping
//     applied in deal read services, not a new authority.
//   - external actor without a linked person → no deals (fail closed).
export async function getDeals(
  actor?: Pick<ActingUser, "accountType" | "personId">,
): Promise<Deal[]> {
  const externalScope =
    actor?.accountType === "external"
      ? sql`
        and d.id in (
          select dp.deal_id
          from deal_participant dp
          where dp.person_id = ${actor.personId}
            and dp.active = true
        )
      `
      : sql``

  const rows = await sql`
    select
      d.id,

      p.id as property_id,
      p.name as property_name,
      p.location as property_location,
      p.property_type,
      p.bedrooms,

      hero_media.media_id as hero_media_id,

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
      end as last_activity_at,

      (select count(*) from showing s where s.deal_id = d.id)::int as showing_count,
      (select count(*) from offer o where o.deal_id = d.id)::int as offer_count,
      (select count(*) from deal_participant dp
        where dp.deal_id = d.id and dp.active = true)::int as participant_count,
      latest_offer.amount as latest_offer_amount,
      latest_offer.status as latest_offer_status

    from deal d

    join property p
      on p.id = d.property_id

    join lateral (
      select
        person.id,
        person.display_name
      from deal_participant dp
      join person
        on person.id = dp.person_id
      where dp.deal_id = d.id
        and dp.role = 'client'
        and dp.active = true
      order by dp.created_at asc
      limit 1
    ) person on true

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

    left join lateral (
      select
        pm.media_id
      from property_media pm
      where pm.property_id = p.id
        and pm.role = 'hero'
      order by
        pm.sort_order asc,
        pm.created_at asc
      limit 1
    ) hero_media on true

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

    left join lateral (
      select
        o.amount,
        o.status
      from offer o
      where o.deal_id = d.id
      order by o.submitted_at desc
      limit 1
    ) latest_offer on true

    where 1 = 1
    ${externalScope}

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

    heroMediaId: row.hero_media_id ?? undefined,

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

    showingCount: row.showing_count ?? 0,
    offerCount: row.offer_count ?? 0,
    participantCount: row.participant_count ?? 0,
    latestOfferAmount: toNumber(row.latest_offer_amount),
    latestOfferStatus: row.latest_offer_status ?? undefined,
  }))
}
// ---------------------------------------------------------------------------
// OPS-05 — light property picker for deal creation. Active (non-archived)
// properties only: an archived property is off the public site (OPS-03 soft
// archive) and cannot start a deal.
// ---------------------------------------------------------------------------

export type DealableProperty = {
  id: string
  name: string
  location: string | null
}

export async function listDealableProperties(): Promise<DealableProperty[]> {
  const rows = await sql`
    select id, name, location
    from property
    where archived_at is null
    order by name asc
  `
  return (rows as { id: string; name: string; location: string | null }[]).map(
    (row) => ({
      id: row.id,
      name: row.name,
      location: row.location ?? null,
    }),
  )
}
