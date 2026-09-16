// ---------------------------------------------------------------------------
// ENG-FORGE-QA-CONSISTENCY-01 — the QA run/verdict agreement check.
//
// One fact has ONE writer. A QA run's own `result_status` on
// `storyboard_story_run` and the durable `forge_workflow_evidence.qa_passed`
// verdict are two readings of the same fact, so when they disagree the
// disagreement IS the bug — the class found on 2026-09-16, where a QA lane that
// held still had a verdict recorded against it.
//
// This module is the PURE half: it takes the two readings and returns
// agree | disagree | unknown, naming BOTH values on a disagreement. It has NO
// database access, NO filesystem access and NO clock — which is what makes the
// agreement, the disagreement and the no-verdict cases unit tests rather than
// live probes.
//
// READ-ONLY IS A HARD REQUIREMENT: this is a report, not a repair. Nothing here
// writes, and a run with no verdict reports `unknown` — never `agree`.
// ---------------------------------------------------------------------------

export type QaVerdict = 'PASS' | 'FAIL'

export type QaRunVerdictInput = {
  /** The QA run's own `result_status` (e.g. Complete / Failed / Hold). */
  runStatus?: string | null
  /** The durable verdict: `forge_workflow_evidence.qa_passed`, or a PASS/FAIL token. */
  verdict?: string | boolean | null
}

export type QaConsistencyResult =
  | { state: 'agree'; runStatus: string; verdict: QaVerdict }
  | { state: 'disagree'; runStatus: string | null; verdict: QaVerdict; detail: string }
  | { state: 'unknown'; reason: string }

/**
 * A verdict that cannot be read is NOT a FAIL — it is no verdict at all. `true`
 * and `false` are the durable boolean; `'PASS'`/`'FAIL'` are the token spelling,
 * case-insensitive. Anything else reads `null`.
 */
export function normalizeQaVerdict(value: string | boolean | null | undefined): QaVerdict | null {
  if (value === true) return 'PASS'
  if (value === false) return 'FAIL'
  if (typeof value === 'string') {
    const token = value.trim().toUpperCase()
    if (token === 'PASS' || token === 'FAIL') return token
  }
  return null
}

/**
 * The verdict a run's own status implies. `Complete` => PASS and `Failed` =>
 * FAIL. Any other status (Hold, Cancelled, blank, absent) makes no clean
 * pass/fail claim, so it implies `null` — a run that did not complete cleanly
 * cannot certify a pass.
 */
export function expectedVerdictForRunStatus(
  runStatus: string | null | undefined,
): QaVerdict | null {
  const token = (runStatus ?? '').trim().toLowerCase()
  if (token === 'complete') return 'PASS'
  if (token === 'failed') return 'FAIL'
  return null
}

/**
 * The engine's own QA-lane rule (`forge-evidence-db.ts:forgeFactFamily`): a run
 * whose `run_type` is the qa/assay family. Kept here so the doctor selects QA
 * runs by one definition instead of a second list.
 */
export function isQaRunType(runType: string | null | undefined): boolean {
  const token = (runType ?? '').trim().toLowerCase()
  if (!token) return false
  return token.includes('qa') || token.includes('assay')
}

function describeRunStatus(runStatus: string | null | undefined): string {
  const token = (runStatus ?? '').trim()
  return token ? token : 'none'
}

/**
 * The check. A run status and a verdict either agree, or the disagreement names
 * both values. A run with no readable verdict is `unknown` — never `agree`.
 */
export function checkQaRunVerdictConsistency(input: QaRunVerdictInput): QaConsistencyResult {
  const verdict = normalizeQaVerdict(input.verdict)
  if (verdict === null) {
    return { state: 'unknown', reason: 'no durable QA verdict recorded for this run' }
  }

  const runStatus = input.runStatus ?? null
  const expected = expectedVerdictForRunStatus(runStatus)

  if (expected === null) {
    return {
      state: 'disagree',
      runStatus,
      verdict,
      detail:
        `run status '${describeRunStatus(runStatus)}' makes no clean pass/fail claim ` +
        `but the durable verdict is ${verdict}`,
    }
  }

  if (expected === verdict) {
    return { state: 'agree', runStatus: describeRunStatus(runStatus), verdict }
  }

  return {
    state: 'disagree',
    runStatus,
    verdict,
    detail:
      `run status '${describeRunStatus(runStatus)}' expects ${expected} ` +
      `but the durable verdict is ${verdict}`,
  }
}

/** One line for the doctor. A disagreement always names both values. */
export function renderQaConsistencyLine(result: QaConsistencyResult): string {
  switch (result.state) {
    case 'agree':
      return `agree (run ${result.runStatus} / verdict ${result.verdict})`
    case 'disagree':
      return `DISAGREE — ${result.detail}`
    case 'unknown':
      return `unknown (${result.reason})`
  }
}
