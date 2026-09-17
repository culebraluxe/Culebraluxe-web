import type {
  AcceptanceMap,
  AssayPlan,
  CommandResult,
  QaReport,
  QaVerdict,
  StaticSlice,
} from './types'
import { acceptanceMapChanged } from './types'

export type RunCommand = (command: string) => CommandResult
export type RunStatic = () => StaticSlice

const excerpt = (s: string) => s.replace(/\s+/g, ' ').trim().slice(0, 240)

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
}): QaReport {
  const blockers: string[] = []
  // AN EMPTY PLAN IS A FAILURE. QA is reached only after a Smith produced work, on a story
  // that carries its own frozen proofs, so no commands means upstream is broken — and a broken upstream is
  // named, not passed.
  if (input.plan.commands.length === 0) blockers.push('NO_ASSAY_COMMANDS')
  if (input.commands.length !== input.plan.commands.length) blockers.push('ASSAY_COMMAND_DRIFT')

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

  // THE ACCEPTANCE, NOT THE CHEAPEST READING OF IT. A condition with no assertion behind it can never be a
  // PASS, and the condition is NAMED so the report says WHICH clause went untested.
  const conditions = input.plan.conditions ?? []
  const unproven = conditions.filter((c) => c.assertions.length === 0).map((c) => c.id)
  if (unproven.length) blockers.push(...unproven.map((id) => `UNPROVEN ${id}`))

  // A lane that changes the mapping after it was frozen is RECORDED as having done so.
  if (input.frozenMap && acceptanceMapChanged(input.frozenMap, conditions)) {
    blockers.push('ACCEPTANCE_MAP_CHANGED')
  }

  // A real command/static failure outranks an unmapped clause (there is nothing to prove either way), but
  // the UNPROVEN blockers above are still recorded so the gap is never hidden behind the failure.
  const commandFailure = blockers.some(
    (b) =>
      b.startsWith('CMD_') ||
      b.startsWith('ARCH ') ||
      b === 'NO_ASSAY_COMMANDS' ||
      b === 'ASSAY_COMMAND_DRIFT',
  )
  const verdict: QaVerdict = commandFailure
    ? 'FAIL'
    : unproven.length || blockers.includes('ACCEPTANCE_MAP_CHANGED')
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
  return adjudicateAssay({
    plan: input.plan,
    commands,
    staticGate,
    ...(input.frozenMap !== undefined ? { frozenMap: input.frozenMap } : {}),
  })
}
