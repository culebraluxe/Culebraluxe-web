/**
 * ADD. Exit gate around the live Smith doors.
 * Parse lives in smith-candidate-parse.ts so the line reader has no git/door imports.
 */
import { serialLaunchDoor, serialScopeMissReasons, SERIAL_SMITH_NODES } from '@/legacy/workflow_app/forge/forge-serial-doors'
import { scopeViolations, type SmithExecutionContract } from '@/legacy/workflow_app/forge/smith-contract'
import {
  SMITH_CANDIDATE_MISSING,
  type SmithCandidate,
} from '@/legacy/workflow_app/forge/smith-candidate-parse'

export {
  parseSmithCandidate,
  SMITH_CANDIDATE_MISSING,
  SMITH_CANDIDATE_PREFIX,
  type SmithCandidate,
} from '@/legacy/workflow_app/forge/smith-candidate-parse'

export type SmithExit =
  | { ok: true; candidate: SmithCandidate }
  | { ok: false; reasons: string[]; candidate: SmithCandidate | null }

export function assessSmithLaunch(input: {
  nodeId: string
  hasAcceptedAssignment: boolean
}): SmithExit | { ok: true; launched: true } {
  const door = serialLaunchDoor(input)
  if (!door.allowed) return { ok: false, reasons: [door.reason ?? 'HOLD'], candidate: null }
  return { ok: true, launched: true }
}

export function assessSmithExit(input: {
  nodeId: string
  assignmentId: string
  contract: SmithExecutionContract | null
  notes: string | null
  runnerDiff?: { candidateSha: string; mergeBase: string; changedPaths: string[] }
}): SmithExit {
  if ((SERIAL_SMITH_NODES as readonly string[]).includes(input.nodeId)) {
    const launch = serialLaunchDoor({
      nodeId: input.nodeId,
      hasAcceptedAssignment: Boolean(input.assignmentId && input.contract),
    })
    if (!launch.allowed) return { ok: false, reasons: [launch.reason ?? 'HOLD'], candidate: null }
  }

  // Git is the cabinet. A SMITH_CANDIDATE chat line cannot invent or veto a SHA.
  if (!input.runnerDiff) {
    return { ok: false, reasons: [SMITH_CANDIDATE_MISSING], candidate: null }
  }

  const candidate: SmithCandidate = {
    version: 1,
    assignmentId: input.assignmentId,
    candidateSha: input.runnerDiff.candidateSha.toLowerCase(),
    mergeBase: input.runnerDiff.mergeBase.toLowerCase(),
    changedPaths: input.runnerDiff.changedPaths,
  }
  if (candidate.changedPaths.length === 0) {
    return { ok: false, reasons: ['Smith produced no diff against merge base'], candidate }
  }

  if (input.contract) {
    const hits = scopeViolations(input.contract, candidate.changedPaths)
    if (hits.length) {
      return { ok: false, reasons: serialScopeMissReasons(hits, input.assignmentId), candidate }
    }
  }

  return { ok: true, candidate }
}
