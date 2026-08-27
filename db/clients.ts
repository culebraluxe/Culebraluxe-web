import { sql, raw } from "./client"
import type { QueryExecutor } from "./query-executor"
import type {
  Client,
  Interaction,
  InteractionChannel,
  InteractionDirection,
  PropertyInterest,
  PropertyInterestStatus,
} from "@/lib/portal/types"
import type { JsonObject } from '@/lib/crm-types'
import { getRelationshipEvidenceForPersons } from './relationship-evidence'
import { summarizeRelationshipEvidence } from '@/lib/relationship-intel/relationship-context'
import type { RelationshipActivity } from '@/lib/portal/types'

type PropertyInterestRow = {
  id: string
  property_id: string
  property_name: string
  location: string | null
  price: string | null
  bedrooms: string | null
  property_type: string | null
  status: PropertyInterestStatus
  hero_media_id: string | null
}

type InteractionRow = {
  id: string
  channel: InteractionChannel
  event_type: string
  direction: InteractionDirection | null
  occurred_at: string
  title: string | null
  summary: string | null
  duration_seconds: number | null
  source_metadata: JsonObject | null
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
  assigned_user_id: string | null

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

export async function getClients(opts: { id?: string } = {}): Promise<Client[]> {
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
      u.id as assigned_user_id,

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
              'status', pi.status,
              'hero_media_id',
                (
                  select pm.media_id
                  from property_media pm
                  where pm.property_id = property.id
                    and pm.role = 'hero'
                  order by
                    pm.sort_order asc,
                    pm.created_at asc
                  limit 1
                )
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
              'event_type', i.event_type,
              'direction', i.direction,
              'occurred_at',
                to_char(
                  i.occurred_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ),
              'title', i.title,
              'summary', i.summary,
              'duration_seconds', i.duration_seconds,
              'source_metadata', i.source_metadata
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
      and (${opts.id ?? null}::text is null or p.id = ${opts.id ?? null})

    order by p.display_name asc
    limit ${opts.id ? 1 : null}
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
        heroMediaId: interest.hero_media_id ?? undefined,
      }))

    const interactions: Interaction[] =
      (row.interactions ?? []).map((interaction) => ({
        id: interaction.id,
        channel: interaction.channel,
        eventType: interaction.event_type,
        direction: interaction.direction ?? undefined,
        occurredAt: interaction.occurred_at,
        title: interaction.title ?? "Interaction",
        summary: interaction.summary ?? undefined,
        durationSeconds:
          interaction.duration_seconds ?? undefined,
        sourceMetadata: interaction.source_metadata ?? {},
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
      assignedUserId: row.assigned_user_id ?? undefined,
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

/** The full canonical Client for a single person (for the working-pane detail). */
export async function getClientById(id: string): Promise<Client | null> {
  const rows = await getClients({ id })
  const client = rows[0]
  if (!client) return null

  const evidenceByPerson = await getRelationshipEvidenceForPersons([id])
  return {
    ...client,
    relationshipActivity: toRelationshipActivity(evidenceByPerson[id] ?? []),
  }
}

function toRelationshipActivity(
  evidence: Parameters<typeof summarizeRelationshipEvidence>[0],
): RelationshipActivity {
  const summary = summarizeRelationshipEvidence(evidence)
  return {
    sources: summary.sources,
    firstObservedAt: summary.firstObservedAt,
    inboundCount: summary.inboundCount,
    outboundCount: summary.outboundCount,
    observedCommunicationCount: summary.observedCommunicationCount,
    twoWay: summary.twoWay,
    lastObservedAt: summary.lastObservedAt,
    lastMeaningfulContactAt: summary.lastMeaningfulContactAt,
    lastInboundAt: summary.lastInboundAt,
    lastOutboundAt: summary.lastOutboundAt,
    coverageLimited: summary.coverageLimited,
    channels: summary.channels,
  }
}

export type ClientSummary = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  location: string | null
  primaryEmail: string | null
  primaryPhone: string | null
  assignedAgent: string | null
  lastContactLabel: string | null
  sources: string[]
  relationshipActivity: RelationshipActivity
}

export type ClientsPageResult = {
  rows: ClientSummary[]
  total: number
  page: number
  pageSize: number
}

type ClientSummaryRaw = {
  person_id: string
  display_name: string
  name_sort_priority: number
  role: string
  status: string
  location: string | null
  primary_email: string | null
  primary_phone: string | null
  assigned_agent: string | null
  last_contact_label: string | null
  sources: unknown
}

const VALID_SORTS = ["name", "created", "recent"] as const

// Sorts against the materialized read model (mv_client_directory) where the
// resolved-first priority, name, created_at, and last_contact_at are already
// pre-shaped columns — no request-time reconstruction.
const ORDER_FRAGMENTS: Record<string, ReturnType<typeof raw>> = {
  name: raw`mv.name_sort_priority desc, mv.display_name asc, mv.person_id asc`,
  created: raw`mv.created_at desc, mv.display_name asc, mv.person_id asc`,
  recent: raw`coalesce(mv.last_contact_at, mv.created_at) desc nulls last, mv.display_name asc, mv.person_id asc`,
}

/**
 * Server-side pagination over the materialized application read model
 * mv_client_directory. One row per canonical Person.
 *   - separate COUNT(*) + SQL LIMIT/OFFSET (50/page default)
 *   - search / filters / sort applied in SQL against the pre-shaped view
 *   - one bounded evidence query enriches only the returned canonical people
 *   - no correlated subqueries / lateral joins / repeated identity lookups /
 *     provenance aggregation / last-contact assembly at request time
 */
export async function getClientsPage(
  opts: {
    search?: string
    status?: string
    role?: string
    sort?: string
    page?: number
    pageSize?: number
  },
  execute: QueryExecutor = sql,
): Promise<ClientsPageResult> {
  const search = (opts.search ?? "").trim()
  const like = search === "" ? null : `%${search}%`
  const status = opts.status ?? null
  const role = opts.role ?? null
  const sort = VALID_SORTS.includes(opts.sort as (typeof VALID_SORTS)[number])
    ? (opts.sort as string)
    : "name"
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, Math.min(50, opts.pageSize ?? 50))
  const offset = (page - 1) * pageSize

  const guard = raw`
    where (${like}::text is null or mv.search_text ilike ${like})
      and (${status}::text is null or mv.status = ${status})
      and (${role}::text is null or mv.role = ${role})
  `

  const countRows = (await execute`
    select count(*)::int as total from mv_client_directory mv ${guard}
  `) as { total: number }[]
  const total = Number(countRows[0]?.total ?? 0)

  const rows = (await execute`
    select
      mv.person_id,
      mv.display_name,
      mv.role,
      mv.status,
      mv.location,
      mv.primary_email,
      mv.primary_phone,
      mv.assigned_agent,
      mv.last_contact_label,
      mv.sources,
      mv.name_sort_priority
    from mv_client_directory mv
    ${guard}
    order by ${ORDER_FRAGMENTS[sort]}
    limit ${pageSize} offset ${offset}
  `) as ClientSummaryRaw[]

  const personIds = rows.map((row) => row.person_id)
  const evidenceByPerson = await getRelationshipEvidenceForPersons(personIds, execute)

  return {
    rows: rows.map((row) => ({
      id: row.person_id,
      displayName: row.display_name,
      nameResolved: row.name_sort_priority === 1,
      role: row.role,
      status: row.status,
      location: row.location ?? null,
      primaryEmail: row.primary_email ?? null,
      primaryPhone: row.primary_phone ?? null,
      assignedAgent: row.assigned_agent ?? null,
      lastContactLabel: row.last_contact_label ?? null,
      sources: Array.isArray(row.sources) ? (row.sources as string[]) : [],
      relationshipActivity: toRelationshipActivity(evidenceByPerson[row.person_id] ?? []),
    })),
    total,
    page,
    pageSize,
  }
}
