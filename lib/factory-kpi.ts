// ---------------------------------------------------------------------------
// AI Software Factory KPI + Dispatch Model (ENG-17).
//
// Operations-research / decision-support layer over the REAL control-plane
// tables. This module is PURE and deterministic: every function takes persisted
// rows (plus an injectable `now`) and returns math, never state. No writes,
// no second queue/run model, no solver. V1 recommendations are deterministic
// rules over priority, dependency eligibility, capability and age; optimization
// is explicitly deferred until data volume justifies it.
//
// KPI categories (per the architect brief):
//   outcome   — delivery + quality (what the factory produced and how clean)
//   flow      — WIP / queue age / cycle+lead time / blocking (how work moves)
//   capacity  — busy / available / blocked / waiting + capability demand
//   decision  — ready eligible work, human gates, critical dependency pressure,
//               the recommended next dispatch
//
// Every KPI below is registered in docs/workflow/factory-kpi-contract.md with
// its formula, source tables, window, edge cases and intended decision. A KPI
// whose telemetry is absent is reported as `null` with `missingReason` — never
// as a fabricated zero (unavailable data must not look like "0").
//
// PIPPIN WATCH SOP: `assessFactoryHealth` implements the queue-health doctrine.
// The north star is protect throughput and detect systemic failure; a single
// isolated story failure is classified and then moved past, never treated as a
// factory-health emergency. Reporting language distinguishes STORY FAILED from
// FACTORY UNHEALTHY (see `health.summary` / `health.level`).
// ---------------------------------------------------------------------------

import type { AgentWorkItem } from '../db/agent-work'
import type { StoryboardStory, StoryRun } from '../db/storyboard'
import type { StoryBoardModel } from './storyboard-data'
import type { PipelineBlockRef } from './factory-command-center-data'

// ---------------------------------------------------------------------------
// Shared vocabulary
// ---------------------------------------------------------------------------

/** The four agent/slot states the acceptance criteria require. */
export type WorkerSlotKind = 'busy' | 'available' | 'blocked' | 'waiting'

export const WORKER_SLOT_KINDS: readonly WorkerSlotKind[] = [
  'busy',
  'waiting',
  'blocked',
  'available',
]

export type KpiCategory = 'outcome' | 'flow' | 'capacity' | 'decision'

export type FactoryHealthLevel = 'healthy' | 'watch' | 'escalate'

// ---------------------------------------------------------------------------
// Time helpers (pure; ISO-8601 in, hours out)
// ---------------------------------------------------------------------------

function parseIso(value: string | null | undefined): number | null {
  if (!value) return null
  const ms = Date.parse(value)
  return Number.isNaN(ms) ? null : ms
}

function hoursBetween(
  fromIso: string | null | undefined,
  toIso: string | null | undefined,
): number | null {
  const from = parseIso(fromIso)
  const to = parseIso(toIso)
  if (from === null || to === null) return null
  return Math.max(0, (to - from) / 3_600_000)
}

function median(values: number[]): number | null {
  if (values.length === 0) return null
  const sorted = [...values].sort((a, b) => a - b)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 0
    ? (sorted[mid - 1] + sorted[mid]) / 2
    : sorted[mid]
}

function round(value: number, digits = 1): number {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
}

function fmtHours(hours: number): number {
  return round(hours, 1)
}

// ---------------------------------------------------------------------------
// KPI result shape
// ---------------------------------------------------------------------------

export type KpiResult = {
  id: string
  label: string
  category: KpiCategory
  /** null = not computable (missing telemetry), never a fabricated zero. */
  value: number | null
  unit: string
  /** Present exactly when value is null. */
  missingReason: string | null
}

// ---------------------------------------------------------------------------
// Deterministic dispatch recommendation
// ---------------------------------------------------------------------------

