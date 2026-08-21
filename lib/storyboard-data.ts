// ---------------------------------------------------------------------------
// CulebraLuxe Story Board — vocabulary and dashboard model.
//
// This module owns the program vocabulary (workstreams, statuses, priorities)
// and the dashboard/metrics computation. Story data is NOT stored here: the 41
// existing human-authored stories are seeded into Neon by
// db/migrations/021_storyboard_story.sql and read through db/storyboard.ts.
// Nothing here derives stories from the repository and no backlog items are
// created.
//
// Statuses and priorities use the program vocabulary exactly as required.
// ---------------------------------------------------------------------------

export const WORKSTREAMS = [
  "CRM / Intake",
  "Portal / Operations",
  "Public Property / Buyer Experience",
  "Platform / Engineering / Data",
] as const

export type Workstream = (typeof WORKSTREAMS)[number]

export const STORY_STATUSES = [
  "Complete",
  "Read-side complete",
  "Partial",
  "Planned",
  "Open",
  "Blocked",
  "Deferred",
  "Hardware/content dependent",
  "Operationalized",
  "Minor remainder",
  "Readiness PASS",
] as const

export type StoryStatus = (typeof STORY_STATUSES)[number]

export const STORY_PRIORITIES = [
  "Critical",
  "High",
  "High-ish",
  "Medium-High",
  "Medium",
  "Low",
  "Later",
  "High-value polish",
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
}

// ---------------------------------------------------------------------------
// The 41 existing human-authored stories are seeded into the database by
// db/migrations/021_storyboard_story.sql (the authoritative copy). The page
// reads them from Neon via db/storyboard.ts; no static duplicate lives here.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Dashboard model
//
// weight — the workstream's share of total program weight. Each story
//          contributes priority points (Critical 5 … Later 1); a workstream's
//          weight is its share of the summed points across all stories.
// completionPercent — priority-weighted completion within the workstream,
//          where each status maps to a completion fraction (below).
// weightedContribution — weight × completionPercent (percentage points);
//          summing across workstreams gives overall program completion.
//
// The three summary metrics aggregate the same story data:
//   Architecture / Foundation % — the Platform / Engineering / Data workstream.
//   Usable Product % — CRM / Intake + Portal / Operations + Public Property.
//   Brokerage-Ready % — the brokerage-operational cluster: Portal / Operations
//          workstream stories plus the operational platform stories that must
//          be done for the brokerage to run daily deals end-to-end.
// ---------------------------------------------------------------------------

/** Completion fraction per status. Human-readable policy, not repo-derived. */
export const STATUS_COMPLETION: Record<StoryStatus, number> = {
  Complete: 1,
  Operationalized: 1,
  "Minor remainder": 0.9,
  "Read-side complete": 0.85,
  "Readiness PASS": 0.85,
  Partial: 0.5,
  Planned: 0.15,
  "Hardware/content dependent": 0.15,
  Blocked: 0.1,
  Open: 0.05,
  Deferred: 0,
}

/** Relative weight per priority (used for workstream weights). */
export const PRIORITY_POINTS: Record<StoryPriority, number> = {
  Critical: 5,
  High: 4,
  "High-ish": 3.5,
  "Medium-High": 3,
  Medium: 2,
  Low: 1.5,
  Later: 1,
  "High-value polish": 1,
}

/** Display ordering for priorities (low number = higher on the page). */
export const PRIORITY_ORDER: Record<StoryPriority, number> = {
  Critical: 0,
  High: 1,
  "High-ish": 2,
  "Medium-High": 3,
  Medium: 4,
  Low: 5,
  Later: 6,
  "High-value polish": 7,
}

/** Stories whose completion determines the Brokerage-Ready metric. */
export const BROKERAGE_READY_STORY_IDS: ReadonlySet<string> = new Set([
  "S-012",
  "S-013",
  "S-014",
  "S-015",
  "S-016",
  "S-030",
  "S-031",
  "S-032",
  "S-037",
  "S-038",
])

export type WorkstreamMetric = {
  workstream: Workstream
  weight: number
  completionPercent: number
  weightedContribution: number
  storyCount: number
  completeCount: number
}

export type SummaryMetric = {
  label: string
  percent: number
  detail: string
}

export type StoryBoardModel = {
  stories: StoryRecord[]
  workstreams: WorkstreamMetric[]
  summary: SummaryMetric[]
  overallPercent: number
}

function round(value: number, digits = 1) {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
}

function completionPercentFor(stories: StoryRecord[]): number {
  const total = stories.reduce((sum, s) => sum + PRIORITY_POINTS[s.priority], 0)
  if (total === 0) return 0
  const done = stories.reduce(
    (sum, s) => sum + PRIORITY_POINTS[s.priority] * STATUS_COMPLETION[s.status],
    0,
  )
  return (done / total) * 100
}

export function buildStoryBoardModel(stories: StoryRecord[]): StoryBoardModel {
  const totalWeight = stories.reduce(
    (sum, s) => sum + PRIORITY_POINTS[s.priority],
    0,
  )

  const workstreams = WORKSTREAMS.map((workstream) => {
    const group = stories.filter((s) => s.workstream === workstream)
    const groupWeight = group.reduce(
      (sum, s) => sum + PRIORITY_POINTS[s.priority],
      0,
    )
    const completionPercent = completionPercentFor(group)
    const weight = totalWeight > 0 ? (groupWeight / totalWeight) * 100 : 0

    return {
      workstream,
      weight: round(weight),
      completionPercent: round(completionPercent),
      weightedContribution: round((weight / 100) * completionPercent),
      storyCount: group.length,
      completeCount: group.filter(
        (s) => s.status === "Complete" || s.status === "Operationalized",
      ).length,
    }
  })

  const overallPercent = round(
    workstreams.reduce(
      (sum, ws) => sum + ws.weightedContribution,
      0,
    ),
  )

  const foundationStories = stories.filter(
    (s) => s.workstream === "Platform / Engineering / Data",
  )
  const productStories = stories.filter(
    (s) =>
      s.workstream === "CRM / Intake" ||
      s.workstream === "Portal / Operations" ||
      s.workstream === "Public Property / Buyer Experience",
  )
  const brokerageStories = stories.filter((s) =>
    BROKERAGE_READY_STORY_IDS.has(s.id),
  )

  const summary: SummaryMetric[] = [
    {
      label: "Architecture / Foundation",
      percent: round(completionPercentFor(foundationStories)),
      detail: `${foundationStories.length} platform stories`,
    },
    {
      label: "Usable Product",
      percent: round(completionPercentFor(productStories)),
      detail: `${productStories.length} product stories`,
    },
    {
      label: "Brokerage-Ready",
      percent: round(completionPercentFor(brokerageStories)),
      detail: `${brokerageStories.length} brokerage-operational stories`,
    },
  ]

  return { stories, workstreams, summary, overallPercent }
}
