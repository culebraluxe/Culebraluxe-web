import { PortalWriteError } from '../lib/portal-write-error'
import type {
  StoryPriority,
  StoryStatus,
  Workstream,
} from '../lib/storyboard-data'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Storyboard story repository (migration 021).
//
// /portal/storyboard reads and edits these rows. Story IDs are human-assigned
// and are the primary key; nothing here auto-generates them. The default
// executor is resolved lazily (mirroring db/tx.ts) so importing this module
// never requires a DATABASE_URL; tests inject an in-memory fake.
// ---------------------------------------------------------------------------

export type StoryboardStory = {
  id: string
  workstream: Workstream
  title: string
  priority: StoryPriority
  status: StoryStatus
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  acceptanceCriteria: string | null
  dependencies: string | null
  createdAt: string
  updatedAt: string
}

export type StoryboardStoryInput = {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  acceptanceCriteria: string | null
  dependencies: string | null
}

export type StoryboardStoryUpdate = Omit<StoryboardStoryInput, 'id'>

type StoryRow = QueryRow & {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  acceptance_criteria: string | null
  dependencies: string | null
  created_at: string
  updated_at: string
}

// NOTE: column lists are written literally in every query. The Neon driver
// parameterizes interpolated string values (a `select ${cols}` would become
// `select $1` and return a `?column?` row), so a shared string constant can
// never be interpolated into these templates.

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function mapStory(row: StoryRow): StoryboardStory {
  return {
    id: row.id,
    workstream: row.workstream as Workstream,
    title: row.title,
    priority: row.priority as StoryPriority,
    status: row.status as StoryStatus,
    notes: row.notes,
    batch: row.batch,
    goal: row.goal,
    scope: row.scope,
    acceptanceCriteria: row.acceptance_criteria,
    dependencies: row.dependencies,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

export async function isStoryboardTableReady(
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`
    select to_regclass('storyboard_story') is not null as ready
  `
  return rows[0]?.ready === true
}

export async function listStoryboardStories(
  execute?: QueryExecutor,
): Promise<StoryboardStory[] | null> {
  const q = execute ?? (await executor())
  const ready = await isStoryboardTableReady(q)
  if (!ready) return null

  const rows = await q`
    select id, workstream, title, priority, status, notes, batch, goal, scope,
      acceptance_criteria, dependencies, created_at, updated_at
    from storyboard_story
    order by workstream, id
  `
  return rows.map((row) => mapStory(row as StoryRow))
}

export async function createStoryboardStory(
  input: StoryboardStoryInput,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes, batch, goal, scope,
      acceptance_criteria, dependencies
    ) values (
      ${input.id}, ${input.workstream}, ${input.title}, ${input.priority},
      ${input.status}, ${input.notes}, ${input.batch ?? null},
      ${input.goal ?? null}, ${input.scope ?? null},
      ${input.acceptanceCriteria ?? null}, ${input.dependencies ?? null}
    )
    on conflict (id) do nothing
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, acceptance_criteria, dependencies, created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError(
      'conflict',
      `Story "${input.id}" already exists. Choose a different ID.`,
    )
  }
  return mapStory(row)
}

export async function updateStoryboardStory(
  id: string,
  input: StoryboardStoryUpdate,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set workstream = ${input.workstream},
        title = ${input.title},
        priority = ${input.priority},
        status = ${input.status},
        notes = ${input.notes},
        batch = ${input.batch ?? null},
        goal = ${input.goal ?? null},
        scope = ${input.scope ?? null},
        acceptance_criteria = ${input.acceptanceCriteria ?? null},
        dependencies = ${input.dependencies ?? null},
        updated_at = now()
    where id = ${id}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, acceptance_criteria, dependencies, created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Story "${id}" was not found.`)
  }
  return mapStory(row)
}

export async function setStoryboardStatus(
  id: string,
  status: string,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set status = ${status},
        updated_at = now()
    where id = ${id}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, acceptance_criteria, dependencies, created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Story "${id}" was not found.`)
  }
  return mapStory(row)
}