export type DispatchRecommendation = {
  storyId: string
  title: string
  priority: string
  /** hours the story has been waiting ready for dispatch (age). */
  ageHours: number | null
  /** Capability demanded by the durable command envelope, if configured. */
  role: string | null
  modelProfile: string | null
  /** Why this pick wins — the deterministic rule trail. */
  reasons: string[]
}

export type FactoryDecisionSignals = {
  readyEligible: string[]
  readyEligibleCount: number
  /** readyEligible minus human-gated — claimable by an unattended worker. */
  autoDispatchEligible: string[]
  autoDispatchEligibleCount: number
  humanGateCount: number
  /** Stories waiting on a dependency that is itself Blocked/Failed (wedge). */
  criticalDependencyPressure: Array<{
    storyId: string
    blockedBy: PipelineBlockRef[]
  }>
  criticalDependencyCount: number
  recommended: DispatchRecommendation | null
}

// ---------------------------------------------------------------------------
// Factory health (PIPPIN WATCH SOP)
// ---------------------------------------------------------------------------

export type FactoryHealth = {
  level: FactoryHealthLevel
  /** STORY vs FACTORY language: distinguishes isolated failure from wedge. */
  summary: string
  reasons: string[]
  /** Terminal failed/error stories (isolated residue, classified once). */
  isolatedFailures: string[]
  /** True when the execution slot is free to take new work. */
  slotFree: boolean
  /** Active Claimed/Running item freshness (null when none). */
  activeHeartbeat: {
    storyId: string
    updatedAt: string
    ageMinutes: number
    stale: boolean
  } | null
  /** Eligible ready work sitting unclaimed while the slot is free (wedge). */
  unclaimedEligible: Array<{ storyId: string; ageHours: number | null }>
}

// ---------------------------------------------------------------------------
// Capability view (derived from the durable command envelope — no roster)
// ---------------------------------------------------------------------------

export type CapabilityState = {
  capability: string
  role: string | null
  modelProfile: string | null
  kind: WorkerSlotKind
  storyId: string | null
}

export type FactoryCapacityKpis = {
  busyCount: number
  waitingCount: number
  blockedCount: number
  availableCount: number
  /** One row per distinct capability with any state signal (demand or slot). */
  byCapability: CapabilityState[]
  /** Capability demand of currently auto-dispatch-eligible commands. */
  demandByCapability: Array<{
    capability: string
    role: string | null
    modelProfile: string | null
    count: number
  }>
}

// ---------------------------------------------------------------------------
// Full KPI model
// ---------------------------------------------------------------------------

export type FactoryKpis = {
  outcome: KpiResult[]
  flow: KpiResult[]
  capacity: FactoryCapacityKpis
  decision: FactoryDecisionSignals
  health: FactoryHealth
  /** Executive scope — rollup-participating vs parent/non-rollup rows. */
  scope: {
    rollupStoryCount: number
    parentStoryCount: number
    totalStoryCount: number
    /** rollup=false story ids — parents are tracked, never double-counted. */
    parentStoryIds: string[]
  }
  generatedAt: string
}

export type FactoryKpiInput = {
  stories: StoryboardStory[]
  workItems: AgentWorkItem[]
  runs: StoryRun[]
  /** The ENG-16 pipeline projection (dependency-eligible, gated, blocked). */
  pipeline: {
    readyWork: string[]
    gatedWork: string[]
    blockedWork: string[]
    nodes: Array<{
      storyId: string
      blockedBy: PipelineBlockRef[]
      gated: boolean
      gate: string | null
    }>
  }
  /** Four-state slot view (busy/waiting/blocked/available). */
  slots: Array<{
    kind: WorkerSlotKind
    workerId: string | null
    role: string | null
    modelProfile: string | null
    storyId: string | null
    workState: string | null
    since: string | null
  }>
  rollup: StoryBoardModel
  nowIso?: string
  /** Outcome/quality window (days); flow/capacity are instantaneous. */
  windowDays?: number
  /** Claimed/Running heartbeat silence threshold (minutes). */
  staleAfterMinutes?: number
  /** Eligible ready work unclaimed beyond this age while slot free → wedge. */
  schedulerWedgeMinutes?: number
}

