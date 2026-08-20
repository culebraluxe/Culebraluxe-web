import { sql } from './client'

// Read-only Showings projection (CRM-11 read-side). Surfaces the actual
// showing lifecycle records from the canonical `showing` table. No workflow,
// scheduling, completion, or write behavior is introduced here.

export type ShowingStatus = 'requested' | 'scheduled' | 'completed' | 'cancelled'

export type Showing = {
  id: string
  personId: string
  personName: string
  propertyId: string | null
  propertyName: string | null
  dealId: string | null
  dealPropertyName: string | null
  status: ShowingStatus
  requestedAtLabel: string
  scheduledAtLabel: string | null
  completedAtLabel: string | null
  cancelledAtLabel: string | null
  feedback: string | null
}

type ShowingRow = {
  id: string
  person_id: string
  person_name: string
  property_id: string | null
  property_name: string | null
  deal_id: string | null
  deal_property_name: string | null
  status: ShowingStatus
  requested_at_label: string
  scheduled_at_label: string | null
  completed_at_label: string | null
  cancelled_at_label: string | null
  feedback: string | null
}

export async function getShowings(): Promise<Showing[]> {
  const rows = await sql`
    select
      s.id,
      s.person_id,
      person.display_name as person_name,
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
      to_char(
        s.cancelled_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
      ) as cancelled_at_label,
      s.feedback
    from showing s
    join person
      on person.id = s.person_id
    left join property
      on property.id = s.property_id
    left join deal d
      on d.id = s.deal_id
    left join property deal_property
      on deal_property.id = d.property_id
    order by
      case s.status
        when 'requested' then 0
        when 'scheduled' then 1
        when 'completed' then 2
        when 'cancelled' then 3
        else 4
      end,
      s.requested_at desc
  `

  return (rows as ShowingRow[]).map((row) => ({
    id: row.id,
    personId: row.person_id,
    personName: row.person_name,
    propertyId: row.property_id ?? null,
    propertyName: row.property_name ?? null,
    dealId: row.deal_id ?? null,
    dealPropertyName: row.deal_property_name ?? null,
    status: row.status,
    requestedAtLabel: row.requested_at_label,
    scheduledAtLabel: row.scheduled_at_label ?? null,
    completedAtLabel: row.completed_at_label ?? null,
    cancelledAtLabel: row.cancelled_at_label ?? null,
    feedback: row.feedback ?? null,
  }))
}
