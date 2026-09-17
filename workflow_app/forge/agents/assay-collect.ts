/**
 * ADD. Deterministic Assay. Uses the adjudicator in ./qa/run (PASS / FAIL /
 * UNPROVEN — a verdict about the STORY's tests, never a condition on a SHA)
 * and reports what it measured. No model verdict, ever.
 */
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import type { AcceptanceCondition } from './qa/types'
import { adjudicateAssay, runAssayCommands } from './qa/run'

/**
 * THE QA VERDICT: DID THE TESTS PASS THE STORY'S OWN ACCEPTANCE.
 *
 * QA has no relationship to git and no role in committing, promoting or advising on a release. It runs the
 * story's frozen proofs in the directory it was given, checks that every acceptance condition has an
 * assertion behind it, and writes down what they did. Everything that used to sit beside that — a candidate
 * SHA, a promotion check, a lineage conjunct — is gone, because every one of them could turn a green test
 * run into a HOLD.
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
  //
  // AN ABSENT MAPPING IS NOT A PASS. If the story's acceptance was never mapped to the frozen proof there
  // is no assertion behind any clause, so the verdict is UNPROVEN with the missing mapping NAMED. The
  // collector never synthesises a mapping: the lane that did the work does not get to choose what is tested.
  const conditions: AcceptanceCondition[] = ports.acceptanceMap
    ? ports.acceptanceMap.conditions
    : commands.length > 0
      ? [
          {
            id: 'acceptance-map-missing',
            text: 'the acceptance-to-assertion mapping was not supplied',
            assertions: [],
          },
        ]
      : []

  const plan = { commands, conditions }
  const results = runAssayCommands(plan, ports.runCommand)
  const report = adjudicateAssay({
    plan,
    commands: results,
    staticGate: ports.runStatic?.() ?? null,
    frozenMap: ports.acceptanceMap ?? null,
  })

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
    const unproven = report.unproven ?? []
    return {
      ...evidence,
      qaPassed: false,
      ...(failed.length ? { failedCommands: failed } : {}),
      // WHAT IT SAID, NOT JUST THAT IT FAILED — a refusal that does not quote the failing command's own
      // output cannot be diagnosed from the log, only re-derived by hand. An UNPROVEN verdict names the
      // acceptance condition that had no assertion behind it, so the untested clause is on the row.
      deliverableRejection:
        `QA ${report.verdict}: blockers=[${report.blockers.join(', ') || 'none'}] ` +
        `failed=[${failed.join(' | ') || 'none'}]` +
        (failedDetail ? ` || output=[${failedDetail}]` : '') +
        (couldNotRun ? ` || couldNotRun=[${couldNotRun}]` : '') +
        (unproven.length ? ` || unproven=[${unproven.join(' | ')}]` : ''),
    }
  }

  return { ...evidence, qaPassed: true }
}