// ---------------------------------------------------------------------------
// Pure derivation
// ---------------------------------------------------------------------------

function latestWorkByStory(items: AgentWorkItem[]): Map<string, AgentWorkItem> {
  const map = new Map<string, AgentWorkItem>()
  for (const item of items) {
    const current = map.get(item.storyId)
    if (!current || item.queuedAt > current.queuedAt) map.set(item.storyId, item)
  }
  return map
}

/** A story's "became Ready" timestamp: the Ready work item's queued_at when
 *  present (the dispatch trigger creates it on Ready), else the last story
 *  mutation as a documented approximation. */
function readySinceOf(
  story: StoryboardStory,
  latestWork: AgentWorkItem | null,
): string | null {
  if (latestWork) return latestWork.queuedAt
  return story.updatedAt || null
}

export function buildFactoryKpis(input: FactoryKpiInput): FactoryKpis {
  const nowMs = parseIso(input.nowIso ?? new Date().toISOString()) ?? Date.now()
  const nowIso = new Date(nowMs).toISOString()
  const windowDays = input.windowDays ?? 30
  const windowMs = windowDays * 86_400_000
  const staleAfterMinutes = input.staleAfterMinutes ?? 60
  const schedulerWedgeMinutes = input.schedulerWedgeMinutes ?? 60
  const windowStartMs = nowMs - windowMs

  const rollupStories = input.stories.filter((s) => s.rollup)
  const parentStories = input.stories.filter((s) => !s.rollup)
  const latestWork = latestWorkByStory(input.workItems)

  // -------------------------------------------------------------------------
  // Outcome KPIs — delivery + quality
  // -------------------------------------------------------------------------
  const outcome: KpiResult[] = []

  const completedInWindow = rollupStories.filter(
    (s) =>
      s.status === 'Complete' &&
      (parseIso(s.completedAt) ?? -Infinity) >= windowStartMs,
  )
  outcome.push({
    id: 'net-net completion',
    label: 'Net-net completion',
    category: 'outcome',
    value: round(input.rollup.netNet, 1),
    unit: '%',
    missingReason: null,
  })

  const rollupComplete = rollupStories.filter(
    (s) => s.status === 'Complete',
  ).length
  outcome.push({
    id: 'completion rate',
    label: 'Completion rate',
    category: 'outcome',
    value:
      rollupStories.length > 0
        ? round((rollupComplete / rollupStories.length) * 100, 1)
        : null,
    unit: '%',
    missingReason:
      rollupStories.length === 0
        ? 'no rollup-participating stories on the board'
        : null,
  })

  outcome.push({
    id: 'throughput (window)',
    label: 'Stories completed',
    category: 'outcome',
    value: completedInWindow.length,
    unit: `count / ${windowDays}d`,
    missingReason: null,
  })

  // Cycle time = active execution window (actual_start_at -> completed_at).
  const cycleTimes = completedInWindow
    .map((s) => hoursBetween(s.actualStartAt, s.completedAt))
    .filter((h): h is number => h !== null)
  const cycleMedian = median(cycleTimes)
  outcome.push({
    id: 'cycle time',
    label: 'Cycle time (median)',
    category: 'outcome',
    value: cycleMedian === null ? null : fmtHours(cycleMedian),
    unit: 'hours',
    missingReason:
      completedInWindow.length === 0
        ? 'no stories completed in window'
        : cycleMedian === null
          ? 'missing telemetry: actual_start_at is null on completed stories (work started before run telemetry existed)'
          : null,
  })

  // Lead time = time in the system (created_at -> completed_at).
  const leadTimes = completedInWindow
    .map((s) => hoursBetween(s.createdAt, s.completedAt))
    .filter((h): h is number => h !== null)
  const leadMedian = median(leadTimes)
  outcome.push({
    id: 'lead time',
    label: 'Lead time (median)',
    category: 'outcome',
    value: leadMedian === null ? null : fmtHours(leadMedian),
    unit: 'hours',
    missingReason:
      completedInWindow.length === 0
        ? 'no stories completed in window'
        : leadMedian === null
          ? 'missing telemetry: created_at is null on completed stories'
          : null,
  })

  // Quality — terminal runs in the window (a run is a persisted execution
  // OUTCOME; Planned/In Progress never appear on storyboard_story_run).
  const windowRuns = input.runs.filter(
    (r) => (parseIso(r.startedAt) ?? -Infinity) >= windowStartMs,
  )
  const terminalRuns = windowRuns.filter((r) => r.resultStatus !== null)
  const completeRuns = terminalRuns.filter((r) => r.resultStatus === 'Complete')
  const failedRuns = terminalRuns.filter((r) => r.resultStatus === 'Failed')
  outcome.push({
    id: 'run pass rate',
    label: 'Run pass rate',
    category: 'outcome',
    value:
      terminalRuns.length > 0
        ? round((completeRuns.length / terminalRuns.length) * 100, 1)
        : null,
    unit: '% of terminal runs',
    missingReason:
      terminalRuns.length === 0 ? 'no terminal runs in window' : null,
  })
  outcome.push({
    id: 'run failure rate',
    label: 'Run failure rate',
    category: 'outcome',
    value:
      terminalRuns.length > 0
        ? round((failedRuns.length / terminalRuns.length) * 100, 1)
        : null,
    unit: '% of terminal runs',
    missingReason:
      terminalRuns.length === 0 ? 'no terminal runs in window' : null,
  })

  // Retry rate — durable commands that were claimed more than once.
  const everClaimed = input.workItems.filter((w) => w.attempts >= 1)
  const retried = input.workItems.filter((w) => w.attempts >= 2)
  outcome.push({
    id: 'retry rate',
    label: 'Command retry rate',
    category: 'outcome',
    value:
      everClaimed.length > 0
        ? round((retried.length / everClaimed.length) * 100, 1)
        : null,
    unit: '% of claimed commands',
    missingReason:
      everClaimed.length === 0 ? 'no command has ever been claimed' : null,
  })

  // -------------------------------------------------------------------------
  // Flow KPIs — WIP / queue / age / blocking (instantaneous)
  // -------------------------------------------------------------------------
  const flow: KpiResult[] = []

  const wip = rollupStories.filter(
    (s) => s.status === 'In Progress' || s.status === 'Partial',
  )
  flow.push({
    id: 'wip',
    label: 'Work in progress',
    category: 'flow',
    value: wip.length,
    unit: 'stories',
    missingReason: null,
  })

  const readyStories = input.stories.filter((s) => s.status === 'Ready')
  flow.push({
    id: 'ready (authorized)',
    label: 'Ready (authorized)',
    category: 'flow',
    value: readyStories.length,
    unit: 'stories',
    missingReason: null,
  })

  const readyEligibleIds = new Set(input.pipeline.readyWork)
  const readyEligibleStories = input.stories.filter((s) =>
    readyEligibleIds.has(s.id),
  )
  const readyWaitingOnDeps = readyStories.filter(
    (s) => !readyEligibleIds.has(s.id),
  )
  flow.push({
    id: 'ready eligible',
    label: 'Ready & dependency-eligible',
    category: 'flow',
    value: readyEligibleStories.length,
    unit: 'stories',
    missingReason: null,
  })
  flow.push({
    id: 'ready waiting on deps',
    label: 'Ready waiting on deps',
    category: 'flow',
    value: readyWaitingOnDeps.length,
    unit: 'stories',
    missingReason: null,
  })

  // Queue age — oldest dependency-eligible Ready story.
  const eligibleAges = readyEligibleStories
    .map((s) =>
      hoursBetween(readySinceOf(s, latestWork.get(s.id) ?? null), nowIso),
    )
    .filter((h): h is number => h !== null)
  const maxQueueAge = eligibleAges.length > 0 ? Math.max(...eligibleAges) : null
  flow.push({
    id: 'queue age (oldest eligible)',
    label: 'Oldest eligible ready age',
    category: 'flow',
    value: maxQueueAge === null ? null : fmtHours(maxQueueAge),
    unit: 'hours',
    missingReason:
      eligibleAges.length === 0
        ? 'no dependency-eligible ready work queued'
        : null,
  })

  // Blocking — hard blocked (Blocked/Failed) plus waiting-on-deps.
  const blockedStoryIds = new Set(input.pipeline.blockedWork)
  const blockedStories = input.stories.filter((s) => blockedStoryIds.has(s.id))
  flow.push({
    id: 'blocked work',
    label: 'Blocked / waiting work',
    category: 'flow',
    value: blockedStories.length,
    unit: 'stories',
    missingReason: null,
  })

  // Blocked age proxy: last story mutation (documented approximation).
  const blockedAges = blockedStories
    .map((s) => hoursBetween(s.updatedAt, nowIso))
    .filter((h): h is number => h !== null)
  flow.push({
    id: 'blocked age (max)',
    label: 'Longest blocked age',
    category: 'flow',
    value: blockedAges.length > 0 ? fmtHours(Math.max(...blockedAges)) : null,
    unit: 'hours',
    missingReason: blockedStories.length === 0 ? 'no blocked work' : null,
  })

  // Stale active slot — Claimed/Running with silent heartbeat.
  const staleActive = input.workItems.filter((w) => {
    if (w.state !== 'Claimed' && w.state !== 'Running') return false
    const updated = parseIso(w.updatedAt)
    if (updated === null) return true
    return nowMs - updated > staleAfterMinutes * 60_000
  })
  flow.push({
    id: 'stale active',
    label: 'Stale active commands',
    category: 'flow',
    value: staleActive.length,
    unit: 'commands',
    missingReason: null,
  })

  // -------------------------------------------------------------------------
  // Capacity KPIs — four-state slot + capability
  // -------------------------------------------------------------------------
  const capacity = buildCapacityKpis(input, readyEligibleStories)

  // -------------------------------------------------------------------------
  // Decision signals
  // -------------------------------------------------------------------------
  const gatedIds = new Set(input.pipeline.gatedWork)
  const autoDispatchEligible = input.pipeline.readyWork.filter(
    (id) => !gatedIds.has(id),
  )

  // Critical dependency pressure: waiting on a dependency that is itself
  // Blocked/Failed — a wedge that will not self-resolve.
  const criticalDependencyPressure: FactoryDecisionSignals['criticalDependencyPressure'] =
    []
  for (const node of input.pipeline.nodes) {
    const wedge = node.blockedBy.filter(
      (b) => b.status === 'Blocked' || b.status === 'Failed',
    )
    if (wedge.length > 0) {
      criticalDependencyPressure.push({ storyId: node.storyId, blockedBy: wedge })
    }
  }

  const decision: FactoryDecisionSignals = {
    readyEligible: [...input.pipeline.readyWork],
    readyEligibleCount: input.pipeline.readyWork.length,
    autoDispatchEligible,
    autoDispatchEligibleCount: autoDispatchEligible.length,
    humanGateCount: input.pipeline.gatedWork.length,
    criticalDependencyPressure,
    criticalDependencyCount: criticalDependencyPressure.length,
    recommended: recommendNextDispatch(input, latestWork),
  }

  // -------------------------------------------------------------------------
  // Factory health (PIPPIN WATCH SOP)
  // -------------------------------------------------------------------------
  const health = assessFactoryHealth({
    nowMs,
    nowIso,
    staleAfterMinutes,
    schedulerWedgeMinutes,
    workItems: input.workItems,
    pipeline: input.pipeline,
    eligibleStories: readyEligibleStories,
    latestWork,
    failedRuns,
  })

  return {
    outcome,
    flow,
    capacity,
    decision,
    health,
    scope: {
      rollupStoryCount: rollupStories.length,
      parentStoryCount: parentStories.length,
      totalStoryCount: input.stories.length,
      parentStoryIds: parentStories.map((s) => s.id),
    },
    generatedAt: nowIso,
  }
}

