// ---------------------------------------------------------------------------
// ENG-FORGE-CONVERGENCE-READMODEL-01 — candidate iteration & convergence read
// model (Scope B). Pure + DB-free. Deterministic.
//
// This is a derived read model over durable execution evidence, NOT a second
// workflow engine. It answers: given a story's chronological candidate/QA
// events, what are the candidate iterations, the convergence state, and is the
// repair loop making progress?
//
// Rules (Scope B acceptance 8-14):
//   - A new persisted candidate SHA creates the next candidate iteration.
//   - A run that produces no new SHA does not create an iteration.
//   - Retrying QA against the same SHA does not create another iteration.
//   - A QA verdict always binds to the exact candidate SHA tested.
//   - Repair after QA FAIL stays in the same generation (next SHA, same lineage).
//   - REPLAN starts a new execution generation; numbering restarts per
//     generation, but every generation's history is retained (executions list).
//   - CONVERGED is derived ONLY from deterministic QA PASS on the current SHA.
//   - No-progress: the SAME SHA re-fails the SAME machine classification with
//     no new candidate in between -> HOLD/NO_PROGRESS (never auto re-repair).
// ---------------------------------------------------------------------------

export type ConvergenceState = 'ITERATING' | 'CONVERGED' | 'REPLAN_REQUIRED' | 'HOLD'

export type ForgeCandidateIteration = {
  iteration: number
  candidateSha: string
  producerNodeId: string
  producerRunId: string | null
  qaRunId: string | null
  qaVerdict: 'PASS' | 'FAIL' | 'PENDING'
  qaFailureClass: string | null
  qaDisposition: string | null
  startedAt: string | null
  endedAt: string | null
}

export type ForgeConvergenceExecution = {
  executionId: string
  storyId: string
  generation: number
  processStatus: string
  currentNode: string | null
  convergenceState: ConvergenceState
  repairAttempts: number
  replanAttempts: number
  iterations: ForgeCandidateIteration[]
  holdReason: string | null
}

export type ForgeCandidateEvent =
  | {
      kind: 'producer'
      generation: number
      sha: string
      nodeId: string
      runId: string | null
      at: string | null
    }
  | {
      kind: 'qa'
      generation: number
      sha: string
      runId: string | null
      verdict: 'PASS' | 'FAIL'
      failureClass?: string | null
      disposition?: string | null
      at: string | null
    }

/** Failure classes that signal a specification/architecture defect -> REPLAN. */
const REPLAN_CLASSES = new Set(['ARCHITECTURE_GAP', 'REQUIREMENTS_GAP'])

function atOf(e: ForgeCandidateEvent): string {
  return e.at ?? ''
}
/**
 * Candidate iterations for ONE generation, in exact-SHA first-seen order.
 * A new SHA starts a new iteration; a re-produced SHA or a QA retry on the same
 * SHA never starts one. QA verdict binds to the candidate SHA it tested (latest
 * QA per SHA wins); absent QA the iteration is PENDING.
 */
export function candidateIterationsForGeneration(events: ForgeCandidateEvent[]): ForgeCandidateIteration[] {
  const producers = events.filter((e): e is Extract<ForgeCandidateEvent, { kind: 'producer' }> => e.kind === 'producer')
  const qas = events.filter((e): e is Extract<ForgeCandidateEvent, { kind: 'qa' }> => e.kind === 'qa')
  const ordered = [...producers].sort((a, b) => atOf(a).localeCompare(atOf(b)))
  const order: string[] = []
  const bySha = new Map<string, ForgeCandidateIteration>()

  for (const p of ordered) {
    const existing = bySha.get(p.sha)
    if (!existing) {
      order.push(p.sha)
      bySha.set(p.sha, {
        iteration: 0,
        candidateSha: p.sha,
        producerNodeId: p.nodeId,
        producerRunId: p.runId,
        qaRunId: null,
        qaVerdict: 'PENDING',
        qaFailureClass: null,
        qaDisposition: null,
        startedAt: p.at,
        endedAt: p.at,
      })
    } else if (p.at) {
      // Re-producing the SAME SHA (no change) is not a new iteration.
      existing.endedAt = p.at
    }
  }

  order.forEach((sha, i) => {
    bySha.get(sha)!.iteration = i + 1
  })

  for (const q of qas) {
    const it = bySha.get(q.sha)
    if (!it) continue // a QA verdict never fabricates a candidate it did not test
    it.qaRunId = q.runId
    it.qaVerdict = q.verdict
    it.qaFailureClass = q.failureClass ?? null
    it.qaDisposition = q.disposition ?? null
    if (q.at) it.endedAt = q.at
  }

  return order.map((sha) => bySha.get(sha)!)
}

