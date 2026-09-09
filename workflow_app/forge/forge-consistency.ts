// ---------------------------------------------------------------------------
// forge-consistency — ENG-FORGE-V5-08: the Forge Consistency Janitor (audit).
//
// A read-only consistency pass over the durable Forge state (per story). It
// detects the known invariant violations so an operator can reconcile instead
// of letting stale/contradictory evidence hide in the storyboard:
//   I1 complete-with-only-scout-evidence    a story "completed" with no build
//                                           evidence behind it
//   I2 in-progress-with-no-actionable-work  a story parked with no open task
//   I3 smith-candidate-without-assay        candidate produced, never assayed
//   I4 clean-assay-with-unpublished         verified candidate never published
//   I5 terminal-work/story-disagreement     run and story disagree on terminal
//
// PURE + DB-free detection over a normalized per-story snapshot, so every rule
// is unit-testable. The DB seam (forge-consistency-db) builds snapshots from
// the real durable tables. This module only reports — repair is a separate,
// explicit operator action (janitors that silently "fix" production evidence
// are dangerous).
// ---------------------------------------------------------------------------

/** Engine node ids that are Scout-grade (evidence-only research/diagnosis). */
export const SCOUT_NODES: ReadonlySet<string> = new Set([
  'feature_scout',
  'research_scout',
  'diagnose_scout',
  'repair_scout',
])

export type StoryConsistencySnapshot = {
  storyId: string
  storyStatus: string
  /** Durable workflow evidence for the story's latest Forge instance, if any. */
  evidence: {
    candidateSha: string | null
    qaPassed: boolean | null
    qaVerifiedSha: string | null
    publishedSha: string | null
    deployedSha: string | null
  } | null
  /** Latest storyboard_story_run outcome, if any. */
  run: { resultStatus: string | null } | null
  /** Distinct engine node ids that reached a terminal (completed) state. */
  engineNodesCompleted: string[]
  /** Open (ready/reserved/in_progress) role tasks remaining for the story. */
  openTaskCount: number
}

export type ConsistencySeverity = 'error' | 'warn'

export type ConsistencyViolation = {
  storyId: string
  kind:
    | 'complete-with-only-scout-evidence'
    | 'in-progress-no-actionable-work'
    | 'smith-candidate-without-assay'
    | 'clean-assay-unpublished'
    | 'terminal-disagreement'
  severity: ConsistencySeverity
  detail: string
}

const terminalStoryStatuses = new Set(['complete', 'done', 'cancelled', 'canceled', 'archived'])

function isStoryTerminal(s: string): boolean {
  return terminalStoryStatuses.has(s.toLowerCase())
}

/** I1 — a story marked "Complete" that the Forge engine processed (it has
 *  durable evidence) yet completed with no build behind it — no candidate, no
 *  verified/passed QA (scout-only or empty build evidence). Legacy stories that
 *  never ran the engine have no evidence row and are NOT this janitor's concern. */
function completeWithOnlyScoutEvidence(s: StoryConsistencySnapshot): ConsistencyViolation | null {
  if (s.storyStatus.toLowerCase() !== 'complete') return null
  if (!s.evidence) return null
  const hasBuild =
    Boolean(s.evidence.candidateSha) ||
    Boolean(s.evidence.qaVerifiedSha) ||
    s.evidence.qaPassed === true
  if (hasBuild) return null
  return {
    storyId: s.storyId,
    kind: 'complete-with-only-scout-evidence',
    severity: 'error',
    detail: 'story Complete but Forge evidence shows no candidate/QA (scout-only or no build)',
  }
}

/** I2 — a story that STARTED (it has run/engine evidence) but is not terminal
 *  and has no open task and no candidate: a started run parked/abandoned without
 *  reaching a terminal. Backlog stories that have never run (no evidence) are not
 *  violations — having no open task before a story is picked up is expected. */
function inProgressNoActionableWork(s: StoryConsistencySnapshot): ConsistencyViolation | null {
  if (isStoryTerminal(s.storyStatus)) return null
  const started = Boolean(s.evidence) || Boolean(s.run?.resultStatus)
  if (!started) return null
  if (s.openTaskCount > 0) return null
  if (s.evidence?.candidateSha) return null
  return {
    storyId: s.storyId,
    kind: 'in-progress-no-actionable-work',
    severity: 'warn',
    detail: `story '${s.storyStatus}' started but has no open task and no candidate (openTaskCount=0)`,
  }
}

/** I3 — a candidate was produced but never assayed (no QA pass/verify recorded). */
function smithCandidateWithoutAssay(s: StoryConsistencySnapshot): ConsistencyViolation | null {
  const e = s.evidence
  if (!e?.candidateSha) return null
  const assayed = e.qaPassed === true || Boolean(e.qaVerifiedSha)
  if (assayed) return null
  return {
    storyId: s.storyId,
    kind: 'smith-candidate-without-assay',
    severity: 'warn',
    detail: `candidate_sha=${e.candidateSha.slice(0, 12)}… has no Assay result`,
  }
}

/** I4 — a verified (clean-assay) candidate was never published/promoted. */
function cleanAssayUnpublished(s: StoryConsistencySnapshot): ConsistencyViolation | null {
  const e = s.evidence
  if (!e) return null
  const verified = e.qaPassed === true || Boolean(e.qaVerifiedSha)
  if (!verified) return null
  const promoted = Boolean(e.publishedSha) || Boolean(e.deployedSha)
  if (promoted) return null
  return {
    storyId: s.storyId,
    kind: 'clean-assay-unpublished',
    severity: 'warn',
    detail: `verified candidate (${(e.qaVerifiedSha ?? e.candidateSha)?.slice(0, 12)}…) never published`,
  }
}

/** I5 — the latest run and the story disagree on whether work is terminal. */
function terminalDisagreement(s: StoryConsistencySnapshot): ConsistencyViolation | null {
  const runStatus = s.run?.resultStatus
  if (!runStatus) return null
  const runTerminal = runStatus === 'Complete' || runStatus === 'Failed'
  const storyTerminal = isStoryTerminal(s.storyStatus)
  if (runTerminal && !storyTerminal) {
    return {
      storyId: s.storyId,
      kind: 'terminal-disagreement',
      severity: 'warn',
      detail: `run is ${runStatus} but story is '${s.storyStatus}' (run terminal, story not)`,
    }
  }
  if (!runTerminal && storyTerminal) {
    return {
      storyId: s.storyId,
      kind: 'terminal-disagreement',
      severity: 'warn',
      detail: `story is terminal ('${s.storyStatus}') but latest run is ${runStatus} (not terminal)`,
    }
  }
  return null
}

/** Run every detector. Returns zero or more violations per story. */
export function auditStoryConsistency(
  snapshot: StoryConsistencySnapshot,
): ConsistencyViolation[] {
  const out: ConsistencyViolation[] = []
  const detectors = [
    completeWithOnlyScoutEvidence,
    inProgressNoActionableWork,
    smithCandidateWithoutAssay,
    cleanAssayUnpublished,
    terminalDisagreement,
  ]
  for (const d of detectors) {
    const v = d(snapshot)
    if (v) out.push(v)
  }
  return out
}