// ---------------------------------------------------------------------------
// Capacity KPIs
// ---------------------------------------------------------------------------

function capabilityKey(
  role: string | null,
  modelProfile: string | null,
): string {
  return `${role ?? '?'}/${modelProfile ?? '?'}`
}

function buildCapacityKpis(
  input: FactoryKpiInput,
  eligibleStories: StoryboardStory[],
): FactoryCapacityKpis {
  const slots = input.slots
  const busy = slots.filter((s) => s.kind === 'busy')
  const waiting = slots.filter((s) => s.kind === 'waiting')
  const blocked = slots.filter((s) => s.kind === 'blocked')
  const available = slots.filter((s) => s.kind === 'available')

  // One row per distinct capability with a state signal: the active slot's
  // capability (busy/waiting), each blocked command's capability, and the
  // available capability when the slot is free (capability = unassigned).
  const byCapabilityMap = new Map<string, CapabilityState>()
  for (const slot of [...busy, ...waiting, ...blocked, ...available]) {
    const key = capabilityKey(slot.role, slot.modelProfile)
    if (!byCapabilityMap.has(key)) {
      byCapabilityMap.set(key, {
        capability: key,
        role: slot.role,
        modelProfile: slot.modelProfile,
        kind: slot.kind,
        storyId: slot.storyId,
      })
    }
  }

  // Demand: capability required by each auto-dispatch-eligible command.
  const latestWork = latestWorkByStory(input.workItems)
  const demandMap = new Map<
    string,
    { capability: string; role: string | null; modelProfile: string | null; count: number }
  >()
  for (const story of eligibleStories) {
    const work = latestWork.get(story.id)
    const key = capabilityKey(work?.role ?? null, work?.modelProfile ?? null)
    const entry =
      demandMap.get(key) ?? {
        capability: key,
        role: work?.role ?? null,
        modelProfile: work?.modelProfile ?? null,
        count: 0,
      }
    entry.count += 1
    demandMap.set(key, entry)
  }

  return {
    busyCount: busy.length,
    waitingCount: waiting.length,
    blockedCount: blocked.length,
    availableCount: available.length,
    byCapability: [...byCapabilityMap.values()],
    demandByCapability: [...demandMap.values()].sort(
      (a, b) => b.count - a.count || a.capability.localeCompare(b.capability),
    ),
  }
}

