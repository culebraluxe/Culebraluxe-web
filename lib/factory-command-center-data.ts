// ---------------------------------------------------------------------------
// AI Software Factory Command Center read model (ENG-16).
//
// The PARENT operating console over the REAL control-plane tables only:
//   storyboard_story       (authoritative backlog/spec)
//   agent_work_item        (durable command/queue)
//   storyboard_story_run   (execution evidence)
//   process_instances/…    (engine workflow instances, read-only, via
//                           workflow_app/read-service — never written here)
//
// Three information layers on one screen, all DERIVED at read time:
//   1. Executive rollup  — how the factory is doing (buildStoryBoardModel)
//   2. Agent dispatch/capacity — who is assigned/idle/blocked and what becomes
//      eligible next (derived from agent_work_item, never a second roster)
//   3. Dependency-aware factory pipeline — what work is flowing and what
//      artifacts/results each worker is producing (nodes + dependency edges
//      parsed from the canonical storyboard_story.dependencies text).
//
// No second queue/run/state model is introduced: every field below is a read
// projection over the existing repositories. Dependency edges are derived from
// the free-text `dependencies` field with a tolerant parser; a reference that
// matches a KNOWN board story is verifiable (satisfied = that story is
// Complete), a reference that matches no board story is EXTERNAL/unverifiable
// and never blocks eligibility (prose references must not false-block).
// ---------------------------------------------------------------------------

import {
  buildStoryBoardModel,
  type StoryBoardModel,
} from './storyboard-data'
import {
  listAgentWorkItems,
  type AgentWorkItem,
  type AgentWorkState,
} from '../db/agent-work'
import {
  listStoryboardStories,
  listStoryboardRuns,
  type StoryboardStory,
  type StoryRun,
} from '../db/storyboard'
import {
  getWorkflowSummaries,
  type WorkflowSummary,
} from '../workflow_app/read-service'

// ---------------------------------------------------------------------------
// Dependency parsing (tolerant, case-insensitive, deduplicated)
// ---------------------------------------------------------------------------

const DEP_REF_RE = /\b([A-Z][A-Z0-9]{0,7}-[A-Z0-9]{1,6})\b/gi

/**
 * Extract story references (e.g. ENG-15, S-018, CRM-14A) from the canonical
 * free-text `dependencies` field. Tolerant by design: slash-separated lists,
 * parentheticals, "S-020, S-022 and S-026" prose, and "through" ranges all
 * degrade to the individual tokens that ARE present. Order preserved, first
 * occurrence wins.
 */
export function extractDependencyRefs(
  text: string | null | undefined,
): string[] {
  if (!text) return []
  const seen = new Set<string>()
  const out: string[] = []
  DEP_REF_RE.lastIndex = 0
  let m: RegExpExecArray | null
  while ((m = DEP_REF_RE.exec(text)) !== null) {
    const ref = m[1].toUpperCase()
    if (!seen.has(ref)) {
      seen.add(ref)
      out.push(ref)
    }
  }
  return out
}

// ---------------------------------------------------------------------------
// Pipeline types
// ---------------------------------------------------------------------------

export type PipelineStage =
  | 'complete'
  | 'active'
  | 'ready'
  | 'blocked'
  | 'planned'
  | 'hold'

export type PipelineBlockRef = {
  storyId: string
  status: string
  completion: number
}

export type FactoryPipelineNode = {
  storyId: string
  title: string
  workstream: string
  priority: string
  status: string
  completion: number
  /** Assigned worker (claimed_by on the latest work item), if any. */
  worker: string | null
  role: string | null
  workState: AgentWorkState | null
  /** Latest run evidence — concise work product state. */
  runResult: string | null
  runCompletion: number | null
  testsSummary: string | null
  commitHash: string | null
  latestStep: string | null
  dependencyRefs: string[]
  /** True when the story itself is in a hard blocked/failed state. */
  blocked: boolean
  /** True when a Ready story is waiting on an uncompleted dependency. */
  waitingOnDeps: boolean
  /** Human-readable explanation of why this node is blocked/unblocked. */
  blockedReason: string | null
  blockedBy: PipelineBlockRef[]
  /** Ready AND every known dependency is Complete → eligible to execute. */
  ready: boolean
  /** Latest work item carries an execution policy that needs a human. */
  gated: boolean
  gate: 'Human Gate' | 'Manual Only' | 'Daytime Only' | null
  stage: PipelineStage
}

