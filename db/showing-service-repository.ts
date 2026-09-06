import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  SaveShowingReportRequest,
  ShowingDto,
  ShowingRepository,
  ShowingReportOutcome,
} from '@/services/showing'

type ShowingRow = {
  id: string
  person_id: string
  property_id: string | null
  status: string
  showing_date: string | Date | null
  duration: string | null
  outcome: string | null
  interest_score: number | string | null
  feedback: string | null
  follow_up: string | null
  completed_at: string | Date | null
}

function isoDate(value: string | Date | null): string | null {
  if (value === null) return null
  if (value instanceof Date) return value.toISOString().slice(0, 10)
  return String(value).slice(0, 10)
}

function isoTimestamp(value: string | Date | null): string | null {
  if (value === null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function mapRow(row: ShowingRow): ShowingDto {
  return {
    id: row.id,
    personId: row.person_id,
    propertyId: row.property_id ?? '',
    status: row.status,
    showingDate: isoDate(row.showing_date),
    duration: row.duration,
    outcome: row.outcome as ShowingReportOutcome | null,
    interestScore: row.interest_score === null ? null : Number(row.interest_score),
    feedback: row.feedback,
    followUp: row.follow_up,
    completedAt: isoTimestamp(row.completed_at),
  }
}

export class SqlShowingRepository implements ShowingRepository {
  constructor(private readonly query: QueryExecutor = sql) {}

  async get(showingId: string): Promise<ShowingDto | null> {
    const rows = (await this.query`
      select
        id, person_id, property_id, status, showing_date, duration,
        outcome, interest_score, feedback, follow_up, completed_at
      from showing
      where id = ${showingId}
      limit 1
    `) as ShowingRow[]
    return rows[0] ? mapRow(rows[0]) : null
  }

  async saveReport(request: SaveShowingReportRequest): Promise<ShowingDto> {
    const status = request.outcome
      ? 'completed'
      : request.showingDate
        ? 'scheduled'
        : 'requested'

    const rows = (await this.query`
      insert into showing (
        id, person_id, property_id, deal_id, status,
        showing_date, duration, outcome, interest_score, feedback, follow_up,
        completed_at
      ) values (
        ${request.showingId}, ${request.personId}, ${request.propertyId}, null, ${status},
        ${request.showingDate}::date, ${request.duration}, ${request.outcome},
        ${request.interestScore}, ${request.feedback}, ${request.followUp},
        case when ${status} = 'completed' then now() else null end
      )
      on conflict (id) do update
      set status = excluded.status,
          showing_date = excluded.showing_date,
          duration = excluded.duration,
          outcome = excluded.outcome,
          interest_score = excluded.interest_score,
          feedback = excluded.feedback,
          follow_up = excluded.follow_up,
          completed_at = case
            when excluded.status = 'completed' then coalesce(showing.completed_at, now())
            else showing.completed_at
          end,
          updated_at = now()
      where showing.person_id = excluded.person_id
        and showing.property_id is not distinct from excluded.property_id
      returning
        id, person_id, property_id, status, showing_date, duration,
        outcome, interest_score, feedback, follow_up, completed_at
    `) as ShowingRow[]

    if (!rows[0]) {
      throw new Error(
        `Showing ${request.showingId} is already bound to a different Person or Property.`,
      )
    }
    return mapRow(rows[0])
  }
}
