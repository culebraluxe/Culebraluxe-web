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

/** Workstream code stored on each story (editable secondary classification). */
export const WORKSTREAMS = [
  { code: 'PUBLIC', name: 'Public Website / Property Experience' },
  { code: 'CRM', name: 'CRM Foundation / Intake Core' },
  { code: 'PORTAL', name: 'Portal / Relationship Operations' },
  { code: 'TXN', name: 'Transaction / Deal Workflow' },
  { code: 'ADMIN', name: 'Admin / Data Management' },
  { code: 'AUTH', name: 'Auth / Security / User Access' },
  { code: 'CONTENT', name: 'Content / Marketing / Source Cleanup' },
  { code: 'HARDEN', name: 'Integrations / Operational Hardening' },
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

// ---------------------------------------------------------------------------
// Top-level operating DOMAINS (authoritative parent rollup authority).
//
// The primary board rollup groups by these five domains. Story status,
// execution state, and completion stay DISTINCT axes — status is the
// categorical story state, execution state is the latest Forge work-item
// state, completion is the stored 0..100.
// ---------------------------------------------------------------------------

export const STORY_DOMAINS = [
  'NEXUS',
  'MAIN',
  'OPPS',
  'SUPPORT',
  'TECH',
] as const

export type StoryDomain = (typeof STORY_DOMAINS)[number]

export const STORY_DOMAIN_LABELS: Record<StoryDomain, string> = {
  NEXUS: 'NEXUS — Real-Estate Operations',
  MAIN: 'MAIN — Public Site / Brand',
  OPPS: 'OPPS — Business Operations',
  SUPPORT: 'SUPPORT — Auth / Security / Reliability',
  TECH: 'TECH — Platform Engineering',
}

export function storyDomainName(domain: StoryDomain): string {
  return STORY_DOMAIN_LABELS[domain]
}

/** Canonical TECH subgroups — shown even when empty (0-story subgroups stay
 *  visible so the technical-system taxonomy is explicit). */
export const TECH_SUBGROUPS = [
  'ARCH',
  'FRAMEWORKS',
  'WORKFLOW ENGINE',
  'MQ MINI',
  'ALERTS',
] as const

/** Forge execution state — the latest coding-agent work-item state. */
export type StoryExecutionState = {
  workItemState: string | null
  latestRunResult: string | null
  latestRunAt: string | null
}

export const EXECUTION_ACTIVE_STATES = ['Running', 'Claimed'] as const
export const EXECUTION_ERROR_STATES = ['Error', 'Cancelled'] as const
export const EXECUTION_RUN_ERROR_RESULTS = ['Failed', 'Cancelled'] as const

function prefixOf(id: string): string {
  const part = id.split('-')[0]
  return part ? `${part}-` : id
}

/** MAIN: public-site / brand work (PX-, POLISH-, PLAT- prefixes or PUBLIC /
 *  CONTENT workstreams). */
const MAIN_PREFIXES = ['PX-', 'POLISH-', 'PLAT-']
const MAIN_WORKSTREAMS = ['PUBLIC', 'CONTENT']

/**
 * Deterministic DOMAIN classification. Precedence (documented, not silent):
 *   1. MAIN  — public-site / brand work (PX-/POLISH-/PLAT- prefix or
 *              PUBLIC/CONTENT workstream). The public site has no operating
 *              surface, so these are re-homed here.
 *   2. SUPPORT — AUTH-* prefix. The new model owns auth/security under
 *              SUPPORT; TECH has no AUTH subgroup, so the old TECH surface
 *              assignment is overridden.
 *   3. Otherwise the deliberate operating_surface classification is trusted
 *              (OPS → OPPS). A story with no operating_surface is UNCLASSIFIED
 *              and reported explicitly — never guessed.
 */
export function storyDomainOf(
  story: Pick<StoryRecord, 'id' | 'workstream' | 'operatingSurface'>,
): StoryDomain | 'UNCLASSIFIED' {
  const prefix = prefixOf(story.id)
  if (
    MAIN_PREFIXES.includes(prefix) ||
    MAIN_WORKSTREAMS.includes(story.workstream)
  ) {
    return 'MAIN'
  }
  if (prefix === 'AUTH-') return 'SUPPORT'
  switch (story.operatingSurface) {
    case 'NEXUS':
      return 'NEXUS'
    case 'OPS':
      return 'OPPS'
    case 'SUPPORT':
      return 'SUPPORT'
    case 'TECH':
      return 'TECH'
    default:
      return 'UNCLASSIFIED'
  }
}

/**
 * Deterministic SUBGROUP classification within a domain, driven by the story
 * prefix (primary) and workstream (secondary). Old prefixes remain visible as
 * secondary hints; they are no longer the board rollup authority.
 */
export function storySubgroupOf(
  story: Pick<StoryRecord, 'id' | 'workstream' | 'operatingSurface'>,
): string {
  const prefix = prefixOf(story.id)
  const w = story.workstream
  switch (storyDomainOf(story)) {
    case 'MAIN':
      if (prefix === 'PX-') return 'PX / Public'
      if (prefix === 'POLISH-') return 'Public Site / Polish'
      if (prefix === 'PLAT-') return 'Property / Platform Data'
      return 'Public / Content'
    case 'NEXUS':
      if (prefix === 'DOC-') return 'Forms / DOC'
      if (prefix === 'INTAKE-') return 'CRM / Intake'
      if (w === 'CRM') return 'CRM / Intake'
      if (w === 'PORTAL') return 'Portal / Relationship'
      if (w === 'TXN') return 'Deals / TXN'
      return 'Other'
    case 'OPPS':
      if (w === 'ADMIN') return 'Admin / Process'
      if (w === 'CRM') return 'CRM / Intake'
      if (w === 'PORTAL') return 'Portal / Ops'
      return 'OPS / Operations'
    case 'SUPPORT':
      if (prefix === 'AUTH-') return 'AUTH / Security'
      return 'Support / Ops'
    case 'TECH':
      if (prefix === 'ARCH-') return 'ARCH'
      if (prefix === 'FORGE-') return 'FRAMEWORKS'
      if (prefix === 'MQ-') return 'MQ MINI'
      if (prefix === 'ALERT-') return 'ALERTS'
      if (prefix === 'WORKFLOW-') return 'WORKFLOW ENGINE'
      if (prefix === 'CRM-') return 'WORKFLOW ENGINE'
      return 'FRAMEWORKS'
    default:
      return 'Other'
  }
}

export function isExecutionActive(state: string | null): boolean {
  return state != null && (EXECUTION_ACTIVE_STATES as readonly string[]).includes(state)
}

export function isExecutionError(
  exec: Pick<StoryExecutionState, 'workItemState' | 'latestRunResult'>,
): boolean {
  if (
    exec.workItemState != null &&
    (EXECUTION_ERROR_STATES as readonly string[]).includes(exec.workItemState)
  ) {
    return true
  }
  return (
    exec.latestRunResult != null &&
    (EXECUTION_RUN_ERROR_RESULTS as readonly string[]).includes(exec.latestRunResult)
  )
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
  /** Forge execution projection (latest work item + latest run). Null/absent
   *  when the board was built without execution data. */
  execution?: StoryExecutionState | null
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
}

export function defaultStoryBoardFilter(): StoryBoardFilter {
  return {
    q: '',
    workstream: 'all',
    status: 'all',
    priority: 'all',
    view: 'all',
    rollup: 'all',
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
    return storyMatchesQuery(story, filter.q)
  })
}

