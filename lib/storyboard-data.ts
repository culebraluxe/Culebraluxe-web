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
  'Complete',
  'Complete V1',
  'Complete V2',
  'Operationalized',
  'Operationalized V1',
  'Operationalized V2',
  'Read-side complete',
  'Read-side V1',
  'Read-side V1 complete',
  'Readiness PASS',
  'Partial',
  'strong V1 core',
  'Minor remainder',
  'Browser-local V1',
  'Planned',
  'Open',
  'Blocked',
  'Deferred',
  'Hardware/content dependent',
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
  acceptanceCriteria: string | null
  dependencies: string | null
  /** Human-authored 0–100 completion (informational; rollup uses status). */
  completion: number
  /** Whether the story participates in the workstream rollup. */
  rollup: boolean
}

// ---------------------------------------------------------------------------
// Status completion scoring and buckets (8/21 master board policy).
//
//   complete  — Complete / Complete V1 / Complete V2 / Operationalized /
//               Operationalized V1 / Operationalized V2              → 1.00
//   partial   — Read-side complete / Read-side V1 / Read-side V1 complete /
//               Readiness PASS (0.80) and Partial / strong V1 core /
//               Minor remainder / Browser-local V1 (0.50)
//   open      — Planned / Open / Deferred / Hardware/content dependent → 0.00
//   blocked   — Blocked                                                → 0.00
//
// Human-readable labels are stored on the story; scoring/bucketing happens
// internally from these maps.
// ---------------------------------------------------------------------------

export type StatusBucket = 'complete' | 'partial' | 'open' | 'blocked'

export const STATUS_SCORE: Record<StoryStatus, number> = {
  Complete: 1,
  'Complete V1': 1,
  'Complete V2': 1,
  Operationalized: 1,
  'Operationalized V1': 1,
  'Operationalized V2': 1,
  'Read-side complete': 0.8,
  'Read-side V1': 0.8,
  'Read-side V1 complete': 0.8,
  'Readiness PASS': 0.8,
  Partial: 0.5,
  'strong V1 core': 0.5,
  'Minor remainder': 0.5,
  'Browser-local V1': 0.5,
  Planned: 0,
  Open: 0,
  Blocked: 0,
  Deferred: 0,
  'Hardware/content dependent': 0,
}

export const STATUS_BUCKET: Record<StoryStatus, StatusBucket> = {
  Complete: 'complete',
  'Complete V1': 'complete',
  'Complete V2': 'complete',
  Operationalized: 'complete',
  'Operationalized V1': 'complete',
  'Operationalized V2': 'complete',
  'Read-side complete': 'partial',
  'Read-side V1': 'partial',
  'Read-side V1 complete': 'partial',
  'Readiness PASS': 'partial',
  Partial: 'partial',
  'strong V1 core': 'partial',
  'Minor remainder': 'partial',
  'Browser-local V1': 'partial',
  Planned: 'open',
  Open: 'open',
  Deferred: 'open',
  'Hardware/content dependent': 'open',
  Blocked: 'blocked',
}

export function statusScore(status: string): number {
  return STATUS_SCORE[status as StoryStatus] ?? 0
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
// the mean status score over the workstream's rollup stories.
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
}

function round(value: number, digits = 1) {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
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

    const completionPercent =
      storyCount > 0
        ? round(
            (group.reduce((sum, s) => sum + statusScore(s.status), 0) /
              storyCount) *
              100,
          )
        : 0

    return {
      workstream: ws.name,
      code: ws.code,
      weight: ws.weight,
      storyCount,
      completeCount,
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

  return { stories, workstreams, netNet }
}
