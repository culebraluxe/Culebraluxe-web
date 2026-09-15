/**
 * ADD. Deterministic Assay. Uses the adjudicator in ./qa/run (PASS / FAIL /
 * INCOMPLETE, never a pass on an empty plan) and the live promotionEligibility.
 * No model verdict, ever.
 */
import { promotionEligibility } from '../evidence-gate'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { adjudicateAssay, runAssayCommands } from './qa/run'

export function collectAssayEvidence(evidence: ForgeGateEvidence, ports: RoleEffectPorts): ForgeGateEvidence {
  const commands = ports.assayCommands ?? []
  const candidate = evidence.candidateSha ?? null

  // No frozen commands, or no way to run them, is NOT a pass and is not a FAIL
  // either: it is a verification GAP (INCOMPLETE). Recording the gap is what
  // keeps "we could not check" distinguishable from "we checked and it broke".
  //
  // The reason rides `deliverableRejection` so the gap is READABLE: this lane used
  // to set the flag and say nothing, so a held run showed `verificationGap: true`
  // with an empty record and no way to tell which of the three branches fired.
  if (!commands.length || typeof ports.runCommand !== 'function') {
    return {
      ...evidence,
      qaPassed: false,
      verificationGap: true,
      deliverableRejection:
        `QA GAP: nothing to run. frozen commands=${commands.length} ` +
        `runner=${typeof ports.runCommand === 'function' ? 'supplied' : 'MISSING'}`,
    }
  }

  const plan = { candidateSha: candidate ?? '', commands }
  const results = runAssayCommands(plan, ports.runCommand)
  const report = adjudicateAssay({ plan, commands: results, staticGate: ports.runStatic?.() ?? null })

  if (report.verdict !== 'PASS') {
    // Only commands that RAN and failed are "failed commands". An unmeasurable one is a gap, and listing it
    // here would send repair after code that was never tested.
    const failed = results.filter((r) => !r.passed && !r.unmeasurable).map((r) => r.command)
    // WHAT IT SAID, NOT JUST THAT IT FAILED. On 2026-09-15 the engine recorded `CMD_FAIL <proof>` + repair
    // while this same run's evidence said `Assay verdict: PASS`, exit 0, 7/7 — so a SECOND execution of the
    // proof was failing somewhere this text did not name. A verdict that refuses without quoting the failing
    // command's own output cannot be diagnosed from the log, only re-derived by hand.
    const failedDetail = results
      .filter((r) => !r.passed && !r.unmeasurable)
      .map((r) => `${r.command} -> exit ${r.exitCode}: ${r.excerpt}`)
      .join(' | ')
    const gapDetail = results
      .filter((r) => r.unmeasurable)
      .map((r) => `${r.command} -> could not run: ${r.excerpt}`)
      .join(' | ')
    return {
      ...evidence,
      qaPassed: false,
      verificationGap: report.verdict === 'INCOMPLETE',
      ...(failed.length ? { failedCommands: failed } : {}),
      // Name the verdict and the adjudicator's own blockers. A FAIL and a gap are
      // different outcomes and the record must say which one happened and why.
      deliverableRejection:
        `QA ${report.verdict}: blockers=[${report.blockers.join(', ') || 'none'}] ` +
        `candidate=${candidate ?? 'MISSING'} verified=${report.verifiedSha ?? 'none'} ` +
        `failed=[${failed.join(' | ') || 'none'}]` +
        (failedDetail ? ` || output=[${failedDetail}]` : '') +
        (gapDetail ? ` || gaps=[${gapDetail}]` : ''),
    }
  }

  // PASS binds exactly one SHA: QA for candidate A must never certify candidate B.
  const promo = promotionEligibility({
    candidateSha: candidate,
    evidence: {
      evaluatedSha: candidate,
      qaVerdict: 'PASS',
      verifiedSha: report.verifiedSha,
      evidencePresent: true,
    },
  })
  if (!promo.eligible) {
    return {
      ...evidence,
      qaPassed: false,
      verificationGap: promo.blockers.includes('NO_CANDIDATE'),
      deliverableRejection:
        `QA PASS-BUT-NOT-PROMOTABLE: blockers=[${promo.blockers.join(', ') || 'none'}] ` +
        `candidate=${candidate ?? 'MISSING'} verified=${report.verifiedSha ?? 'none'}`,
    }
  }
  return { ...evidence, qaPassed: true, qaVerifiedSha: report.verifiedSha, verificationGap: false }
}