// ---------------------------------------------------------------------------
// Next Work selection (OPS-08) — bounded, deterministic work-selection
// projection ("Next 20 without building Jira").
//
// A pure projection over the authoritative stored stories: no assignments, no
// sprints, no new tables. The board stays the single source of truth; this
// derives the next bounded slice of actionable work from it.
//
// Eligibility (an actionable story):
//   - rollup = true — reference / parent rows (rollup=false) are never
//     selected as work; their children carry the weight
//   - status ∈ { Planned, Ready, In Progress, Partial } — Complete, Blocked,
//     Failed, Deferred and Hold are not actionable
//   - every board story referenced in `dependencies` is Complete. References
//     to stories the board does not know are unverifiable and never block.
//
// Ordering (deterministic): batch ascending (unbatched last) → priority rank
// (ENG-16 ladder) → planned start (earliest first, unplanned last) → id.
// The selection is capped (default 20, max 50); `truncated` reports when
// eligible work exists beyond the cap.
// ---------------------------------------------------------------------------

export const NEXT_WORK_DEFAULT_LIMIT = 20
export const NEXT_WORK_MAX_LIMIT = 50

/** ENG-16 priority ladder — Critical first; unknown priorities rank last. */
const PRIORITY_RANK: Record<string, number> = {
  Critical: 0,
  High: 1,
  'High-ish': 2,
  'Medium-High': 3,
  Medium: 4,
  Low: 5,
  Later: 6,
  'High-value polish': 7,
}

