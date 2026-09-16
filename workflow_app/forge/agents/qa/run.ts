import type { AssayPlan, CommandResult, QaReport, QaVerdict, StaticSlice } from './types'

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
 * PASS only when at least one frozen command ran and every command that ran exited 0. A command that could
 * not run is a failure of the run (and its own `unmeasurable` flag says why on the row). Nothing else is
 * consulted: QA answers one question — did the tests pass — and writes down what it measured.
 */
export function adjudicateAssay(input: {
  plan: AssayPlan
  commands: CommandResult[]
  staticGate?: StaticSlice | null
}): QaReport {
  const blockers: string[] = []
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

  const verdict: QaVerdict = blockers.length ? 'FAIL' : 'PASS'

  return {
    version: 1,
    verdict,
    commands: input.commands,
    staticGate: arch ?? null,
    blockers,
  }
}

export function runAssay(input: {
  plan: AssayPlan
  runCommand: RunCommand
  runStatic?: RunStatic
}): QaReport {
  const commands = runAssayCommands(input.plan, input.runCommand)
  const staticGate = input.runStatic ? input.runStatic() : null
  return adjudicateAssay({ plan: input.plan, commands, staticGate })
}
