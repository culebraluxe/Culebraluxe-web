// ---------------------------------------------------------------------------
// ENG-FORGE-CONVERGENCE / Scope D data glue — turn durable engine-task + story-run
// rows into ForgeCandidateEvents, projecting them through the pure convergence
// read model (forge-convergence.ts).
//
// Durable sources (confirmed schema):
//   forge_engine_task_execution: per role-node run (node_id, story_run_id, ...)
//   storyboard_story_run:         frozen run (commit_hash, result_status, ...)
//   storyboard_story:             forge_repair_attempts / forge_replan_attempts
//
// Execution generation is NOT stamped per run, so it is reconstructed
// deterministically: generation advances each time a replan node
// (repair_architect) executes. This mirrors how forge-executor increments the
// durable replan counter at repair_architect completion.
//
// Pure + DB-free. Deterministic.
// ---------------------------------------------------------------------------

import {
  projectForgeConvergence,
  type ForgeCandidateEvent,
  type ForgeConvergenceExecution,
} from './forge-convergence'

/** A role-node run as it exists durably (engine task execution + its story run). */
export type ForgeNodeRun = {
  nodeId: string
  storyRunId: string | null
  /** Exact candidate commit produced / tested by this run (storyboard_story_run.commit_hash). */
  commitHash: string | null
  createdAt: string
  /** storyboard_story_run.result_status — deterministic PASS/FAIL for a QA run. */
  resultStatus: string | null
}

const PRODUCER_NODES = new Set([
  'smith',
  'smith_split_work',
  'repair_smith',
  'lead_post',
  'lead_solo_implement',
  'fast_smith',
  'fast_repair_smith',
])
const QA_NODES = new Set(['qa_verify', 'fast_qa_verify'])
const REPLAN_NODES = new Set(['repair_architect'])

function isPassResult(status: string | null): boolean {
  return /pass|success|complete/i.test(status ?? '')
}

/**
 * Reconstruct a story's candidate/QA event stream from its durable role-node
 * runs, deriving each run's execution generation from prior replan nodes.
 */
export function buildConvergenceEvents(runs: ForgeNodeRun[]): ForgeCandidateEvent[] {
  const sorted = [...runs].sort((a, b) => a.createdAt.localeCompare(b.createdAt))
  const events: ForgeCandidateEvent[] = []
  let generation = 0
  for (const run of sorted) {
    if (REPLAN_NODES.has(run.nodeId)) {
      generation += 1
      continue
    }
    if (!run.commitHash) continue
    if (PRODUCER_NODES.has(run.nodeId)) {
      events.push({
        kind: 'producer',
        generation,
        sha: run.commitHash,
        nodeId: run.nodeId,
        runId: run.storyRunId,
        at: run.createdAt,
      })
    } else if (QA_NODES.has(run.nodeId)) {
      events.push({
        kind: 'qa',
        generation,
        sha: run.commitHash,
        runId: run.storyRunId,
        verdict: isPassResult(run.resultStatus) ? 'PASS' : 'FAIL',
        at: run.createdAt,
      })
    }
  }
  return events
}

/** Project a story's durable runs through the convergence read model. */
export function projectStoryRunsToConvergence(input: {
  storyId: string
  processInstanceId: string
  runs: ForgeNodeRun[]
  repairAttempts: number
  replanAttempts: number
}): ForgeConvergenceExecution[] {
  return projectForgeConvergence({
    storyId: input.storyId,
    processInstanceId: input.processInstanceId,
    events: buildConvergenceEvents(input.runs),
    repairAttempts: input.repairAttempts,
    replanAttempts: input.replanAttempts,
  })
}
