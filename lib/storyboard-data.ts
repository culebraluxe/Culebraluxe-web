// ---------------------------------------------------------------------------
// CulebraLuxe Story Board — vocabulary and rollup model.
//
// This module owns the program vocabulary (workstreams, statuses, priorities),
// the status completion scoring, and the workstream rollup / net-net
// computation. Story data is NOT stored here: the authoritative 8/21 master
// board (74 human-authored stories) is seeded into Neon by
// db/migrations/022_storyboard_authoritative_seed.sql and read through
// db/storyboard.ts. Nothing here derives stories from the repository and no
// backlog items are created.
// ---------------------------------------------------------------------------

import { OPERATING_SURFACE_ORDER } from './navigation'

/** Workstream code stored on each story, with its rollup name and weight. */
export const WORKSTREAMS = [
  { code: 'PUBLIC', name: 'Public Website / Property Experience', weight: 20 },
  { code: 'CRM', name: 'CRM Foundation / Intake Core', weight: 20 },
  { code: 'PORTAL', name: 'Portal / Relationship Operations', weight: 20 },
  { code: 'TXN', name: 'Transaction / Deal Workflow', weight: 15 },
  { code: 'ADMIN', name: 'Admin / Data Management', weight: 10 },
  { code: 'AUTH', name: 'Auth / Security / User Access', weight: 5 },
  { code: 'CONTENT', name: 'Content / Marketing / Source Cleanup', weight: 5 },
  { code: 'HARDEN', name: 'Integrations / Operational Hardening', weight: 5 },
] as const

export type Workstream = (typeof WORKSTREAMS)[number]['code']

export const WORKSTREAM_CODES: readonly string[] = WORKSTREAMS.map(
  (w) => w.code,
)

export function workstreamName(code: string): string {
  return WORKSTREAMS.find((w) => w.code === code)?.name ?? code
}

// ---------------------------------------------------------------------------
// Operating surfaces (UI-01) — a SECOND organizing axis, INDEPENDENT from
// workstream. The canonical tokens come from the navigation registry so the
// Story Board vocabulary and the application shell can never drift apart.
// NULL on a story means "not yet deliberately classified" — it is never
// silently interpreted as NEXUS, OPS, TECH or SUPPORT.
// ---------------------------------------------------------------------------

export const OPERATING_SURFACES: readonly string[] =
  OPERATING_SURFACE_ORDER as readonly string[]

export type OperatingSurface = (typeof OPERATING_SURFACE_ORDER)[number]

export const OPERATING_SURFACE_CODES: readonly string[] = OPERATING_SURFACE_ORDER.map(
  (s) => s,
)

/** Display helper: null stays "Unclassified" — never a fake surface. */
export function operatingSurfaceName(code: string | null): string {
  if (!code) return 'Unclassified'
  const surface = code.toUpperCase()
  if ((OPERATING_SURFACE_CODES as readonly string[]).includes(surface)) {
    return surface
  }
  return 'Unclassified'
}

export const STORY_STATUSES = [
  'Planned',
  'Ready',
  'In Progress',
  'Complete',
  'Partial',
  'Blocked',
  'Failed',
  'Deferred',
  'Hold',
] as const

export type StoryStatus = (typeof STORY_STATUSES)[number]

export const STORY_PRIORITIES = [
  'Critical',
  'High',
  'High-ish',
  'Medium-High',
  'Medium',
  'Low',
  'Later',
  'High-value polish',
] as const

export type StoryPriority = (typeof STORY_PRIORITIES)[number]

export type StoryRecord = {
  id: string
  workstream: Workstream
  /** UI-01: second organizing axis (NEXUS | OPS | TECH | SUPPORT); null = not
   *  yet deliberately classified. Independent from workstream. */
  operatingSurface: OperatingSurface | null
  title: string
  priority: StoryPriority
  status: StoryStatus
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  dependencies: string | null
  preconditions: string | null
  architectBrief: string | null
  contextRefs: string | null
  acceptanceCriteria: string | null
  postconditions: string | null
  architectBriefUpdatedAt: string | null
  /** Authoritative 0–100 numeric progress; drives the rollup math. */
  completion: number
  /** Whether the story participates in the workstream rollup. */
  rollup: boolean
  plannedStartAt: string | null
  actualStartAt: string | null
  completedAt: string | null
  createdAt: string
  updatedAt: string
}

