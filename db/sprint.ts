import { PortalWriteError } from '../lib/portal-write-error'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Sprint repository (migration 187).
//
// A SPRINT IS THE PARENT OF THE STORIES WHOSE BATCH EQUALS ITS NUMBER. The batch integer stays the
// engine-facing axis — the engine, the release scheduler and the storyboard all read
// `storyboard_story.batch` — and `storyboard_sprint` is that same integer with a goal, a status, an
// owner and an outcome attached, so a sprint can be judged by something other than a story count.
//
// The child link (`storyboard_story.sprint_id`) is DERIVED IN THE DATABASE by trigger and never
// written from here: this module writes the parent and reads the rollup. A disagreement between a
// story's sprint and its batch is refused by the trigger by name, so there is no code path here that
// could quietly reconcile one.
//
// The close rule exists TWICE, deliberately: as a CHECK constraint (so no writer can bypass it) and
// as `sprintCloseRefusal` (so the refusal is a sentence naming what is missing rather than a bare
// constraint code — measured 2026-09-17: the operator-facing client surfaces a raised message as
// `CONSTRAINT`, which tells a human nothing).
//
// The default executor is resolved lazily (mirroring db/storyboard.ts) so importing this module never
// requires a DATABASE_URL; tests inject an in-memory fake.
// ---------------------------------------------------------------------------

export type SprintStatus = 'Planned' | 'Active' | 'Closing' | 'Closed' | 'Cancelled'

export const SPRINT_STATUSES: SprintStatus[] = [
  'Planned',
  'Active',
  'Closing',
  'Closed',
  'Cancelled',
]

export type Sprint = {
  id: string
  number: number
  title: string
  theme: string | null
  goal: string | null
  status: SprintStatus
  owner: string | null
  startedAt: string | null
  targetEndAt: string | null
  closedAt: string | null
  outcome: string | null
  notes: string | null
}

export type SprintRollup = Sprint & {
  stories: number
  storiesComplete: number
  storiesOpen: number
  storiesHeld: number
  /** Null for a sprint with no stories: 0% and 0/0 are different facts. */
  percentComplete: number | null
  firstCompletedAt: string | null
  lastCompletedAt: string | null
}

/**
 * The one spelling of the batch axis. `batch 92` and sprint `S92` are the same sprint, and this is the
 * only place the pairing is expressed in TypeScript.
 */
export function sprintIdForBatch(batch: number | null | undefined): string | null {
  if (batch === null || batch === undefined) return null
  if (!Number.isFinite(batch)) return null
  return `S${batch}`
}

/** The number a sprint id carries, or null when the id is not `S<digits>`. */
export function sprintNumberFromId(id: string | null | undefined): number | null {
  const match = /^S(\d+)$/.exec((id ?? '').trim())
  if (!match) return null
  return Number(match[1])
}

/**
 * WHY A SPRINT MAY NOT CLOSE YET — or null when it may.
 *
 * Mirrors the `storyboard_sprint_closed_says_what_happened` constraint exactly, including its shape:
 * every branch is true or false and never null, because a CHECK treats NULL as satisfied and the
 * first version of that rule therefore let an empty close through.
 */
export function sprintCloseRefusal(sprint: {
  status: string
  closedAt: string | null
  outcome: string | null
}): string | null {
  const outcome = (sprint.outcome ?? '').trim()
  if (sprint.status !== 'Closed') return null
  if (!sprint.closedAt) {
    return 'a closed sprint needs closed_at: record when it closed, not only that it did'
  }
  if (outcome.length === 0) {
    return 'a closed sprint needs an outcome: say what actually happened, or it cannot be judged'
  }
  return null
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function dateOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return String(value)
}

function numberOrNull(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const n = Number(value)
  return Number.isFinite(n) ? n : null
}

function textOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  return String(value)
}

function mapSprint(row: QueryRow): Sprint {
  return {
    id: String(row.id),
    number: Number(row.number),
    title: String(row.title),
    theme: textOrNull(row.theme),
    goal: textOrNull(row.goal),
    status: String(row.status) as SprintStatus,
    owner: textOrNull(row.owner),
    startedAt: dateOrNull(row.started_at),
    targetEndAt: dateOrNull(row.target_end_at),
    closedAt: dateOrNull(row.closed_at),
    outcome: textOrNull(row.outcome),
    notes: textOrNull(row.notes),
  }
}

