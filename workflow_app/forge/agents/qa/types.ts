/**
 * Assay verdict types. `semgrepFindings` / `knipFindings` are optional on
 * purpose: they ride along for the record and never flip a verdict, so a
 * static slice that does not collect them is still valid input.
 */

export type CommandResult = {
  command: string
  exitCode: number
  passed: boolean
  excerpt: string
  /**
   * THE EXECUTED OUTPUT, beyond the 240-char excerpt.
   *
   * The adjudicator reads it to check that a mapped assertion actually RAN: a name listed in the mapping
   * but absent from this output is UNPROVEN, so the excerpt cap must never decide a verdict. This is NOT
   * the stored evidence — `excerpt` remains the bounded field that gets written to the row.
   */
  output?: string
  /**
   * TRUE when the command could not be RUN at all — a spawn error (bad cwd, missing binary) or a timeout
   * kill — as opposed to running and reporting failure.
   *
   * These were the same thing until 2026-09-15, and it made QA look flaky and too strict: `spawnSync`
   * returns `status: null` for a command it never started, the runner coerced that to `exitCode: 1`, and
   * the adjudicator recorded `CMD_FAIL <command>` for a proof that passes when it can actually be run
   * (measured: the doctor's frozen proof exits 0 with 11/11 in the candidate worktree, while the QA lane
   * recorded it as failed). "We could not check" is not "we checked and it broke" — the module already
   * distinguishes those for an empty plan; it must do it per command too.
   */
  unmeasurable?: boolean
}

export type StaticSlice = {
  archRan: boolean
  archOk: boolean
  archErrors: string[]
  semgrepFindings?: string[]
  knipFindings?: string[]
  /**
   * Migration safety (squawk), the second HARD gate. `migrationOk === false` must FAIL the verdict and
   * name the rule; `migrationRan === false` with `migrationOk === false` is a required check that could
   * not run, which is never a pass. Optional: a slice that predates the field carries no opinion.
   */
  migrationRan?: boolean
  migrationOk?: boolean
  migrationFindings?: string[]
  migrationRules?: string[]
}

/**
 * THE VERDICT IS "DID THE TESTS PASS *THE STORY'S OWN ACCEPTANCE*".
 *
 * `PASS` means every frozen command exited zero AND every acceptance condition had an assertion behind it.
 * `FAIL` means a command ran and did not pass (or an empty plan, or the static gate broke).
 * `UNPROVEN` means the proof passed while an acceptance condition had NO assertion behind it: the proof
 * tested the cheapest reading of the clause, not the clause, so it proves nothing and is named as such.
 *
 * The old third state, `INCOMPLETE` ("we could not run it"), stays gone: a command that could not run did
 * not pass, and each command's own `unmeasurable` flag records WHY on the row. UNPROVEN is a different
 * fact — the tests DID run, and they did not cover a clause of the acceptance.
 */
export type QaVerdict = 'PASS' | 'FAIL' | 'UNPROVEN'

export type QaReport = {
  version: 1
  verdict: QaVerdict
  commands: CommandResult[]
  staticGate: StaticSlice | null
  blockers: string[]
  /** Ids of the acceptance conditions that had no assertion behind them. Empty is the normal case. */
  unproven: string[]
  /**
   * Ids of the acceptance conditions whose mapped assertion RAN AND FAILED. A failed assertion is a FAIL,
   * never UNPROVEN: the proof spoke about the clause and said no. Empty is the normal case.
   */
  failedConditions: string[]
  /**
   * The mapped assertions that did NOT appear in the executed proof output — a named-but-unrun assertion
   * proves nothing. Each entry names the clause and the assertion the proof never ran.
   */
  missingAssertions: Array<{ conditionId: string; assertion: string }>
  /**
   * ENG-FORGE-FENCE-CAN-FAIL-01 — the negative-control evidence. Absent when the plan declared no
   * control; present whenever one was declared, whether it killed an assertion, survived, or could
   * not run. A control that survived is why a green fence is UNPROVEN rather than PASS.
   */
  negativeControl?: NegativeControlOutcome
}

/**
 * ONE acceptance condition and the assertions in the frozen proof that assert it.
 *
 * `assertions` EMPTY is a representable, load-bearing state: the clause has no assertion behind it, so the
 * verdict is UNPROVEN rather than PASS. This is the whole point of the story — the lane that did the work
 * does not get to choose what is tested, and a clause with no assertion is reported, not passed.
 */
export type AcceptanceCondition = {
  id: string
  text: string
  assertions: string[]
}

/**
 * THE ACCEPTANCE-TO-ASSERTION MAPPING, written BEFORE the work by the Architect or the Lead.
 *
 * `hash` freezes it. QA is handed the map and compares it to the frozen hash: a lane that changes what is
 * tested after the fact is RECORDED as having done so, never silently re-derived.
 */
