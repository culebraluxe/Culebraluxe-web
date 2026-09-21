/**
 * WHAT EACH CHECK ACTUALLY DID (FORGE-GATE-RECEIPT-01).
 *
 * Astra review feature 3: a run should record which checks ran, which failed, which were SKIPPED, which were
 * UNAVAILABLE and which were never configured — with a reason — because today's evidence says "4 skipped"
 * with no identity, and a reader cannot tell which four. The repository's own rule is the reason this is a
 * receipt and not a verdict: "a skipped gate is visibly skipped; a gate that silently passes because it had
 * no credentials is the failure mode this repository already recorded".
 *
 * TWO PROPERTIES THIS MODULE EXISTS TO GUARANTEE:
 *   1. NO SECOND VERDICT WRITER. It does not decide anything — it is a pure projection of checks that
 *      already happened (the frozen proofs, the static gate's architecture and migration halves, the
 *      acceptance mapping, the negative control). Adding a check cannot change a verdict.
 *   2. SKIPPED IS NOT PASSED. Anything that did not run is `skipped`, `unavailable` or `not-configured`,
 *      never `passed` — the exact confusion FORGE-ASSERTION-SKIP-STATE-01 fixed in the acceptance reader.
 */
import type { CommandResult, NegativeControlOutcome, StaticSlice } from '@/legacy/workflow_app/forge/agents/qa/types'

export type GateCheckStatus = 'passed' | 'failed' | 'skipped' | 'unavailable' | 'not-configured'

export type GateCheck = {
  /** Stable identity a reader can search for: `proof:<command>`, `architecture`, `migration`, … */
  id: string
  status: GateCheckStatus
  /** Why — the rule, the exit code, the missing input. Null only when the status speaks for itself. */
  reason: string | null
}

export function gateChecksFor(input: {
  commands: readonly string[]
  results: readonly CommandResult[]
  staticGate: StaticSlice | null
  acceptanceMapped: boolean
  negativeControl: NegativeControlOutcome | null
  /**
   * The control RAN and killed nothing. A surviving control is the reason the verdict is UNPROVEN, so the
   * receipt must not call it `passed` — that was the review's first reproduction: QA UNPROVEN beside a receipt
   * reading "passed, killed nothing".
   */
  controlSurvived?: boolean
}): GateCheck[] {
  const checks: GateCheck[] = []

  for (const command of input.commands) {
    const result = input.results.find((r) => r.command === command)
    if (!result) {
      checks.push({ id: `proof:${command}`, status: 'unavailable', reason: 'the command produced no result' })
      continue
    }
    // COULD NOT RUN is not "ran and said no": a CommandResult carries `unmeasurable` for a spawn failure or a
    // timeout, and reporting that as an ordinary failed assertion misleads the reader about the clause.
    if (result.unmeasurable === true) {
      checks.push({
        id: `proof:${command}`,
        status: 'unavailable',
        reason: result.excerpt?.trim() || 'the command could not be executed',
      })
      continue
    }
    checks.push({
      id: `proof:${command}`,
      status: result.passed ? 'passed' : 'failed',
      reason: result.passed ? null : `exit ${result.exitCode}`,
    })
  }

  const gate = input.staticGate
  if (!gate) {
    checks.push({ id: 'architecture', status: 'unavailable', reason: 'the static gate did not run' })
    checks.push({ id: 'migration', status: 'unavailable', reason: 'the static gate did not run' })
  } else {
    checks.push({
      id: 'architecture',
      status: !gate.archRan ? 'skipped' : gate.archOk ? 'passed' : 'failed',
      reason: !gate.archRan ? 'the architecture gate was skipped' : gate.archOk ? null : gate.archErrors.join(' | ') || null,
    })
    // The migration half is OPTIONAL in the slice (a slice that predates the field carries no opinion), so an
    // absent half is `not-configured` rather than passed — and a half that could not run is `skipped`.
    checks.push({
      id: 'migration',
      status:
        gate.migrationOk === undefined
          ? 'not-configured'
          : gate.migrationRan === false
            ? 'skipped'
            : gate.migrationOk
              ? 'passed'
              : 'failed',
      reason:
        gate.migrationOk === undefined
          ? 'the static gate returned no migration verdict'
          : gate.migrationRan === false
            ? 'the migration lint did not run'
            : gate.migrationOk
              ? null
              : gate.migrationFindings?.join(' | ') || 'migration lint failed',
    })
  }

  checks.push({
    id: 'acceptance-map',
    status: input.acceptanceMapped ? 'passed' : 'not-configured',
    reason: input.acceptanceMapped ? null : 'no acceptance-to-assertion mapping was declared',
  })

  const control = input.negativeControl
  checks.push({
    id: 'negative-control',
    status: !control
      ? 'not-configured'
      : control.unmeasurable
        ? 'unavailable'
        : control.ran && !input.controlSurvived
          ? 'passed'
          : control.ran
            ? 'failed'
            : 'skipped',
    reason: !control
      ? 'the story declared no negative control'
      : control.unmeasurable
        ? 'the control could not be executed'
        : control.ran
          ? input.controlSurvived
            ? 'the control RAN and killed nothing — a surviving control proves no discrimination'
            : `killed ${control.killingAssertions.join(', ') || 'the intended assertions'}`
          : 'the control did not run',
  })

  return checks
}