export type FactoryDependencyEdge = {
  from: string
  /** Board story id when the ref resolves; null for external/unverifiable. */
  to: string | null
  external: boolean
  toStatus: string | null
  /** false when a known dependency is not Complete; null when unverifiable. */
  satisfied: boolean | null
}

export type FactoryPipeline = {
  nodes: FactoryPipelineNode[]
  edges: FactoryDependencyEdge[]
  /** Ready + deps satisfied story ids, priority order. */
  readyWork: string[]
  /** Story ids whose latest command carries a human execution gate. */
  gatedWork: string[]
  /** Story ids in a hard blocked/failed or waiting-on-dependency state. */
  blockedWork: string[]
}

export type WorkerCapacityEntry = {
  workerId: string | null
  role: string | null
  kind: 'assigned' | 'idle' | 'blocked'
  storyId: string | null
  workState: AgentWorkState | null
  since: string | null
}

export type FactoryCapacity = {
  workers: WorkerCapacityEntry[]
  assignedCount: number
  idleCount: number
  blockedCount: number
  nextEligible: Array<{
    storyId: string
    title: string
    priority: string
    completion: number
  }>
}

export type FactorySnapshot = {
  ready: boolean
  rollup: StoryBoardModel | null
  pipeline: FactoryPipeline
  capacity: FactoryCapacity
  /** Engine workflow instances (deep workflow cockpit drill-down targets). */
  workflowCockpits: WorkflowSummary[]
  humanGateCount: number
}

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

function priorityRank(priority: string): number {
  return PRIORITY_RANK[priority] ?? 99
}

const HUMAN_GATES: ReadonlySet<string> = new Set([
  'Human Gate',
  'Manual Only',
  'Daytime Only',
])

function latestStepOf(run: StoryRun | null): string | null {
  if (!run?.notes) return null
  const lines = run.notes
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
  if (lines.length === 0) return null
  return lines[lines.length - 1].slice(0, 160)
}

function stageOf(
  status: string,
  blocked: boolean,
  waitingOnDeps: boolean,
  ready: boolean,
): PipelineStage {
  switch (status) {
    case 'Complete':
      return 'complete'
    case 'In Progress':
    case 'Partial':
      return 'active'
    case 'Ready':
      if (blocked || waitingOnDeps) return 'blocked'
      return 'ready'
    case 'Blocked':
    case 'Failed':
      return 'blocked'
    case 'Deferred':
    case 'Hold':
      return 'hold'
    default:
      return 'planned'
  }
}

// ---------------------------------------------------------------------------
// Pipeline derivation (pure — unit-tested with in-memory fixtures)
// ---------------------------------------------------------------------------