export function priorityRankOf(priority: string): number {
  return PRIORITY_RANK[priority] ?? 99
}

/** Statuses whose work is actionable now (eligible for Next Work). */
const ACTIONABLE_STATUSES: ReadonlySet<string> = new Set([
  'Planned',
  'Ready',
  'In Progress',
  'Partial',
])

export function isStoryActionable(story: StoryRecord): boolean {
  return story.rollup && ACTIONABLE_STATUSES.has(story.status)
}

/** Story-ID-like tokens inside free-text dependency notes (e.g. CRM-14B). */
const STORY_ID_TOKEN = /[A-Z]{2,}-\d+[A-Z]?/g

export function dependencyStoryIds(dependencies: string | null): string[] {
  if (!dependencies) return []
  const seen = new Set<string>()
  const ids: string[] = []
  for (const match of dependencies.toUpperCase().matchAll(STORY_ID_TOKEN)) {
    const id = match[0]
    if (!seen.has(id)) {
      seen.add(id)
      ids.push(id)
    }
  }
  return ids
}

export type NextWorkEntry = {
  story: StoryRecord
  /** 1-based position within the returned selection. */
  rank: number
}

export type NextWorkSelection = {
  entries: NextWorkEntry[]
  /** Actionable stories before the cap (dependency-blocked ones excluded). */
  totalEligible: number
  /** Actionable-status stories held back ONLY by unmet dependencies. */
  totalBlockedByDependency: number
  /** The applied cap (1..NEXT_WORK_MAX_LIMIT). */
  limit: number
  /** True when eligible stories exist beyond the returned cap. */
  truncated: boolean
}

function unmetDependenciesOf(
  story: StoryRecord,
  byId: Map<string, StoryRecord>,
): string[] {
  return dependencyStoryIds(story.dependencies).filter((id) => {
    const dep = byId.get(id)
    return dep !== undefined && dep.status !== 'Complete'
  })
}

