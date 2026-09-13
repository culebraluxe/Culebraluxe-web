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
  if (!commands.length || typeof ports.runCommand !== 'function') {
    return { ...evidence, qaPassed: false, verificationGap: true }
  }

  const plan = { candidateSha: candidate ?? '', commands }
  const results = runAssayCommands(plan, ports.runCommand)
  const report = adjudicateAssay({ plan, commands: results, staticGate: ports.runStatic?.() ?? null })

  if (report.verdict !== 'PASS') {
    const failed = results.filter((r) => !r.passed).map((r) => r.command)
    return {
      ...evidence,
      qaPassed: false,
      verificationGap: report.verdict === 'INCOMPLETE',
      ...(failed.length ? { failedCommands: failed } : {}),
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
    return { ...evidence, qaPassed: false, verificationGap: promo.blockers.includes('NO_CANDIDATE') }
  }
  return { ...evidence, qaPassed: true, qaVerifiedSha: report.verifiedSha, verificationGap: false }
}