export function buildFactoryPipeline(
  stories: StoryboardStory[],
  workItems: AgentWorkItem[],
  latestRunByStory: Map<string, StoryRun>,
): FactoryPipeline {
  const storyById = new Map(stories.map((s) => [s.id, s]))

  // Latest (newest queued) work item per story — the "current command".
  const latestWorkByStory = new Map<string, AgentWorkItem>()
  for (const item of workItems) {
    const current = latestWorkByStory.get(item.storyId)
    if (!current || item.queuedAt > current.queuedAt) {
      latestWorkByStory.set(item.storyId, item)
    }
  }

  const nodes: FactoryPipelineNode[] = []
  const edges: FactoryDependencyEdge[] = []

  for (const story of stories) {
    const refs = extractDependencyRefs(story.dependencies)
    const work = latestWorkByStory.get(story.id) ?? null
    const run = latestRunByStory.get(story.id) ?? null

    const blockedBy: PipelineBlockRef[] = []
    let waitingOnDeps = false
    for (const ref of refs) {
      const dep = storyById.get(ref)
      if (dep && dep.status !== 'Complete') {
        blockedBy.push({
          storyId: dep.id,
          status: dep.status,
          completion: dep.completion,
        })
        if (story.status === 'Ready') waitingOnDeps = true
      }
    }

    const workBlocked = work?.state === 'Error' || work?.state === 'Cancelled'
    const statusBlocked = story.status === 'Blocked' || story.status === 'Failed'
    const hardBlocked = workBlocked || statusBlocked
    const ready =
      story.status === 'Ready' && blockedBy.length === 0

    let blockedReason: string | null = null
    // The work-item outcome is the most actionable reason when present
    // (failAgentWork pairs work Error with story Failed — do not let the
    // coarse status shadow the specific failure).
    if (workBlocked) {
      blockedReason =
        work?.state === 'Error'
          ? `work item failed${work?.errorText ? `: ${work.errorText}` : ''}`
          : 'run cancelled by operator'
    } else if (statusBlocked) {
      blockedReason = `story status is ${story.status}`
    } else if (waitingOnDeps) {
      blockedReason = `waiting on ${blockedBy
        .map((b) => `${b.storyId} (${b.status})`)
        .join(', ')}`
    }

    const gate = work
      ? (HUMAN_GATES.has(work.executionPolicy)
          ? (work.executionPolicy as FactoryPipelineNode['gate'])
          : null)
      : null

    nodes.push({
      storyId: story.id,
      title: story.title,
      workstream: story.workstream,
      priority: story.priority,
      status: story.status,
      completion: story.completion,
      worker: work?.claimedBy ?? null,
      role: work?.role ?? null,
      workState: work?.state ?? null,
      runResult: run?.resultStatus ?? null,
      runCompletion: run?.completion ?? null,
      testsSummary: run?.testsSummary ?? null,
      commitHash: run?.commitHash ?? null,
      latestStep: latestStepOf(run),
      dependencyRefs: refs,
      blocked: hardBlocked,
      waitingOnDeps,
      blockedReason,
      blockedBy,
      ready,
      gated: gate !== null,
      gate,
      stage: stageOf(story.status, hardBlocked, waitingOnDeps, ready),
    })
  }

  for (const node of nodes) {
    for (const ref of node.dependencyRefs) {
      const dep = storyById.get(ref) ?? null
      edges.push({
        from: node.storyId,
        to: dep?.id ?? null,
        external: dep === null,
        toStatus: dep?.status ?? null,
        satisfied: dep === null ? null : dep.status === 'Complete',
      })
    }
  }

  const byStage = (stage: PipelineStage) =>
    nodes
      .filter((n) => n.stage === stage)
      .sort(
        (a, b) =>
          priorityRank(a.priority) - priorityRank(b.priority) ||
          a.storyId.localeCompare(b.storyId),
      )
  const readyWork = byStage('ready').map((n) => n.storyId)
  const gatedWork = nodes.filter((n) => n.gated).map((n) => n.storyId)
  const blockedWork = nodes
    .filter((n) => n.stage === 'blocked')
    .map((n) => n.storyId)

  return { nodes, edges, readyWork, gatedWork, blockedWork }
}

// ---------------------------------------------------------------------------
// Agent dispatch / capacity (pure — derived from the durable queue, never a
// second roster)
// ---------------------------------------------------------------------------

