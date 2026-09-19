import type {
  AcceptanceCondition,
  AcceptanceMap,
  AssayPlan,
  CommandResult,
  NegativeControl,
  NegativeControlOutcome,
  QaReport,
  QaVerdict,
  StaticSlice,
} from './types'
import { acceptanceMapChanged } from './types'

export type RunCommand = (command: string) => CommandResult
export type RunStatic = () => StaticSlice

const excerpt = (s: string) => s.replace(/\s+/g, ' ').trim().slice(0, 240)

/**
 * WHAT THE EXECUTED PROOF SAID ABOUT ONE NAMED ASSERTION.
 *
 * `passed` — the assertion ran and its line reports success.
 * `failed` — the assertion ran and its line reports failure.
 * `absent` — the assertion does not appear in the executed output at all. A named-but-unrun assertion
 *            proves nothing, so its clause is UNPROVEN exactly like a clause with no assertion.
 * `skipped` — the assertion's marker line carries a SKIP/TODO directive, so the proof says it did NOT run
 *            by its own choice. This is a NAMED state, not a silent absence: acceptance evidence can tell
 *            "did not run" from "was skipped by its own proof", and neither is ever a pass.
 */
export type AssertionOutcome = 'passed' | 'failed' | 'absent' | 'skipped'

const PASS_MARKERS = ['\u2714', '\u2713'] // ✔ ✓
const FAIL_MARKERS = ['\u2716', '\u2717', '\u2718'] // ✖ ✗ ✘

/**
 * A LINE CARRYING A SKIP/TODO DIRECTIVE RAN NOTHING, so it is neither a pass nor a failure.
 *
 * MEASURED (Astra review, 2026-09-18) and reproduced: `ok 1 - required assertion # SKIP missing tool`
 * was read as `passed`, so a required assertion could be satisfied by a test that never executed. That
 * is the worst kind of false PASS — the acceptance evidence is the thing being forged — and the TAP
 * directive is the honest signal that it did not run. Such a line is now its own `skipped` verdict: the
 * clause stays UNPROVEN, but it is NAMED as skipped rather than silently read as absent.
 */
const SKIP_DIRECTIVE = /(^|\s)#\s*(skip|todo)\b/i

/**
 * The verdict a single output line reports, or null when the line is not a marker line at all.
 *
 * A SKIP/TODO directive line is its OWN verdict, not an absence: the proof spoke about the assertion and
 * said it did not run. Returning `'skipped'` (rather than the old `null`) keeps it a marker line so
 * `markerName` still resolves the assertion's name, while `assertionOutcome` refuses to call it a pass.
 */
function markerLine(line: string): 'passed' | 'failed' | 'skipped' | null {
  const text = line.trimStart()
  if (SKIP_DIRECTIVE.test(text)) return 'skipped'
  // `not ok` must be tested BEFORE `ok`: the TAP failure line contains the pass word.
  if (text.startsWith('not ok')) return 'failed'
  for (const marker of FAIL_MARKERS) if (text.startsWith(marker)) return 'failed'
  for (const marker of PASS_MARKERS) if (text.startsWith(marker)) return 'passed'
  if (/^ok\b/.test(text)) return 'passed' // TAP: `ok 1 - name`
  return null
}

