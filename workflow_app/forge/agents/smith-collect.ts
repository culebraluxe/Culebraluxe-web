/**
 * ADD. Uses live serial doors. Does not re-declare serialLaunchDoor.
 */
import { serialLaunchDoor, serialScopeMissReasons } from '../forge-serial-doors'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'

export function collectSmithEvidence(
  nodeId: string,
  evidence: ForgeGateEvidence,
  ports: RoleEffectPorts,
  hasAcceptedAssignment: boolean,
): ForgeGateEvidence {
  const door = serialLaunchDoor({ nodeId, hasAcceptedAssignment })
  if (!door.allowed) return evidence

  const diff = ports.runnerDiff
  if (!diff?.candidateSha) return evidence

  const allowed = ((evidence.leadRouting as { assignments?: Array<{ plan?: { chunks?: Array<{ scope?: string[] }> } }> } | undefined)
    ?.assignments ?? [])
    .flatMap((a) => a.plan?.chunks ?? [])
    .flatMap((c) => c.scope ?? [])

  // Split children and serial smith both lock on declared seams when present.
  const seams = (evidence.findings ?? []).filter((f) => f.required).flatMap((f) => f.seams)
  const lock = allowed.length ? allowed : seams
  if (lock.length) {
    const hits = diff.changedPaths.filter((path) => {
      const file = path.replace(/^\.\//, '')
      return !lock.some((area) => file === area || file.startsWith(`${area}/`) || file.startsWith(`${area.split('#')[0]}/`) || file === area.split('#')[0])
    })
    if (hits.length) {
      // Do not set candidateSha — parent missingDeliverables then HOLDs.
      void serialScopeMissReasons(hits, nodeId)
      return evidence
    }
  }

  return { ...evidence, candidateSha: diff.candidateSha.toLowerCase() }
}
