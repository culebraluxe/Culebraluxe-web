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