// ---------------------------------------------------------------------------
// Deterministic next-dispatch recommendation (V1 — rules, not a solver)
// ---------------------------------------------------------------------------

export function recommendNextDispatch(
  input: Pick<FactoryKpiInput, 'stories' | 'workItems' | 'pipeline' | 'nowIso'>,
  latestWork?: Map<string, AgentWorkItem>,
): DispatchRecommendation | null {
  const workMap = latestWork ?? latestWorkByStory(input.workItems)
  const gatedIds = new Set(input.pipeline.gatedWork)
  const candidates = input.pipeline.readyWork.filter(
    (id) => !gatedIds.has(id),
  )
  if (candidates.length === 0) return null

  const now = input.nowIso ?? new Date().toISOString()
  const byId = new Map(input.stories.map((s) => [s.id, s]))
  const ranked = candidates
    .map((id) => {
      const story = byId.get(id)
      if (!story) return null
      const work = workMap.get(id) ?? null
      return {
        story,
        work,
        ageHours: hoursBetween(readySinceOf(story, work), now),
      }
    })
    .filter((c): c is NonNullable<typeof c> => c !== null)
    .sort((a, b) => {
      const aRank = priorityRankOf(a.story.priority)
      const bRank = priorityRankOf(b.story.priority)
      if (aRank !== bRank) return aRank - bRank
      // Oldest ready first — age breaks priority ties.
      const aAge = a.ageHours ?? -1
      const bAge = b.ageHours ?? -1
      if (aAge !== bAge) return bAge - aAge
      return a.story.id.localeCompare(b.story.id)
    })

  const top = ranked[0]
  if (!top) return null

  const { story, work, ageHours } = top
  const reasons: string[] = [
    `${story.priority} priority`,
    'dependency-satisfied (Ready and unblocked)',
  ]
  if (ageHours !== null) {
    reasons.push(`oldest eligible ready (aged ${fmtHours(ageHours)}h)`)
  } else {
    reasons.push('age unavailable (missing queued timestamp)')
  }
  if (work?.role && work?.modelProfile) {
    reasons.push(`capability ready: ${work.role} / ${work.modelProfile}`)
  } else {
    reasons.push('command envelope not configured (role/model profile missing)')
  }

  return {
    storyId: story.id,
    title: story.title,
    priority: story.priority,
    ageHours: ageHours === null ? null : fmtHours(ageHours),
    role: work?.role ?? null,
    modelProfile: work?.modelProfile ?? null,
    reasons,
  }
}