// ---------------------------------------------------------------------------
// Status buckets (counts only).
//
// Completion math uses storyboard_story.completion (0..100), NOT status.
// Status is categorical state; buckets are used for the count columns only:
//
//   complete — Complete
//   partial  — In Progress, Partial
//   open     — Planned, Deferred, Hold, Failed
//   blocked  — Blocked
// ---------------------------------------------------------------------------

export type StatusBucket = 'complete' | 'partial' | 'open' | 'blocked'

export const STATUS_BUCKET: Record<StoryStatus, StatusBucket> = {
  Complete: 'complete',
  'In Progress': 'partial',
  Partial: 'partial',
  Planned: 'open',
  Ready: 'open',
  Deferred: 'open',
  Hold: 'open',
  Failed: 'open',
  Blocked: 'blocked',
}

export function statusBucket(status: string): StatusBucket {
  return STATUS_BUCKET[status as StoryStatus] ?? 'open'
}

// ---------------------------------------------------------------------------
// Story Board filtering / search model
//
// Pure functions over the authoritative stored stories. URL query parameters
// are parsed into a StoryBoardFilter (stable names: q, workstream, status,
// priority, view, rollup) and serialized back for links/bookmarks. The
// executive dashboard and rollup math are computed from the FULL story set
// and never depend on the active filter.
// ---------------------------------------------------------------------------

export type StoryBoardView =
  | 'all'
  | 'open'
  | 'blocked-failed'
  | 'complete'
  | 'deferred-hold'

export const WORK_VIEWS: ReadonlyArray<{
  code: StoryBoardView
  label: string
  statuses: readonly StoryStatus[]
}> = [
  { code: 'all', label: 'All', statuses: [] },
  {
    code: 'open',
    label: 'Open Work',
    statuses: ['Planned', 'Ready', 'In Progress', 'Partial'],
  },
  {
    code: 'blocked-failed',
    label: 'Blocked / Failed',
    statuses: ['Blocked', 'Failed'],
  },
  { code: 'complete', label: 'Complete', statuses: ['Complete'] },
  {
    code: 'deferred-hold',
    label: 'Deferred / Hold',
    statuses: ['Deferred', 'Hold'],
  },
]

export type StoryBoardFilter = {
  q: string
  workstream: 'all' | Workstream
  status: 'all' | StoryStatus
  priority: 'all' | StoryPriority
  view: StoryBoardView
  rollup: 'all' | 'in' | 'out'
  /** SB-01: operating-surface filter; 'unclassified' = NULL operating_surface. */
  surface: 'all' | OperatingSurface | 'unclassified'
}

export function defaultStoryBoardFilter(): StoryBoardFilter {
  return {
    q: '',
    workstream: 'all',
    status: 'all',
    priority: 'all',
    view: 'all',
    rollup: 'all',
    surface: 'all',
  }
}

type SearchParamsLike = Record<string, string | string[] | undefined>

function firstParam(params: SearchParamsLike, key: string): string | undefined {
  const value = params[key]
  return Array.isArray(value) ? value[0] : value
}

export function parseStoryBoardFilter(
  params: SearchParamsLike,
): StoryBoardFilter {
  const filter = defaultStoryBoardFilter()

  const q = firstParam(params, 'q')
  if (q) filter.q = q

  const workstream = firstParam(params, 'workstream')
  if (
    workstream &&
    (WORKSTREAM_CODES as readonly string[]).includes(workstream)
  ) {
    filter.workstream = workstream as Workstream
  }

  const status = firstParam(params, 'status')
  if (status && (STORY_STATUSES as readonly string[]).includes(status)) {
    filter.status = status as StoryStatus
  }

  const priority = firstParam(params, 'priority')
  if (priority && (STORY_PRIORITIES as readonly string[]).includes(priority)) {
    filter.priority = priority as StoryPriority
  }

  const view = firstParam(params, 'view')
  if (view && WORK_VIEWS.some((v) => v.code === view)) {
    filter.view = view as StoryBoardView
  }

  const rollup = firstParam(params, 'rollup')
  if (rollup === 'in' || rollup === 'out') filter.rollup = rollup

  const surface = firstParam(params, 'surface')
  if (
    surface &&
    (surface === 'unclassified' ||
      (OPERATING_SURFACE_CODES as readonly string[]).includes(surface))
  ) {
    filter.surface = surface as StoryBoardFilter['surface']
  }

  return filter
}