/** A marker line's assertion name, with the TAP counter, a timing suffix and any directive stripped. */
function markerName(line: string): string | null {
  if (markerLine(line) === null) return null
  const text = line.trimStart()
  const tap = /^(?:not ok|ok)\s+\d+\s*-\s*(.*)$/i.exec(text)
  const raw = tap
    ? tap[1]
    : [...PASS_MARKERS, ...FAIL_MARKERS].some((m) => text.startsWith(m))
      ? text.slice(1)
      : null
  if (raw === null) return null
  return raw
    .replace(/\s*#\s*(skip|todo)\b.*$/i, '')
    .replace(/\s*\(\s*\d+(\.\d+)?\s*m?s\s*\)\s*$/, '')
    .trim()
}

/**
 * Read one named assertion out of the EXECUTED proof output.
 *
 * Only a pass/fail MARKER line counts: a name that appears on a suite header, a summary line or echoed
 * source was not a run assertion, so it stays absent. A failure outranks a pass, so a clause whose mapped
 * assertion ran and failed is FAIL and never UNPROVEN.
 */
export function assertionOutcome(output: string | null | undefined, ref: string): AssertionOutcome {
  const trimmed = (ref ?? '').trim()
  if (!output || !trimmed) return 'absent'
  // A ref may be FILE-QUALIFIED (`path#name`, the Forge convention for a named thing inside a file).
  // A marker line carries the NAME, never the path, so resolve by the tail after the LAST `#`; a ref
  // with no `#` is the whole ref, exactly as before. The marker rule is untouched: only `markerLine`
  // decides, so a name echoed on a non-marker line stays absent.
  const hashAt = trimmed.lastIndexOf('#')
  const needle = (hashAt >= 0 ? trimmed.slice(hashAt + 1) : trimmed).trim()
  if (!needle) return 'absent'
  let sawPass = false
  let sawSkip = false
  for (const raw of output.split(/\r?\n/)) {
    // EXACT IDENTITY, NOT SUBSTRING (Astra review, 2026-09-18). The old test was
    // `if (!raw.includes(needle)) continue`, so a DIFFERENT test whose longer name merely contained the
    // required name satisfied the clause — reproduced: a proof whose required assertion never ran passed
    // because `…required assertion…` appeared inside some other test's name. A clause is proven by the
    // assertion it names, and by nothing else.
    const name = markerName(raw)
    if (name !== needle) continue
    const verdict = markerLine(raw)
    if (verdict === 'failed') return 'failed'
    if (verdict === 'passed') sawPass = true
    else if (verdict === 'skipped') sawSkip = true
  }
  if (sawPass) return 'passed'
  return sawSkip ? 'skipped' : 'absent'
}

/**
 * Adjudicate the negative control. Its RED is the required outcome.
 *
 * At least one INTENDED assertion must report `failed` in the control's EXECUTED output. A control
 * that ran and killed nothing proves the fence has no power, so its clause is UNPROVEN. A control
 * that could not RUN (spawn error or timeout) is `unmeasurable` — "we could not check" is never a
 * kill. Only a marker line counts, so a compile error (which prints no marker) yields no failed
 * assertion and cannot masquerade as a discriminating control.
 */
export function adjudicateNegativeControl(input: {
  control: NegativeControl
  result: CommandResult | null | undefined
  conditions: readonly AcceptanceCondition[]
}): {
  outcome: NegativeControlOutcome
  missing: boolean
  unmeasurable: boolean
  survived: boolean
} {
  const intended = input.control.assertions?.length
    ? input.control.assertions
    : input.conditions.flatMap((condition) => condition.assertions ?? [])
  const refs = [...new Set(intended.map((ref) => ref.trim()).filter(Boolean))]
  if (!input.result) {
    return {
      outcome: { command: input.control.command, ran: false, unmeasurable: false, killingAssertions: [] },
      missing: true,
      unmeasurable: false,
      survived: false,
    }
  }
  if (input.result.unmeasurable) {
    return {
      outcome: { command: input.control.command, ran: false, unmeasurable: true, killingAssertions: [] },
      missing: false,
      unmeasurable: true,
      survived: false,
    }
  }
  const killingAssertions = refs.filter(
    (ref) => assertionOutcome(input.result?.output, ref) === 'failed',
  )
  return {
    outcome: {
      command: input.control.command,
      ran: true,
      unmeasurable: false,
      killingAssertions,
    },
    missing: false,
    unmeasurable: false,
    survived: killingAssertions.length === 0,
  }
}

export function runAssayCommands(plan: AssayPlan, run: RunCommand): CommandResult[] {
  return plan.commands.map((command) => {
    const result = run(command)
    // `unmeasurable` travels with the result: the record must be able to say "could not run" rather than
    // implying the tests ran and failed, and dropping it here would put that back to guessing.
    const unmeasurable = result.unmeasurable === true
    return {
      command,
      exitCode: result.exitCode,
      passed: unmeasurable ? false : result.exitCode === 0 && result.passed,
      ...(unmeasurable ? { unmeasurable: true } : {}),
      excerpt: excerpt(result.excerpt),
      // THE EXECUTED OUTPUT TRAVELS TOO. The adjudicator reads it to check that a mapped assertion ran;
      // dropping it here would make every mapped clause read as UNPROVEN.
      ...(typeof result.output === 'string' ? { output: result.output } : {}),
    }
  })
}

/**
 * Deterministic Assay. No model, no git, no promotion advice.
 *
 * PASS only when at least one frozen command ran, every command that ran exited 0, AND every acceptance
 * condition had an assertion behind it. A command that could not run is a failure of the run (and its own
 * `unmeasurable` flag says why on the row). A condition with no assertion is UNPROVEN: the proof passed,
 * but it did not assert that clause, so the verdict names the clause instead of passing it.
 *
 * Nothing else is consulted: QA answers one question — did the tests pass the story's own acceptance — and
 * writes down what it measured.
 */
export function adjudicateAssay(input: {
  plan: AssayPlan
  commands: CommandResult[]
  staticGate?: StaticSlice | null
  /**
   * The mapping QA was HANDED, frozen before the work. When the plan's conditions no longer hash to it, a
   * lane changed what is tested and that is recorded (`ACCEPTANCE_MAP_CHANGED`) — never silently re-derived.
   */
  frozenMap?: AcceptanceMap | null
  /**
   * The result of running the plan's declared negative control, when there is one. It is adjudicated
   * SEPARATELY from `commands` because its non-zero exit is the EXPECTED outcome: the fence is
   * supposed to go red. Absent while the plan declares a control is a wiring failure, not a pass.
   */
  negativeControlResult?: CommandResult | null
}): QaReport {
  const blockers: string[] = []
  // AN EMPTY PLAN IS A FAILURE. QA is reached only after a Smith produced work, on a story
  // that carries its own frozen proofs, so no commands means upstream is broken — and a broken upstream is
  // named, not passed.
  if (input.plan.commands.length === 0) blockers.push('NO_ASSAY_COMMANDS')
  if (input.commands.length !== input.plan.commands.length) {
    blockers.push('ASSAY_COMMAND_DRIFT')
  } else {
    // THE SET, NOT THE COUNT. Two runs with the same NUMBER of commands but different
    // identities are not the same evidence. The plan is ordered and `runAssayCommands` pairs
    // results to it positionally, so a result whose `command` does not match its planned
    // command is a SUBSTITUTION — named here, never accepted as the same proof.
    for (let i = 0; i < input.plan.commands.length; i++) {
      const actual = input.commands[i]?.command
      if (actual !== input.plan.commands[i]) {
        blockers.push(`ASSAY_COMMAND_SUBSTITUTED ${actual ?? input.plan.commands[i]}`)
      }
    }
  }

  const failed = input.commands.filter((c) => !c.passed)
  if (failed.length) {
    blockers.push(
      ...failed.map((c) =>
        c.unmeasurable ? `CMD_UNMEASURABLE ${c.command}` : `CMD_FAIL ${c.command}`,
      ),
    )
  }

  const arch = input.staticGate
  if (arch && arch.archRan && !arch.archOk) {
    blockers.push(...arch.archErrors.slice(0, 8).map((e) => `ARCH ${e}`))
  }

  // THE MIGRATION HARD GATE. An unsafe statement about to hit a live database must refuse the candidate,
  // and a required migration check that could not RUN is never a pass. This is read from the static slice
  // precisely because the adapter dropping these fields is how a FAIL used to disappear before the verdict.
  const migrationFailure = arch?.migrationOk === false
  if (migrationFailure && arch) {
    if (arch.migrationRan !== true) {
      blockers.push(
        `MIGRATION_UNMEASURABLE ${arch.migrationFindings?.[0] ?? 'migration lint did not run'}`,
      )
    } else {
      const named = arch.migrationFindings?.length
        ? arch.migrationFindings
        : (arch.migrationRules ?? [])
      if (named.length === 0) blockers.push('MIGRATION migration lint reported a failure')
      else blockers.push(...named.slice(0, 8).map((finding) => `MIGRATION ${finding}`))
    }
  }

  // THE ACCEPTANCE, NOT THE CHEAPEST READING OF IT. A condition is satisfied only when one of its mapped
  // assertions actually RAN in the executed proof output. A name that is merely LISTED proves nothing: if it
  // never appears in the output the clause is UNPROVEN and NAMED, and if it ran and failed the clause is
  // FAIL — never UNPROVEN, because the proof did speak about it. A command that could not run contributed no
  // output, so it can never make an assertion look like it ran.
  const proofOutput = input.commands
    .filter((c) => c.unmeasurable !== true)
    .map((c) => c.output ?? '')
    .filter((text) => text.length > 0)
    .join('\n')

  const conditions = input.plan.conditions ?? []
  const unproven: string[] = []
  const failedConditions: string[] = []
  const missingAssertions: Array<{ conditionId: string; assertion: string }> = []
  for (const condition of conditions) {
    const refs = condition.assertions ?? []
    if (refs.length === 0) {
      unproven.push(condition.id)
      blockers.push(`UNPROVEN ${condition.id}`)
      continue
    }
    const outcomes = refs.map((ref) => ({ ref, outcome: assertionOutcome(proofOutput, ref) }))
    const failed = outcomes.filter((entry) => entry.outcome === 'failed')
    if (failed.length > 0) {
      failedConditions.push(condition.id)
      for (const entry of failed) blockers.push(`ASSERTION_FAILED ${condition.id} ${entry.ref}`)
      continue
    }
    if (outcomes.some((entry) => entry.outcome === 'passed')) continue
    // No mapped assertion appears in the executed output: a named-but-unrun assertion proves nothing, so
    // the clause is UNPROVEN and BOTH the clause and the missing assertion are named.
    unproven.push(condition.id)
    blockers.push(`UNPROVEN ${condition.id}`)
    for (const entry of outcomes) {
      // A SKIPPED assertion appeared in the output and said it did not run; it is NAMED as skipped, never
      // reported as absent. Either way the clause is UNPROVEN and can never PASS.
      if (entry.outcome === 'skipped') {
        blockers.push(`ASSERTION_SKIPPED ${condition.id} ${entry.ref}`)
        continue
      }
      missingAssertions.push({ conditionId: condition.id, assertion: entry.ref })
      blockers.push(`ASSERTION_NOT_RUN ${condition.id} ${entry.ref}`)
    }
  }

  // A lane that changes the mapping after it was frozen is RECORDED as having done so.
  if (input.frozenMap && acceptanceMapChanged(input.frozenMap, conditions)) {
    blockers.push('ACCEPTANCE_MAP_CHANGED')
  }

  // THE NEGATIVE CONTROL. A fence that has never been shown to fail is not proof. The control's own
  // non-zero exit is EXPECTED (the fence goes red under the inversion), so it is adjudicated apart
  // from `commands` and can never become a CMD_FAIL. What it must NOT do is survive: a control that
  // killed no intended assertion leaves the fence unproven.
  const negative = input.plan.negativeControl
    ? adjudicateNegativeControl({
        control: input.plan.negativeControl,
        result: input.negativeControlResult,
        conditions,
      })
    : null
  if (negative) {
    if (negative.missing) blockers.push(`NEGATIVE_CONTROL_MISSING ${negative.outcome.command}`)
    else if (negative.unmeasurable)
      blockers.push(`NEGATIVE_CONTROL_UNMEASURABLE ${negative.outcome.command}`)
    else if (negative.survived)
      blockers.push(`NEGATIVE_CONTROL_SURVIVED ${negative.outcome.command}`)
  }

  // A real command/static failure outranks an unmapped clause (there is nothing to prove either way), but
  // the UNPROVEN blockers above are still recorded so the gap is never hidden behind the failure.
  const commandFailure = blockers.some(
    (b) =>
      b.startsWith('CMD_') ||
      b.startsWith('ASSAY_COMMAND_SUBSTITUTED') ||
      b.startsWith('ARCH ') ||
      b === 'NO_ASSAY_COMMANDS' ||
      b === 'ASSAY_COMMAND_DRIFT',
  )
  // A control that could not run, or was declared but never handed a result, is a FAILURE of the run.
  const negativeFailure = negative !== null && (negative.missing || negative.unmeasurable)
  // A control that RAN and killed nothing is UNPROVEN: the proof passed, but the fence was never shown
  // to discriminate, so PASS is refused.
  const negativeSurvived = negative !== null && negative.survived
  // A mapped assertion that ran and FAILED is a FAIL in its own right: the proof ran, and it said no.
  const verdict: QaVerdict =
    commandFailure || migrationFailure || failedConditions.length > 0 || negativeFailure
      ? 'FAIL'
      : unproven.length || blockers.includes('ACCEPTANCE_MAP_CHANGED') || negativeSurvived
        ? 'UNPROVEN'
        : blockers.length
          ? 'FAIL'
          : 'PASS'

  return {
    version: 1,
    verdict,
    commands: input.commands,
    staticGate: arch ?? null,
    blockers,
    unproven,
    failedConditions,
    missingAssertions,
    ...(negative ? { negativeControl: negative.outcome } : {}),
  }
}

export function runAssay(input: {
  plan: AssayPlan
  runCommand: RunCommand
  runStatic?: RunStatic
  frozenMap?: AcceptanceMap | null
}): QaReport {
  const commands = runAssayCommands(input.plan, input.runCommand)
  const staticGate = input.runStatic ? input.runStatic() : null
  // THE CONTROL IS RUN WITH THE SAME RUNNER, separately from the frozen proofs, because its red is
  // the required outcome. It is never appended to `commands`.
  const negativeControlResult = input.plan.negativeControl
    ? input.runCommand(input.plan.negativeControl.command)
    : undefined
  return adjudicateAssay({
    plan: input.plan,
    commands,
    staticGate,
    ...(input.frozenMap !== undefined ? { frozenMap: input.frozenMap } : {}),
    ...(negativeControlResult !== undefined ? { negativeControlResult } : {}),
  })
}