// ---------------------------------------------------------------------------
// PIPPIN WATCH SOP — factory health assessment
// ---------------------------------------------------------------------------

type HealthInput = {
  nowMs: number
  nowIso: string
  staleAfterMinutes: number
  schedulerWedgeMinutes: number
  workItems: AgentWorkItem[]
  pipeline: FactoryKpiInput['pipeline']
  eligibleStories: StoryboardStory[]
  latestWork: Map<string, AgentWorkItem>
  failedRuns: StoryRun[]
}

export function assessFactoryHealth(input: HealthInput): FactoryHealth {
  const reasons: string[] = []
  const isolatedFailures: string[] = []

  const active = input.workItems.filter(
    (w) => w.state === 'Claimed' || w.state === 'Running',
  )
  const activeHeartbeat = (() => {
    if (active.length === 0) return null
    const item = active[0]
    const updated = parseIso(item.updatedAt)
    const ageMinutes =
      updated === null ? null : Math.round((input.nowMs - updated) / 60_000)
    const stale =
      updated === null || ageMinutes === null
        ? true
        : ageMinutes > input.staleAfterMinutes
    return {
      storyId: item.storyId,
      updatedAt: item.updatedAt,
      ageMinutes: ageMinutes ?? -1,
      stale,
    }
  })()

  const slotFree = active.length === 0
  const pending = input.workItems.filter((w) => w.state === 'Paused')
  // A Paused item holds the slot — treat it as not-free for replenishment.
  const replenishmentFree = slotFree && pending.length === 0

  // 1. Stale active slot (heartbeat silence) — escalate (wedge).
  const staleActive = active.filter((w) => {
    const updated = parseIso(w.updatedAt)
    return (
      updated === null ||
      input.nowMs - updated > input.staleAfterMinutes * 60_000
    )
  })
  if (staleActive.length > 0) {
    reasons.push(
      `ESCALATE: stale active slot — ${staleActive
        .map((w) => `${w.storyId} (${w.state})`)
        .join(', ')} with no heartbeat within ${input.staleAfterMinutes} minutes. Recover the slot before trusting the queue.`,
    )
  }

  // 2. Scheduler not claiming eligible Ready work (slot free + old eligible).
  const eligibleIds = new Set(input.pipeline.readyWork)
  const unclaimedEligible = input.eligibleStories
    .filter((s) => eligibleIds.has(s.id))
    .map((s) => ({
      storyId: s.id,
      ageHours: hoursBetween(
        readySinceOf(s, input.latestWork.get(s.id) ?? null),
        input.nowIso,
      ),
    }))
    .filter(
      (u) =>
        u.ageHours !== null && u.ageHours * 60 > input.schedulerWedgeMinutes,
    )

  if (replenishmentFree && unclaimedEligible.length > 0) {
    reasons.push(
      `ESCALATE: scheduler wedge — slot is free but eligible work is unclaimed: ${unclaimedEligible
        .map((u) => `${u.storyId} (${fmtHours(u.ageHours!)}h)`)
        .join(', ')}. The scheduler is not claiming dependency-ready work.`,
    )
  }

  // 3. Repeated failures on the same story (watch).
  const failuresByStory = new Map<string, number>()
  for (const w of input.workItems) {
    if (w.state === 'Error' || w.state === 'Cancelled') {
      failuresByStory.set(w.storyId, (failuresByStory.get(w.storyId) ?? 0) + 1)
    }
  }
  for (const [storyId, count] of failuresByStory) {
    if (count >= 2) {
      reasons.push(
        `WATCH: repeated failure — ${storyId} failed ${count} times. Isolated unless the pattern repeats systemically.`,
      )
    }
  }

  // 4. Multiple/systemic failures in the window (watch).
  const failedStoryIds = new Set(input.failedRuns.map((r) => r.storyId))
  if (failedStoryIds.size >= 3) {
    reasons.push(
      `WATCH: ${failedStoryIds.size} distinct stories failed in the outcome window — systemic pattern worth review before replenishing aggressively.`,
    )
  }

  // 5. Historical residue — terminal failures that are NOT active are
  //    classified once and suppressed from alarm language.
  for (const w of input.workItems) {
    if (w.state === 'Error' || w.state === 'Cancelled') {
      isolatedFailures.push(w.storyId)
    }
  }

  // Severity: escalate beats watch beats healthy.
  let level: FactoryHealthLevel = 'healthy'
  if (
    staleActive.length > 0 ||
    (replenishmentFree && unclaimedEligible.length > 0)
  ) {
    level = 'escalate'
  } else if (reasons.some((r) => r.startsWith('WATCH'))) {
    level = 'watch'
  }

  const uniqueFailures = [...new Set(isolatedFailures)]
  let summary: string
  if (level === 'escalate') {
    summary = `FACTORY UNHEALTHY — ${reasons.join(' ')}`
  } else if (level === 'watch') {
    summary = `Forge watch — ${reasons.join(' ')}`
  } else if (activeHeartbeat && !activeHeartbeat.stale) {
    summary = `Forge healthy; ${activeHeartbeat.storyId} Running with fresh heartbeat.`
  } else if (replenishmentFree && eligibleIds.size > 0) {
    summary = `Forge healthy; slot free with ${eligibleIds.size} dependency-ready command${
      eligibleIds.size > 1 ? 's' : ''
    } to claim.`
  } else if (slotFree) {
    summary = 'Forge healthy; slot free, no eligible ready work queued.'
  } else {
    summary = 'Forge healthy; slot engaged.'
  }

  // STORY vs FACTORY language: a single failed story whose slot was released
  // and later work progressed is NOT a factory emergency.
  if (level === 'healthy' && uniqueFailures.length > 0) {
    summary += ` ${uniqueFailures.slice(0, 5).join(', ')} ${
      uniqueFailures.length === 1
        ? 'is an isolated failed story'
        : 'are isolated failed stories'
    }; slot released, not blocking throughput.`
  }

  return {
    level,
    summary,
    reasons,
    isolatedFailures: uniqueFailures,
    slotFree: replenishmentFree,
    activeHeartbeat,
    unclaimedEligible,
  }
}

// ---------------------------------------------------------------------------
// Priority rank (shared with ENG-16)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Convenience: KPI lookups used by the console UI
// ---------------------------------------------------------------------------

export function kpiValue(
  kpis: KpiResult[],
  id: string,
): { value: number | null; missingReason: string | null } {
  const kpi = kpis.find((k) => k.id === id)
  return { value: kpi?.value ?? null, missingReason: kpi?.missingReason ?? null }
}