function mapRollup(row: QueryRow): SprintRollup {
  return {
    ...mapSprint(row),
    stories: Number(row.stories ?? 0),
    storiesComplete: Number(row.stories_complete ?? 0),
    storiesOpen: Number(row.stories_open ?? 0),
    storiesHeld: Number(row.stories_held ?? 0),
    percentComplete: numberOrNull(row.percent_complete),
    firstCompletedAt: dateOrNull(row.first_completed_at),
    lastCompletedAt: dateOrNull(row.last_completed_at),
  }
}

export async function isSprintTableReady(execute?: QueryExecutor): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`select to_regclass('storyboard_sprint') is not null as ready`
  return rows[0]?.ready === true
}

/** Every sprint with its children counted, newest number first. Null when the table is not there yet. */
export async function listSprintRollups(execute?: QueryExecutor): Promise<SprintRollup[] | null> {
  const q = execute ?? (await executor())
  if (!(await isSprintTableReady(q))) return null
  const rows = await q`select * from storyboard_sprint_rollup order by number desc`
  return rows.map(mapRollup)
}

export async function getSprintByNumber(
  number: number,
  execute?: QueryExecutor,
): Promise<SprintRollup | null> {
  const q = execute ?? (await executor())
  const rows = await q`select * from storyboard_sprint_rollup where number = ${number}`
  return rows[0] ? mapRollup(rows[0]) : null
}

export type OpenSprintInput = {
  number: number
  title: string
  theme?: string | null
  goal?: string | null
  owner?: string | null
}

/**
 * Create the sprint a batch belongs to. REFUSES when the sprint already exists rather than
 * overwriting a goal somebody may have written, because a sprint whose goal is silently replaced is
 * a sprint nobody can be held to.
 */
export async function openSprint(
  input: OpenSprintInput,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(input.number)
  if (!id) {
    throw new PortalWriteError(
      'validation',
      `a sprint number is required (got ${String(input.number)})`,
    )
  }
  const title = input.title.trim()
  if (title.length === 0) {
    throw new PortalWriteError('validation', `sprint ${id} needs a title: name what this sprint is`)
  }
  const existing = await q`select id from storyboard_sprint where number = ${input.number}`
  if (existing.length > 0) {
    throw new PortalWriteError(
      'conflict',
      `sprint ${id} already exists; open the next number, or change its goal with the goal command`,
    )
  }
  await q`
    insert into storyboard_sprint (id, number, title, theme, goal, owner, status, started_at)
    values (${id}, ${input.number}, ${title}, ${input.theme ?? null}, ${input.goal ?? null},
            ${input.owner ?? null}, 'Active', now())
  `
  const created = await getSprintByNumber(input.number, q)
  if (!created) throw new PortalWriteError('not-found', `sprint ${id} was not created`)
  return created
}

/**
 * Set (or correct) the goal. A sprint with no goal cannot be judged at close, so writing one is a
 * real step rather than a note.
 */
export async function setSprintGoal(
  number: number,
  goal: string,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(number)
  const text = goal.trim()
  if (!id || text.length === 0) {
    throw new PortalWriteError('validation', 'a sprint goal must say something')
  }
  const rows = await q`
    update storyboard_sprint set goal = ${text}, updated_at = now()
    where number = ${number} returning id
  `
  if (rows.length === 0) throw new PortalWriteError('not-found', `sprint ${id} does not exist`)
  const updated = await getSprintByNumber(number, q)
  if (!updated) throw new PortalWriteError('not-found', `sprint ${id} could not be read back`)
  return updated
}

/**
 * Close the sprint and record what actually happened. The rule is checked HERE first so the refusal
 * is a sentence a human can act on, and again by the CHECK constraint so no other writer can skip it.
 */
export async function closeSprint(
  number: number,
  outcome: string,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(number)
  const text = outcome.trim()
  const current = await getSprintByNumber(number, q)
  if (!current) throw new PortalWriteError('not-found', `sprint ${id} does not exist`)
  const refusal = sprintCloseRefusal({
    status: 'Closed',
    closedAt: current.closedAt ?? new Date().toISOString(),
    outcome: text,
  })
  if (refusal) throw new PortalWriteError('validation', `cannot close ${id}: ${refusal}`)
  await q`
    update storyboard_sprint
    set status = 'Closed', closed_at = coalesce(closed_at, now()), outcome = ${text}, updated_at = now()
    where number = ${number}
  `
  const closed = await getSprintByNumber(number, q)
  if (!closed) throw new PortalWriteError('not-found', `sprint ${id} could not be read back`)
  return closed
}