export function selectNextWork(
  stories: StoryRecord[],
  opts?: { limit?: number },
): NextWorkSelection {
  const requested = opts?.limit
  const limit =
    typeof requested === 'number' && Number.isFinite(requested)
      ? Math.min(Math.max(Math.floor(requested), 1), NEXT_WORK_MAX_LIMIT)
      : NEXT_WORK_DEFAULT_LIMIT

  const byId = new Map<string, StoryRecord>(
    stories.map((s) => [s.id.toUpperCase(), s]),
  )

  let totalBlockedByDependency = 0
  const eligible: StoryRecord[] = []
  for (const story of stories) {
    if (!isStoryActionable(story)) continue
    if (unmetDependenciesOf(story, byId).length > 0) {
      totalBlockedByDependency += 1
      continue
    }
    eligible.push(story)
  }

  const ordered = [...eligible].sort((a, b) => {
    const aBatch = a.batch ?? Number.POSITIVE_INFINITY
    const bBatch = b.batch ?? Number.POSITIVE_INFINITY
    if (aBatch !== bBatch) return aBatch - bBatch
    const pr = priorityRankOf(a.priority) - priorityRankOf(b.priority)
    if (pr !== 0) return pr
    const aStart = a.plannedStartAt ?? ''
    const bStart = b.plannedStartAt ?? ''
    if (aStart !== bStart) {
      if (aStart === '') return 1
      if (bStart === '') return -1
      return aStart < bStart ? -1 : 1
    }
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
  })

  return {
    entries: ordered
      .slice(0, limit)
      .map((story, index) => ({ story, rank: index + 1 })),
    totalEligible: eligible.length,
    totalBlockedByDependency,
    limit,
    truncated: eligible.length > limit,
  }
}

// ---------------------------------------------------------------------------
// DOMAIN rollup model (authoritative parent rollup).
//
// Primary rollup groups by the five operating DOMAINS (NEXUS / MAIN / OPPS /
// SUPPORT / TECH); subgroup rollups sit beneath each parent domain.
//
// Axes stay DISTINCT:
//   DOMAIN       — storyDomainOf(story) (deterministic classifier above)
//   STORY STATUS — the categorical story status (counts only)
//   EXECUTION    — latest Forge work-item state / latest run (counts only)
//   COMPLETION   — the stored 0..100 completion (the only percentage input)
//
// Counts cover ALL stories in a domain (so domain totals always sum to the
// total story count). completion_percent is the AVG of the stored completion
// over ROLLUP-participating stories (rollup=false parents are stored and
// counted but carry no completion weight — their children do).
//
// net_net = simple mean of the five domain completion percentages (each
// operating domain is an equal authoritative pillar). No legacy weight table.
// ---------------------------------------------------------------------------

export type StoryDomainSubgroup = {
  subgroup: string
  storyCount: number
  completeCount: number
  inProgressPartialCount: number
  blockedFailedCount: number
  completionPercent: number
  stories: StoryRecord[]
}

export type StoryDomainRollup = {
  domain: StoryDomain
  label: string
  storyCount: number
  completeCount: number
  inProgressPartialCount: number
  blockedFailedCount: number
  /** Story status === Ready (authorized for coding-agent execution). */
  readyStoryCount: number
  /** Latest Forge work-item is Running/Claimed. */
  runningCount: number
  /** Latest work-item Error/Cancelled or latest run Failed/Cancelled. */
  errorCount: number
  completionPercent: number
  subgroups: StoryDomainSubgroup[]
  stories: StoryRecord[]
}