export type NoProgressCheck = { noProgress: boolean; reason: string | null }

/**
 * No-progress guard: the SAME candidate SHA re-fails the SAME machine
 * classification (failure class + disposition) with no new candidate in
 * between. Uses exact SHA + typed evidence only — never fuzzy semantics.
 */
export function checkNoProgress(events: ForgeCandidateEvent[]): NoProgressCheck {
  const sorted = [...events].sort((a, b) => atOf(a).localeCompare(atOf(b)))
  let currentCandidate: string | null = null
  let lastKey: string | null = null
  let repeats = 0
  for (const e of sorted) {
    if (e.kind === 'producer') {
      if (e.sha !== currentCandidate) {
        currentCandidate = e.sha
        lastKey = null
        repeats = 0
      }
      continue
    }
    if (e.verdict === 'PASS') {
      lastKey = null
      repeats = 0
      continue
    }
    // FAIL on the current candidate
    if (e.sha !== currentCandidate) continue
    const key = `${e.failureClass ?? 'NA'}|${e.disposition ?? 'NA'}`
    if (key === lastKey) {
      repeats += 1
    } else {
      lastKey = key
      repeats = 1
    }
    if (repeats >= 2) {
      return {
        noProgress: true,
        reason: `NO_PROGRESS: candidate ${e.sha} re-failed the same classification (${key}) with no new candidate in between`,
      }
    }
  }
  return { noProgress: false, reason: null }
}

/** Derive the convergence state for one execution from its iterations. */
export function convergenceState(iterations: ForgeCandidateIteration[], noProgress: NoProgressCheck): ConvergenceState {
  if (noProgress.noProgress) return 'HOLD'
  const last = iterations[iterations.length - 1]
  if (!last) return 'ITERATING'
  if (last.qaVerdict === 'PASS') return 'CONVERGED'
  if (last.qaVerdict === 'FAIL') {
    const replan =
      last.qaDisposition === 'REPLAN' ||
      (last.qaFailureClass != null && REPLAN_CLASSES.has(last.qaFailureClass))
    return replan ? 'REPLAN_REQUIRED' : 'ITERATING'
  }
  return 'ITERATING'
}

/**
 * Project a story's chronological candidate/QA events into one execution per
 * generation. Iteration numbering restarts per generation (rule 6); the full
 * prior history is retained because every generation is returned.
 */
export function projectForgeConvergence(input: {
  storyId: string
  processInstanceId: string
  events: ForgeCandidateEvent[]
  repairAttempts: number
  replanAttempts: number
  processStatus?: string
  currentNode?: string | null
}): ForgeConvergenceExecution[] {
  const { storyId, processInstanceId, events, repairAttempts, replanAttempts } = input
  const gens = new Map<number, ForgeCandidateEvent[]>()
  for (const e of events) {
    const list = gens.get(e.generation) ?? []
    list.push(e)
    gens.set(e.generation, list)
  }
  const sortedGens = [...gens.keys()].sort((a, b) => a - b)
  return sortedGens.map((gen) => {
    const genEvents = gens.get(gen)!
    const iterations = candidateIterationsForGeneration(genEvents)
    const np = checkNoProgress(genEvents)
    return {
      executionId: `${processInstanceId}-e${gen}`,
      storyId,
      generation: gen,
      processStatus: input.processStatus ?? 'active',
      currentNode: input.currentNode ?? null,
      convergenceState: convergenceState(iterations, np),
      repairAttempts,
      replanAttempts,
      iterations,
      holdReason: np.reason,
    } satisfies ForgeConvergenceExecution
  })
}

