import { sql } from './client'
import { getRelationshipEvidenceForPersons } from './relationship-evidence'
import { summarizeRelationshipEvidence } from '../lib/relationship-intel/relationship-context'

// Read-only attention projection (CRM-10). Surfaces operational follow-up
// categories derived deterministically from existing task, interaction, and
// person/deal/property data. No AI scoring or invented prioritization.

export type AttentionTask = {
  id: string
  personId: string | null
  title: string
  detail: string | null
  dueAt: string | null
  dueAtLabel: string | null
  isOverdue: boolean
  personName: string | null
  propertyName: string | null
  dealPropertyName: string | null
}

export type AttentionPerson = {
  id: string
  displayName: string
  role: string
  status: string
  openTaskCount: number
  lastContactChannel: string | null
  lastContactLabel: string | null
}

export type AttentionQuietRelationship = {
  id: string
  displayName: string
  role: string
  status: string
  activeDealCount: number
  openTaskCount: number
  lastContactLabel: string | null
}

export type AttentionSnapshot = {
  overdueTasks: AttentionTask[]
  dueSoonTasks: AttentionTask[]
  peopleWithOpenWork: AttentionPerson[]
  quietButImportant: AttentionQuietRelationship[]
}

const DUE_SOON_WINDOW_DAYS = 7
const QUIET_THRESHOLD_DAYS = 30

type TaskRow = {
  id: string
  person_id: string | null
  title: string
  detail: string | null
  due_at: string | null
  due_at_label: string | null
  person_name: string | null
  property_name: string | null
  deal_property_name: string | null
}

type PersonRow = {
  id: string
  display_name: string
  role: string
  status: string
  open_task_count: number
  last_contact_channel: string | null
  last_contact_label: string | null
}

type QuietRow = {
  id: string
  display_name: string
  role: string
  status: string
  active_deal_count: number
  open_task_count: number
  last_contact_label: string | null
}

export async function getAttentionSnapshot(): Promise<AttentionSnapshot> {
  const quietCutoffIso = new Date(
    Date.now() - QUIET_THRESHOLD_DAYS * 24 * 60 * 60 * 1000,
  ).toISOString()

  const [taskRows, personRows, quietRows] = await Promise.all([
    sql`
      select
        t.id,
        person.id as person_id,
        t.title,
        t.detail,
        t.due_at,
        to_char(
          t.due_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as due_at_label,
        person.display_name as person_name,
        property.name as property_name,
        deal_property.name as deal_property_name
      from task t
      left join person
        on person.id = t.person_id
      left join property
        on property.id = t.property_id
      left join deal
        on deal.id = t.deal_id
      left join property deal_property
        on deal_property.id = deal.property_id
      where t.status = 'open'
      order by
        t.due_at asc nulls last,
        t.created_at asc
    `,
    sql`
      select
        p.id,
        p.display_name,
        p.role,
        p.status,
        count(t.id)::int as open_task_count,
        latest.channel as last_contact_channel,
        to_char(
          latest.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as last_contact_label
      from person p
      join task t
        on t.person_id = p.id
        and t.status = 'open'
      left join lateral (
        select i.channel, i.occurred_at
        from interaction i
        where i.person_id = p.id
        order by i.occurred_at desc
        limit 1
      ) latest on true
      where p.archived_at is null
      group by
        p.id,
        p.display_name,
        p.role,
        p.status,
        latest.channel,
        latest.occurred_at
      order by count(t.id) desc, p.display_name asc
    `,
    sql`
      select
        p.id,
        p.display_name,
        p.role,
        p.status,
        (
          select count(*)::int
          from deal d
          where d.stage <> 'closed'
            and exists (
              select 1
              from deal_participant dp
              where dp.deal_id = d.id
                and dp.person_id = p.id
                and dp.role = 'client'
                and dp.active = true
            )
        ) as active_deal_count,
        (
          select count(*)::int
          from task t
          where t.person_id = p.id
            and t.status = 'open'
        ) as open_task_count,
        to_char(
          latest.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as last_contact_label
      from person p
      left join lateral (
        select i.occurred_at
        from interaction i
        where i.person_id = p.id
        order by i.occurred_at desc
        limit 1
      ) latest on true
      where p.archived_at is null
        and (
          (
            select count(*)::int
            from deal d
            where d.stage <> 'closed'
              and exists (
                select 1
                from deal_participant dp
                where dp.deal_id = d.id
                  and dp.person_id = p.id
                  and dp.role = 'client'
                  and dp.active = true
              )
          ) > 0
          or (
            select count(*)::int
            from task t
            where t.person_id = p.id
              and t.status = 'open'
          ) > 0
        )
        and (
          latest.occurred_at is null
          or latest.occurred_at < ${quietCutoffIso}
        )
      order by latest.occurred_at asc nulls first, p.display_name asc
    `,
  ])

  const now = Date.now()
  const dueSoonCutoff =
    now + DUE_SOON_WINDOW_DAYS * 24 * 60 * 60 * 1000

  const overdueTasks: AttentionTask[] = []
  const dueSoonTasks: AttentionTask[] = []

  for (const raw of taskRows as TaskRow[]) {
    const dueAt = raw.due_at ?? null
    const task: AttentionTask = {
      id: raw.id,
      personId: raw.person_id ?? null,
      title: raw.title,
      detail: raw.detail ?? null,
      dueAt,
      dueAtLabel: raw.due_at_label ?? null,
      isOverdue: dueAt !== null && new Date(dueAt).getTime() < now,
      personName: raw.person_name ?? null,
      propertyName: raw.property_name ?? null,
      dealPropertyName: raw.deal_property_name ?? null,
    }

    if (task.isOverdue) {
      overdueTasks.push(task)
    } else if (dueAt !== null && new Date(dueAt).getTime() <= dueSoonCutoff) {
      dueSoonTasks.push(task)
    }
  }

  return {
    overdueTasks,
    dueSoonTasks,
    peopleWithOpenWork: (personRows as PersonRow[]).map((row) => ({
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,
      openTaskCount: row.open_task_count,
      lastContactChannel: row.last_contact_channel ?? null,
      lastContactLabel: row.last_contact_label ?? null,
    })),
    quietButImportant: (quietRows as QuietRow[]).map((row) => ({
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,
      activeDealCount: row.active_deal_count,
      openTaskCount: row.open_task_count,
      lastContactLabel: row.last_contact_label ?? null,
    })),
  }
}

