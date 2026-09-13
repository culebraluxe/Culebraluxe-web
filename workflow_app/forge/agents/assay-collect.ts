/**
 * ADD. Deterministic Assay. Imports live promotionEligibility. No model verdict.
 */
import { promotionEligibility } from '../evidence-gate'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'

export function collectAssayEvidence(evidence: ForgeGateEvidence, ports: RoleEffectPorts): ForgeGateEvidence {
  const commands = ports.assayCommands ?? []
  const candidate = evidence.candidateSha ?? null
  if (!commands.length || !ports.runCommand) {
    return { ...evidence, qaPassed: false, verificationGap: true }
  }

  const results = commands.map((command) => ports.runCommand!(command))
  const failed = results.filter((r) => !r.passed || r.exitCode !== 0)
  const staticGate = ports.runStatic?.()
  const archBlocked = Boolean(staticGate?.archRan && !staticGate.archOk)

  if (failed.length || archBlocked) {
    return {
      ...evidence,
      qaPassed: false,
      verificationGap: false,
      failedCommands: failed.map((r) => r.command),
    }
  }

  const verified = candidate
  const next: ForgeGateEvidence = {
    ...evidence,
    qaPassed: true,
    qaVerifiedSha: verified,
    verificationGap: false,
  }
  const promo = promotionEligibility({
    candidateSha: candidate,
    evidence: {
      evaluatedSha: candidate,
      qaVerdict: 'PASS',
      verifiedSha: verified,
      evidencePresent: true,
    },
  })
  if (!promo.eligible) {
    return { ...evidence, qaPassed: false, verificationGap: promo.blockers.includes('NO_CANDIDATE') }
  }
  return next
}
