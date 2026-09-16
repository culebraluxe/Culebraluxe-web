/**
 * ADD. Deterministic Assay. Uses the adjudicator in ./qa/run (PASS / FAIL /
 * INCOMPLETE — a verdict about the STORY's tests, never a condition on a SHA)
 * and reports what it measured. No model verdict, ever.
 */
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { adjudicateAssay, runAssayCommands } from './qa/run'

/**
 * THE QA VERDICT: DID THE TESTS PASS.
 *
 * QA has no relationship to git and no role in committing, promoting or advising on a release. It runs the
 * story's frozen proofs in the directory it was given, and it writes down what they did. Everything that
 * used to sit beside that — a candidate SHA, a promotion check, a lineage conjunct, a "we could not measure
 * it" third verdict — is gone, because every one of them could turn a green test run into a HOLD.
 *
 * The evidence is the row (see the run/evidence writers): the verdict, each command, its exit code, and
 * whether it could run at all.
 */
export function collectAssayEvidence(evidence: ForgeGateEvidence, ports: RoleEffectPorts): ForgeGateEvidence {
  const commands = ports.assayCommands ?? []

  // NO RUNNER IS A FAILURE, not a pass. The lane was handed a plan it cannot execute.
  if (typeof ports.runCommand !== 'function') {
    return {
      ...evidence,
      qaPassed: false,
      deliverableRejection: 'QA FAIL: the lane was handed assay commands but no way to run them.',
    }
  }

  // AN EMPTY PLAN IS A FAILURE
  // In a working system QA cannot be reached without a Smith having produced work and without the story
  // carrying its frozen proofs, so no commands means something upstream is broken. The adjudicator reports
  // it as a FAIL with `NO_ASSAY_COMMANDS` and the branch below records that reason on the row.
  const plan = { commands }
  const results = runAssayCommands(plan, ports.runCommand)
  const report = adjudicateAssay({ plan, commands: results, staticGate: ports.runStatic?.() ?? null })

  if (report.verdict !== 'PASS') {
    // Only commands that RAN and failed are "failed commands"; a command that could not run is named
    // separately so the record never implies the tests ran when they did not.
    const failed = results.filter((r) => !r.passed && !r.unmeasurable).map((r) => r.command)
    const failedDetail = results
      .filter((r) => !r.passed && !r.unmeasurable)
      .map((r) => `${r.command} -> exit ${r.exitCode}: ${r.excerpt}`)
      .join(' | ')
    const couldNotRun = results
      .filter((r) => r.unmeasurable)
      .map((r) => `${r.command} -> could not run: ${r.excerpt}`)
      .join(' | ')
    return {
      ...evidence,
      qaPassed: false,
      ...(failed.length ? { failedCommands: failed } : {}),
      // WHAT IT SAID, NOT JUST THAT IT FAILED — a refusal that does not quote the failing command's own
      // output cannot be diagnosed from the log, only re-derived by hand.
      deliverableRejection:
        `QA FAIL: blockers=[${report.blockers.join(', ') || 'none'}] ` +
        `failed=[${failed.join(' | ') || 'none'}]` +
        (failedDetail ? ` || output=[${failedDetail}]` : '') +
        (couldNotRun ? ` || couldNotRun=[${couldNotRun}]` : ''),
    }
  }

  return { ...evidence, qaPassed: true }
}