export function storyBoardFilterToQuery(filter: StoryBoardFilter): string {
  const params = new URLSearchParams()
  if (filter.q.trim()) params.set('q', filter.q.trim())
  if (filter.workstream !== 'all') params.set('workstream', filter.workstream)
  if (filter.status !== 'all') params.set('status', filter.status)
  if (filter.priority !== 'all') params.set('priority', filter.priority)
  if (filter.view !== 'all') params.set('view', filter.view)
  if (filter.rollup !== 'all') params.set('rollup', filter.rollup)
  // Defensive: older callers may omit the field entirely (undefined == 'all').
  if (filter.surface && filter.surface !== 'all')
    params.set('surface', filter.surface)
  return params.toString()
}

export function isStoryBoardFilterActive(filter: StoryBoardFilter): boolean {
  return storyBoardFilterToQuery(filter) !== ''
}

const SEARCH_FIELDS: ReadonlyArray<keyof StoryRecord> = [
  'id',
  'title',
  'notes',
  'goal',
  'architectBrief',
  'acceptanceCriteria',
  'dependencies',
  'preconditions',
  'contextRefs',
  'postconditions',
]

export function storyMatchesQuery(story: StoryRecord, q: string): boolean {
  const needle = q.trim().toLowerCase()
  if (!needle) return true
  return SEARCH_FIELDS.some((field) => {
    const value = story[field]
    return (
      typeof value === 'string' && value.toLowerCase().includes(needle)
    )
  })
}

export function filterStories(
  stories: StoryRecord[],
  filter: StoryBoardFilter,
): StoryRecord[] {
  return stories.filter((story) => {
    if (filter.workstream !== 'all' && story.workstream !== filter.workstream) {
      return false
    }
    if (filter.status !== 'all' && story.status !== filter.status) {
      return false
    }
    if (filter.priority !== 'all' && story.priority !== filter.priority) {
      return false
    }
    if (filter.view !== 'all') {
      const view = WORK_VIEWS.find((v) => v.code === filter.view)
      if (view && view.statuses.length > 0 && !view.statuses.includes(story.status)) {
        return false
      }
    }
    if (filter.rollup === 'in' && !story.rollup) return false
    if (filter.rollup === 'out' && story.rollup) return false
    // Defensive: older callers may omit the field entirely (undefined == 'all').
    if (filter.surface && filter.surface !== 'all') {
      if (filter.surface === 'unclassified') {
        if (story.operatingSurface !== null) return false
      } else if (story.operatingSurface !== filter.surface) {
        return false
      }
    }
    return storyMatchesQuery(story, filter.q)
  })
}

// ---------------------------------------------------------------------------
// Rollup model
//
// Per workstream (matching the required rollup fields):
//   workstream, story_count, complete_count, partial_count, open_count,
//   blocked_count, completion_percent
//
// story_count and the four counts cover ROLLUP-participating stories only
// (parents such as CRM-14, CRM-16, PORTAL-01 are stored with rollup=false and
// excluded — their children carry the rollup weight). completion_percent is
// the AVG of the stored completion (0..100) over the workstream's rollup
// stories — status scoring is NOT used for the percentage.
//
// net_net = SUM(workstream_completion_percent * workstream_weight), with
// weights as percentages summing to 100.
// ---------------------------------------------------------------------------

export type WorkstreamRollup = {
  workstream: string
  code: Workstream
  weight: number
  storyCount: number
  completeCount: number
  inProgressPartialCount: number
  blockedFailedCount: number
  partialCount: number
  openCount: number
  blockedCount: number
  completionPercent: number
  /** All stored stories for the workstream, including rollup=false parents. */
  storedCount: number
}

/**
 * SB-01 — per-surface completion projection (NEXUS | OPS | SUPPORT | TECH).
 *
 * Uses the SAME completion semantics as the workstream rollup: completionPercent
 * is the AVG of the stored completion over ROLLUP-participating stories
 * (rollup=true) classified to that surface. Reference / rollup=false rows are
 * never in the percentage or count columns (they cannot pollute the
 * projection); they are only reflected in storedCount. Stories with a NULL
 * operating surface are excluded from every surface projection entirely.
 */
