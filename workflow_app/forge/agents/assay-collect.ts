/**
 * ADD. Deterministic Assay. Uses the adjudicator in ./qa/run (PASS / FAIL /
 * INCOMPLETE — a verdict about the STORY's tests, never a condition on a SHA)
 * and reports what it measured. No model verdict, ever.
 */
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { adjudicateAssay, runAssayCommands } from './qa/run'

/**
 * THE QA VERDICT: DID THE TESTS PASS (Captain, 2026-09-16).
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

  // NOTHING TO TEST IS NOT A FAILURE: "if there is nothing to test then go to sleep". No commands, or no
  // way to run them, is nothing to do — it does not block, and it does not claim a pass either. What
  // refuses a QA-applicable story with no assayable contract is the READY GATE, one step earlier.
  if (!commands.length || typeof ports.runCommand !== 'function') {
    return { ...evidence, qaPassed: true }
  }

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