export type AcceptanceMap = {
  version: 1
  hash: string
  conditions: AcceptanceCondition[]
}

/** Split a story's free-text acceptance into discrete conditions. One clause per non-empty line. */
export function acceptanceClauses(criteria: string | null | undefined): string[] {
  return (criteria ?? '')
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*(?:[-*•]|\d+[.)])\s*/, '').trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
}

export function acceptanceConditionId(text: string, index: number): string {
  const slug = text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48)
  return slug || `condition-${index + 1}`
}

/** A stable content hash of the mapping. Pure: no crypto import, so it travels anywhere the types do. */
export function acceptanceMapHash(conditions: readonly AcceptanceCondition[]): string {
  const canonical = conditions
    .map((c) => `${c.id}\u0000${c.text}\u0000${[...c.assertions].sort().join('\u0001')}`)
    .join('\u0002')
  let h = 2166136261
  for (let i = 0; i < canonical.length; i++) {
    h ^= canonical.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return (h >>> 0).toString(16).padStart(8, '0')
}

/**
 * Build the mapping from the story's acceptance clauses and the Architect/Lead-declared assertions.
 * A clause with no declared assertion becomes a condition with `assertions: []` — representable, and
 * therefore UNPROVEN at verdict time. This is authored BEFORE the work and frozen by `hash`.
 */
export function buildAcceptanceMap(input: {
  acceptance: readonly string[]
  assertions?: Record<string, string[]> | null
}): AcceptanceMap {
  const conditions: AcceptanceCondition[] = input.acceptance
    .map((text) => text.trim())
    .filter(Boolean)
    .map((text, index) => {
      const id = acceptanceConditionId(text, index)
      const refs = input.assertions?.[text] ?? input.assertions?.[id] ?? []
      return { id, text, assertions: [...new Set(refs.filter((r) => r.trim().length > 0))] }
    })
  return { version: 1, hash: acceptanceMapHash(conditions), conditions }
}

/** True when a lane handed QA a mapping that differs from the frozen one. */
export function acceptanceMapChanged(
  frozen: AcceptanceMap,
  current: readonly AcceptanceCondition[],
): boolean {
  return acceptanceMapHash(current) !== frozen.hash
}

/**
 * A NEGATIVE CONTROL: the SAME fence, run with the claimed behaviour withheld or inverted.
 *
 * Its RED is the required outcome — it must kill at least one intended assertion, or the fence has
 * never been shown to discriminate and its green proves nothing. It is a single command so the
 * inversion, the run and the destruction of the scratch state happen inside one invocation: no
 * mutant, no scratch file and no branch outlives it.
 */
export type NegativeControl = {
  /** The single command that applies the inversion, runs the SAME fence, and destroys its scratch. */
  command: string
  /**
   * The intended assertions the control is expected to kill. ABSENT means every assertion mapped on
   * the plan's acceptance conditions is intended — the mapping already says what the fence proves.
   */
  assertions?: string[]
}

/** What the negative control did, recorded on the report as evidence. */
export type NegativeControlOutcome = {
  /** The command that was run, so the evidence names the exact control. */
  command: string
  /** True when the control actually executed. */
  ran: boolean
  /** True when it could not be RUN at all (spawn error or timeout) — never a kill. */
  unmeasurable: boolean
  /** The mapped assertions whose marker line reported `failed` under the control. */
  killingAssertions: string[]
}

/**
 * What QA is given: the story's frozen proofs, and nothing else. No candidate, no SHA, no lineage — QA has
 * no relationship to git and no role in committing, promoting or advising on a release.
 */
export type AssayPlan = {
  /** Frozen story/chunk proofs. Empty means there is nothing to test. */
  commands: string[]
  /**
   * The story's acceptance conditions and the assertion behind each.
   *
   * ABSENT means no mapping was supplied at this layer (legacy direct callers), and the verdict is decided
   * by the commands alone. A condition PRESENT with an empty `assertions` is the UNPROVEN case: the clause
   * has no assertion behind it and PASS is refused.
   */
  conditions?: AcceptanceCondition[]
  /**
   * The story's declared negative control, if any. ABSENT preserves today's semantics exactly: the
   * verdict is decided by the commands and the mapping alone. PRESENT means a green fence must also
   * be shown to discriminate — a control that kills nothing makes the verdict UNPROVEN.
   */
  negativeControl?: NegativeControl
}

export const FAILURE_CLASSES = [
  'CODE_DEFECT',
  'TEST_DEFECT',
  'ARCHITECTURE_GAP',
  'REQUIREMENTS_GAP',
  'UNKNOWN_CAUSE',
  'ENVIRONMENT',
  'MIGRATION',
  'PUBLISH_CONFLICT',
  'DEPLOYMENT',
  'PRODUCTION_SMOKE',
  'HOLD',
] as const

export type FailureClass = (typeof FAILURE_CLASSES)[number]