export function buildFactoryCapacity(
  stories: StoryboardStory[],
  workItems: AgentWorkItem[],
  pipeline: FactoryPipeline,
): FactoryCapacity {
  const workers: WorkerCapacityEntry[] = []

  // Assigned: the single active command (Claimed / Running / Paused — the
  // system enforces at most one system-wide by migration 025).
  const active = workItems.filter(
    (i) => i.state === 'Claimed' || i.state === 'Running' || i.state === 'Paused',
  )
  for (const item of active) {
    workers.push({
      workerId: item.claimedBy,
      role: item.role,
      kind: 'assigned',
      storyId: item.storyId,
      workState: item.state,
      since: item.claimedAt,
    })
  }

  // Blocked: most recent terminal failure/cancellation per worker — the slot
  // is occupied by a blocked outcome until a human resolves it.
  const failedByWorker = new Map<string, AgentWorkItem>()
  for (const item of workItems) {
    if (
      (item.state === 'Error' || item.state === 'Cancelled') &&
      item.claimedBy
    ) {
      const current = failedByWorker.get(item.claimedBy)
      if (!current || (item.finishedAt ?? '') > (current.finishedAt ?? '')) {
        failedByWorker.set(item.claimedBy, item)
      }
    }
  }
  for (const item of failedByWorker.values()) {
    workers.push({
      workerId: item.claimedBy,
      role: item.role,
      kind: 'blocked',
      storyId: item.storyId,
      workState: item.state,
      since: item.finishedAt ?? item.claimedAt,
    })
  }

  // Idle: no active command and no blocked slot → the factory has capacity.
  if (workers.length === 0) {
    workers.push({
      workerId: null,
      role: null,
      kind: 'idle',
      storyId: null,
      workState: null,
      since: null,
    })
  }

  const readyIds = new Set(pipeline.readyWork)
  const nextEligible = stories
    .filter((s) => readyIds.has(s.id))
    .sort(
      (a, b) =>
        priorityRank(a.priority) - priorityRank(b.priority) ||
        a.id.localeCompare(b.id),
    )
    .slice(0, 8)
    .map((s) => ({
      storyId: s.id,
      title: s.title,
      priority: s.priority,
      completion: s.completion,
    }))

  return {
    workers,
    assignedCount: workers.filter((w) => w.kind === 'assigned').length,
    idleCount: workers.filter((w) => w.kind === 'idle').length,
    blockedCount: workers.filter((w) => w.kind === 'blocked').length,
    nextEligible,
  }
}

// ---------------------------------------------------------------------------
// Snapshot loader (server-side; the page renders this)
// ---------------------------------------------------------------------------

function emptyPipeline(): FactoryPipeline {
  return { nodes: [], edges: [], readyWork: [], gatedWork: [], blockedWork: [] }
}

function emptyCapacity(): FactoryCapacity {
  return {
    workers: [],
    assignedCount: 0,
    idleCount: 0,
    blockedCount: 0,
    nextEligible: [],
  }
}

/**
 * Load the full factory snapshot. `ready:false` when the storyboard tables
 * are absent (migrations not yet applied) so the page renders a setup notice
 * instead of crashing. Workflow cockpit targets are best-effort: an engine
 * failure must never take down the factory console.
 */
export async function getFactoryCommandCenterSnapshot(): Promise<FactorySnapshot> {
  const stories = await listStoryboardStories()
  if (!stories) {
    return {
      ready: false,
      rollup: null,
      pipeline: emptyPipeline(),
      capacity: emptyCapacity(),
      workflowCockpits: [],
      humanGateCount: 0,
    }
  }

  const workItems = (await listAgentWorkItems()) ?? []

  // One read over the run table (newest first) → latest run per story.
  const allRuns = (await listStoryboardRuns()) ?? []
  const latestRunByStory = new Map<string, StoryRun>()
  for (const run of allRuns) {
    if (!latestRunByStory.has(run.storyId)) {
      latestRunByStory.set(run.storyId, run)
    }
  }

  const pipeline = buildFactoryPipeline(stories, workItems, latestRunByStory)

  let workflowCockpits: WorkflowSummary[] = []
  try {
    workflowCockpits = await getWorkflowSummaries()
  } catch {
    workflowCockpits = []
  }

  return {
    ready: true,
    rollup: buildStoryBoardModel(stories),
    pipeline,
    capacity: buildFactoryCapacity(stories, workItems, pipeline),
    workflowCockpits,
    humanGateCount: pipeline.nodes.filter((n) => n.gated).length,
  }
}
