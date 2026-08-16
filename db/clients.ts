import { sql } from "./client"
import type {
  Client,
  Interaction,
  InteractionChannel,
  InteractionDirection,
  PropertyInterest,
  PropertyInterestStatus,
} from "@/lib/portal/types"

type PropertyInterestRow = {
  id: string
  property_id: string
  property_name: string
  location: string | null
  price: string | null
  bedrooms: string | null
  property_type: string | null
  status: PropertyInterestStatus
}

type InteractionRow = {
  id: string
  channel: InteractionChannel
  direction: InteractionDirection | null
  occurred_at: string
  title: string | null
  summary: string | null
  duration_seconds: number | null
}

type ClientRow = {
  id: string
  display_name: string
  role: Client["role"]
  status: Client["status"]

  location: string | null

  budget_min: string | null
  budget_max: string | null

  preferred_areas: string[] | null
  property_types: string[] | null
  priorities: string[] | null

  timeline: string | null
  notes: string | null

  assigned_user_name: string | null

  email: string | null
  phone: string | null

  property_interests: PropertyInterestRow[] | null
  interactions: InteractionRow[] | null

  last_contact_channel: InteractionChannel | null
  last_contact_at: string | null
  last_contact_summary: string | null

  next_action_title: string | null
  next_action_at: string | null
  next_action_detail: string | null
}

function toNumber(value: string | null) {
  return value === null ? undefined : Number(value)
}

function formatBedrooms(value: string | null) {
  if (value === null) return undefined

  const bedrooms = Number(value)

  if (Number.isInteger(bedrooms)) {
    return bedrooms
  }

  return bedrooms
}

function propertyDescriptor(
  bedrooms: string | null,
  propertyType: string | null
) {
  const parts: string[] = []

  if (bedrooms !== null) {
    const value = Number(bedrooms)

    parts.push(
      `${Number.isInteger(value) ? value : value} bedrooms`
    )
  }

  if (propertyType) {
    parts.push(propertyType)
  }

  return parts.length > 0 ? parts.join(" · ") : undefined
}

export async function getClients(): Promise<Client[]> {
  const rows = await sql`
    select
      p.id,
      p.display_name,
      p.role,
      p.status,
      p.location,

      p.budget_min,
      p.budget_max,

      p.preferred_areas,
      p.property_types,
      p.priorities,

      p.timeline,
      p.notes,

      u.display_name as assigned_user_name,

      (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = p.id
          and pi.identity_type = 'email'
        order by
          pi.is_primary desc,
          pi.created_at asc
        limit 1
      ) as email,

      (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = p.id
          and pi.identity_type = 'phone'
        order by
          pi.is_primary desc,
          pi.created_at asc
        limit 1
      ) as phone,

      coalesce(
        (
          select jsonb_agg(
            jsonb_build_object(
              'id', pi.id,
              'property_id', property.id,
              'property_name', property.name,
              'location', property.location,
              'price', property.list_price::text,
              'bedrooms', property.bedrooms::text,
              'property_type', property.property_type,
              'status', pi.status
            )
            order by
              pi.ranking asc nulls last,
              pi.created_at desc
          )
          from property_interest pi

          join property
            on property.id = pi.property_id

          where pi.person_id = p.id
            and property.archived_at is null
        ),
        '[]'::jsonb
      ) as property_interests,

      coalesce(
        (
          select jsonb_agg(
            jsonb_build_object(
              'id', i.id,
              'channel', i.channel,
              'direction', i.direction,
              'occurred_at',
                to_char(
                  i.occurred_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ),
              'title', i.title,
              'summary', i.summary,
              'duration_seconds', i.duration_seconds
            )
            order by i.occurred_at desc
          )
          from interaction i
          where i.person_id = p.id
        ),
        '[]'::jsonb
      ) as interactions,

      last_contact.channel as last_contact_channel,

      case
        when last_contact.occurred_at is not null
        then to_char(
          last_contact.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        )
        else null
      end as last_contact_at,

      coalesce(
        last_contact.summary,
        last_contact.title
      ) as last_contact_summary,

      next_action.title as next_action_title,

      case
        when next_action.due_at is not null
        then to_char(
          next_action.due_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        )
        else null
      end as next_action_at,

      next_action.detail as next_action_detail

    from person p

    left join app_user u
      on u.id = p.assigned_user_id

    left join lateral (
      select
        i.channel,
        i.occurred_at,
        i.title,
        i.summary
      from interaction i
      where i.person_id = p.id
      order by i.occurred_at desc
      limit 1
    ) last_contact on true

    left join lateral (
      select
        t.title,
        t.detail,
        t.due_at
      from task t
      where t.person_id = p.id
        and t.status = 'open'
      order by
        t.due_at asc nulls last,
        t.created_at asc
      limit 1
    ) next_action on true

    where p.archived_at is null

    order by p.display_name asc
  `

  return (rows as ClientRow[]).map((row) => {
    const propertyInterests: PropertyInterest[] =
      (row.property_interests ?? []).map((interest) => ({
        id: interest.id,
        propertyId: interest.property_id,
        propertyName: interest.property_name,
        location: interest.location ?? "Culebra, Puerto Rico",
        price: Number(interest.price ?? 0),
        bedrooms: formatBedrooms(interest.bedrooms),
        descriptor: propertyDescriptor(
          interest.bedrooms,
          interest.property_type
        ),
        status: interest.status,
      }))

    const interactions: Interaction[] =
      (row.interactions ?? []).map((interaction) => ({
        id: interaction.id,
        channel: interaction.channel,
        direction: interaction.direction ?? undefined,
        occurredAt: interaction.occurred_at,
        title: interaction.title ?? "Interaction",
        summary: interaction.summary ?? undefined,
        durationSeconds:
          interaction.duration_seconds ?? undefined,
      }))

    return {
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,

      location: row.location ?? undefined,
      email: row.email ?? undefined,
      phone: row.phone ?? undefined,

      budgetMin: toNumber(row.budget_min),
      budgetMax: toNumber(row.budget_max),

      preferredAreas: row.preferred_areas ?? [],
      propertyTypes: row.property_types ?? [],
      priorities: row.priorities ?? [],

      timeline: row.timeline ?? undefined,
      assignedAgent: row.assigned_user_name ?? undefined,
      notes: row.notes ?? undefined,

      propertyInterests,
      interactions,

      lastContact:
        row.last_contact_channel && row.last_contact_at
          ? {
              channel: row.last_contact_channel,
              occurredAt: row.last_contact_at,
              summary:
                row.last_contact_summary ?? undefined,
            }
          : undefined,

      nextAction:
        row.next_action_title
          ? {
              title: row.next_action_title,
              occurredAt: row.next_action_at ?? "Unscheduled",
              detail: row.next_action_detail ?? undefined,
            }
          : undefined,
    }
  })
}