export type StoryBoardModel = {
  stories: StoryRecord[]
  domains: StoryDomainRollup[]
  /** Overall net-net completion, 0–100 (mean of the five domain percents). */
  netNet: number
  /** Executive summary counts over ALL stored stories. */
  totalStories: number
  totalComplete: number
  totalInProgressPartial: number
  totalBlockedFailed: number
  totalReady: number
  totalRunning: number
  totalError: number
  /** Stories with no determinable domain (reported explicitly, never guessed). */
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

function subgroupCompletion(stories: StoryRecord[]): number {
  const participating = stories.filter((s) => s.rollup)
  if (participating.length === 0) return 0
  return round(
    participating.reduce((sum, s) => sum + s.completion, 0) /
      participating.length,
  )
}

export function buildStoryBoardModel(stories: StoryRecord[]): StoryBoardModel {
  const domains: StoryDomainRollup[] = STORY_DOMAINS.map((domain) => {
    const domainStories = stories.filter((s) => storyDomainOf(s) === domain)

    const bySubgroup = new Map<string, StoryRecord[]>()
    for (const s of domainStories) {
      const subgroup = storySubgroupOf(s)
      const list = bySubgroup.get(subgroup) ?? []
      list.push(s)
      bySubgroup.set(subgroup, list)
    }
    const subgroups: StoryDomainSubgroup[] = [...bySubgroup]
      .map(([subgroup, group]) => ({
        subgroup,
        storyCount: group.length,
        completeCount: group.filter(
          (s) => statusBucket(s.status) === 'complete',
        ).length,
        inProgressPartialCount: group.filter((s) =>
          isInProgressPartial(s.status),
        ).length,
        blockedFailedCount: group.filter((s) => isBlockedFailed(s.status))
          .length,
        completionPercent: subgroupCompletion(group),
        stories: group,
      }))
      .sort((a, b) => b.storyCount - a.storyCount)

    // TECH shows its canonical five subgroups even when empty (MQ MINI /
    // ALERTS may have no stories yet — the taxonomy stays explicit).
    if (domain === 'TECH') {
      const order = new Map(
        (TECH_SUBGROUPS as readonly string[]).map((s, i) => [s, i]),
      )
      for (const s of TECH_SUBGROUPS) {
        if (!subgroups.some((x) => x.subgroup === s)) {
          subgroups.push({
            subgroup: s,
            storyCount: 0,
            completeCount: 0,
            inProgressPartialCount: 0,
            blockedFailedCount: 0,
            completionPercent: 0,
            stories: [],
          })
        }
      }
      subgroups.sort((a, b) => {
        const ia = order.get(a.subgroup) ?? Number.MAX_SAFE_INTEGER
        const ib = order.get(b.subgroup) ?? Number.MAX_SAFE_INTEGER
        return ia - ib
      })
    }

    return {
      domain,
      label: storyDomainName(domain),
      storyCount: domainStories.length,
      completeCount: domainStories.filter(
        (s) => statusBucket(s.status) === 'complete',
      ).length,
      inProgressPartialCount: domainStories.filter((s) =>
        isInProgressPartial(s.status),
      ).length,
      blockedFailedCount: domainStories.filter((s) => isBlockedFailed(s.status))
        .length,
      readyStoryCount: domainStories.filter((s) => s.status === 'Ready').length,
      runningCount: domainStories.filter((s) =>
        isExecutionActive(s.execution?.workItemState ?? null),
      ).length,
      errorCount: domainStories.filter((s) =>
        isExecutionError({
          workItemState: s.execution?.workItemState ?? null,
          latestRunResult: s.execution?.latestRunResult ?? null,
        }),
      ).length,
      completionPercent: subgroupCompletion(domainStories),
      subgroups,
      stories: domainStories,
    }
  })

  const present = domains.filter((d) => d.storyCount > 0)
  const netNet = round(
    present.length > 0
      ? present.reduce((sum, d) => sum + d.completionPercent, 0) /
          present.length
      : 0,
  )

  return {
    stories,
    domains,
    netNet,
    totalStories: stories.length,
    totalComplete: stories.filter((s) => s.status === 'Complete').length,
    totalInProgressPartial: stories.filter((s) =>
      isInProgressPartial(s.status),
    ).length,
    totalBlockedFailed: stories.filter((s) => isBlockedFailed(s.status)).length,
    totalReady: stories.filter((s) => s.status === 'Ready').length,
    totalRunning: stories.filter((s) =>
      isExecutionActive(s.execution?.workItemState ?? null),
    ).length,
    totalError: stories.filter((s) =>
      isExecutionError({
        workItemState: s.execution?.workItemState ?? null,
        latestRunResult: s.execution?.latestRunResult ?? null,
      }),
    ).length,
    unclassifiedCount: stories.filter(
      (s) => storyDomainOf(s) === 'UNCLASSIFIED',
    ).length,
  }
}