// ---------------------------------------------------------------------------
// REL-INTEL — Catch-Up relationship context (derived, conservative).
// Surfaces trustworthy relationship memory for the people already in the
// attention snapshot. Bulk/service evidence never refreshes "meaningful"
// contact. Partial Gmail coverage produces an explicit limited/limited-note
// rather than fake freshness. Deterministic, human-readable reasons only.
// ---------------------------------------------------------------------------

export type PersonRelationshipContext = {
  personId: string
  hasEvidence: boolean
  sources: string[]
  lastMeaningfulContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  twoWay: boolean
  hasEmail: boolean
  hasPhone: boolean
  coverageLimited: boolean
  reason: string | null
}

/**
 * Build relationship context for the people in a Catch-Up snapshot. Bounded:
 * only reads evidence for the people already surfaced in the snapshot.
 */
export async function getAttentionRelationshipContext(
  snapshot: Pick<AttentionSnapshot, 'peopleWithOpenWork' | 'quietButImportant' | 'overdueTasks' | 'dueSoonTasks'>,
): Promise<Record<string, PersonRelationshipContext>> {
  const ids = new Set<string>()
  for (const p of snapshot.peopleWithOpenWork) ids.add(p.id)
  for (const p of snapshot.quietButImportant) ids.add(p.id)
  for (const t of snapshot.overdueTasks) if (t.personId) ids.add(t.personId)
  for (const t of snapshot.dueSoonTasks) if (t.personId) ids.add(t.personId)

  const byPerson = await getRelationshipEvidenceForPersons([...ids])
  const out: Record<string, PersonRelationshipContext> = {}

  for (const personId of ids) {
    const summary = summarizeRelationshipEvidence(byPerson[personId] ?? [])
    out[personId] = {
      personId,
      hasEvidence: summary.hasEvidence,
      sources: summary.sources,
      lastMeaningfulContactAt: summary.lastMeaningfulContactAt,
      lastInboundAt: summary.lastInboundAt,
      lastOutboundAt: summary.lastOutboundAt,
      twoWay: summary.twoWay,
      hasEmail: summary.hasEmail,
      hasPhone: summary.hasPhone,
      coverageLimited: summary.coverageLimited,
      reason: summary.reason,
    }
  }

  return out
}
