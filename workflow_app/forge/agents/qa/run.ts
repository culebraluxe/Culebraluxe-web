import type { AssayPlan, CommandResult, QaReport, StaticSlice } from './types'

export type RunCommand = (command: string) => CommandResult
export type RunStatic = () => StaticSlice

const excerpt = (s: string) => s.replace(/\s+/g, ' ').trim().slice(0, 240)

export function runAssayCommands(plan: AssayPlan, run: RunCommand): CommandResult[] {
  return plan.commands.map((command) => {
    const result = run(command)
    // `unmeasurable` travels with the result: the adjudicator must be able to tell "could not run" from
    // "ran and failed", and dropping it here would put that distinction back to guessing.
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
 * Deterministic Assay. No model.
 *
 * PASS only when:
 *   - at least one frozen command ran
 *   - every command exited 0
 *   - architecture hard gate is ok (or was skipped — skipped is NOT a silent PASS of arch,
 *     but it also must not FAIL the story; recorded on the report)
 *   - evaluatedSha is non-empty
 *
 * Missing commands => INCOMPLETE, never PASS.
 */
export function adjudicateAssay(input: {
  plan: AssayPlan
  commands: CommandResult[]
  staticGate?: StaticSlice | null
}): QaReport {
  const blockers: string[] = []
  if (!input.plan.candidateSha) blockers.push('NO_CANDIDATE')
  if (!input.plan.commands.length) blockers.push('NO_ASSAY_COMMANDS')
  if (input.commands.length !== input.plan.commands.length) blockers.push('ASSAY_COMMAND_DRIFT')

  const failed = input.commands.filter((c) => !c.unmeasurable && !c.passed)
  if (failed.length) blockers.push(...failed.map((c) => `CMD_FAIL ${c.command}`))

  // COULD NOT RUN is a VERIFICATION GAP, not a test failure. A command that never started (bad cwd,
  // missing toolchain, timeout kill) says nothing about the candidate, and recording it as CMD_FAIL is
  // what made QA look flaky and too strict: it failed a proof that passes 11/11 when it can actually be
  // run. The gap is named per command so the reason is readable, and the verdict is INCOMPLETE — the same
  // "we could not check" outcome the empty-plan branch already uses.
  const unmeasurable = input.commands.filter((c) => c.unmeasurable)
  if (unmeasurable.length) {
    blockers.push(...unmeasurable.map((c) => `CMD_UNMEASURABLE ${c.command}`))
  }

  const arch = input.staticGate
  if (arch && arch.archRan && !arch.archOk) {
    blockers.push(...arch.archErrors.slice(0, 8).map((e) => `ARCH ${e}`))
  }

  const incomplete =
    blockers.includes('NO_CANDIDATE') ||
    blockers.includes('NO_ASSAY_COMMANDS') ||
    unmeasurable.length > 0
  const verdict: QaReport['verdict'] = incomplete ? 'INCOMPLETE' : blockers.length ? 'FAIL' : 'PASS'

  return {
    version: 1,
    evaluatedSha: input.plan.candidateSha,
    verifiedSha: verdict === 'PASS' ? input.plan.candidateSha : null,
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