export type SurfaceRollup = {
  surface: OperatingSurface
  /** AVG stored completion over rollup-participating stories, 0–100. */
  completionPercent: number
  /** Rollup-participating story count for this surface. */
  storyCount: number
  /** All stored stories carrying this surface, including rollup=false parents. */
  storedCount: number
  completeCount: number
  openCount: number
  blockedCount: number
}

export type StoryBoardModel = {
  stories: StoryRecord[]
  workstreams: WorkstreamRollup[]
  /** Overall net-net completion, 0–100. */
  netNet: number
  /** Executive summary counts over ALL stored stories. */
  totalStories: number
  totalComplete: number
  totalInProgressPartial: number
  totalBlockedFailed: number
  /** Stories explicitly authorized for coding-agent execution. */
  totalReady: number
  /** SB-01: per-surface completion projections, in Tier-1 order. */
  surfaceRollups: SurfaceRollup[]
  /** Stories deliberately left unclassified (operating_surface IS NULL). */
  unclassifiedCount: number
}

function round(value: number, digits = 1) {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
}

function isInProgressPartial(status: string): boolean {
  return status === 'In Progress' || status === 'Partial'
}

function isBlockedFailed(status: string): boolean {
  return status === 'Blocked' || status === 'Failed'
}

export function buildStoryBoardModel(stories: StoryRecord[]): StoryBoardModel {
  const workstreams: WorkstreamRollup[] = WORKSTREAMS.map((ws) => {
    const stored = stories.filter((s) => s.workstream === ws.code)
    const group = stored.filter((s) => s.rollup)
    const storyCount = group.length

    const completeCount = group.filter(
      (s) => statusBucket(s.status) === 'complete',
    ).length
    const partialCount = group.filter(
      (s) => statusBucket(s.status) === 'partial',
    ).length
    const openCount = group.filter(
      (s) => statusBucket(s.status) === 'open',
    ).length
    const blockedCount = group.filter(
      (s) => statusBucket(s.status) === 'blocked',
    ).length
    const inProgressPartialCount = group.filter((s) =>
      isInProgressPartial(s.status),
    ).length
    const blockedFailedCount = group.filter((s) =>
      isBlockedFailed(s.status),
    ).length

    const completionPercent =
      storyCount > 0
        ? round(
            group.reduce((sum, s) => sum + s.completion, 0) / storyCount,
          )
        : 0

    return {
      workstream: ws.name,
      code: ws.code,
      weight: ws.weight,
      storyCount,
      completeCount,
      inProgressPartialCount,
      blockedFailedCount,
      partialCount,
      openCount,
      blockedCount,
      completionPercent,
      storedCount: stored.length,
    }
  })

  const netNet = round(
    workstreams.reduce(
      (sum, ws) => sum + (ws.completionPercent * ws.weight) / 100,
      0,
    ),
  )

  // SB-01 — per-surface completion projection. SAME completion semantics as the
  // workstream rollup (AVG over rollup-participating stories); reference /
  // rollup=false rows never pollute the percentage and count columns. NULL
  // operating_surface stories are excluded from every surface projection.
  const surfaceRollups: SurfaceRollup[] = OPERATING_SURFACE_ORDER.map(
    (surface) => {
      const stored = stories.filter((s) => s.operatingSurface === surface)
      const group = stored.filter((s) => s.rollup)
      const storyCount = group.length
      return {
        surface,
        completionPercent:
          storyCount > 0
            ? round(
                group.reduce((sum, s) => sum + s.completion, 0) / storyCount,
              )
            : 0,
        storyCount,
        storedCount: stored.length,
        completeCount: group.filter(
          (s) => statusBucket(s.status) === 'complete',
        ).length,
        openCount: group.filter(
          (s) => statusBucket(s.status) === 'open',
        ).length,
        blockedCount: group.filter(
          (s) => statusBucket(s.status) === 'blocked',
        ).length,
      }
    },
  )

  return {
    stories,
    workstreams,
    netNet,
    totalStories: stories.length,
    totalComplete: stories.filter((s) => s.status === 'Complete').length,
    totalInProgressPartial: stories.filter((s) =>
      isInProgressPartial(s.status),
    ).length,
    totalBlockedFailed: stories.filter((s) => isBlockedFailed(s.status)).length,
    totalReady: stories.filter((s) => s.status === 'Ready').length,
    surfaceRollups,
    unclassifiedCount: stories.filter(
      (s) => s.operatingSurface === null,
    ).length,
  }
}
