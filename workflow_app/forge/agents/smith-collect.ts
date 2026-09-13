/**
 * ADD. Uses the live serial doors AND the Lead-assignment scope lock. The path
 * rules have ONE definition (`shared/path.ts`); this module never re-implements
 * prefix matching.
 */
import { serialLaunchDoor, serialScopeMissReasons } from '../forge-serial-doors'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { pathAllowed } from './shared/path'
import { assignmentFromLead } from './smith/from-lead'
import { assessSmithScope } from './smith/scope'

type RoutingAssignment = {
  id?: string
  findingIds?: string[]
  plan?: { chunks?: Array<{ id?: number; scope?: string[]; proof?: string }> }
}

export function collectSmithEvidence(
  nodeId: string,
  evidence: ForgeGateEvidence,
  ports: RoleEffectPorts,
  hasAcceptedAssignment: boolean,
): ForgeGateEvidence {
  const door = serialLaunchDoor({ nodeId, hasAcceptedAssignment })
  if (!door.allowed) {
    // No accepted assignment => no launch. Record WHY (the parent's
    // smith-candidate gate HOLDs either way, but a bare HOLD teaches nothing).
    return door.reason ? { ...evidence, deliverableRejection: door.reason } : evidence
  }

  const diff = ports.runnerDiff
  if (!diff?.candidateSha) return evidence

  const routingAssignment = ((evidence.leadRouting as { assignments?: RoutingAssignment[] } | undefined)
    ?.assignments ?? [])[0]
  const declaredScope = (routingAssignment?.plan?.chunks ?? []).flatMap((c) => c.scope ?? [])
  // Split children and serial smith both lock on declared seams when the Lead
  // declared no chunk scope at all.
  const seamFallback = (evidence.findings ?? []).filter((f) => f.required).flatMap((f) => f.seams)

  if (declaredScope.length) {
    // The accepted assignment's own chunk scopes ARE the lock, judged by the Lead
    // assignment adapter + scope assessor (shared with the split lane's rules).
    const assignment = assignmentFromLead({
      id: routingAssignment?.id ?? nodeId,
      findingIds: routingAssignment?.findingIds ?? [],
      chunks: (routingAssignment?.plan?.chunks ?? []).map((c, i) => ({
        id: c.id ?? i + 1,
        scope: c.scope ?? [],
        proof: c.proof ?? '',
      })),
    })
    const scope = assessSmithScope({ assignment, changedPaths: diff.changedPaths })
    if (!scope.ok) {
      // Do not set candidateSha — the parent's smith-candidate gate then HOLDs.
      // The scope-miss reasons are RECORDED rather than discarded, so the
      // self-heal reprompt can name the file that fell outside the assignment.
      return { ...evidence, deliverableRejection: scope.reasons.join('; ') }
    }
  } else if (seamFallback.length) {
    const hits = diff.changedPaths.filter((path) => !pathAllowed(path, seamFallback, []))
    if (hits.length) {
      return {
        ...evidence,
        deliverableRejection: serialScopeMissReasons(hits, nodeId).join('; '),
      }
    }
  }

  return { ...evidence, candidateSha: diff.candidateSha.toLowerCase() }
}

