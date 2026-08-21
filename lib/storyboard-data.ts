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

export const STORY_STATUSES = [
  'Planned',
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
  Deferred: 'open',
  Hold: 'open',
  Failed: 'open',
  Blocked: 'blocked',
}

export function statusBucket(status: string): StatusBucket {
  return STATUS_BUCKET[status as StoryStatus] ?? 'open'
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
  }
}
