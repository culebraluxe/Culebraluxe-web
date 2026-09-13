/**
 * ADD. Exit gate around the live Smith doors.
 * Parse lives in smith-candidate-parse.ts so the line reader has no git/door imports.
 */
import { serialLaunchDoor, serialScopeMissReasons, SERIAL_SMITH_NODES } from './forge-serial-doors'
import { scopeViolations, type SmithExecutionContract } from './smith-contract'
import {
  parseSmithCandidate,
  SMITH_CANDIDATE_MISSING,
  type SmithCandidate,
} from './smith-candidate-parse'

export {
  parseSmithCandidate,
  SMITH_CANDIDATE_MISSING,
  SMITH_CANDIDATE_PREFIX,
  type SmithCandidate,
} from './smith-candidate-parse'

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

  const parsed = parseSmithCandidate(input.notes)
  if (!parsed && !input.runnerDiff) {
    return { ok: false, reasons: [SMITH_CANDIDATE_MISSING], candidate: null }
  }

  const candidate: SmithCandidate = input.runnerDiff
    ? {
        version: 1,
        assignmentId: input.assignmentId,
        candidateSha: input.runnerDiff.candidateSha.toLowerCase(),
        mergeBase: input.runnerDiff.mergeBase.toLowerCase(),
        changedPaths: input.runnerDiff.changedPaths,
      }
    : parsed!

  if (parsed && parsed.assignmentId !== input.assignmentId) {
    return {
      ok: false,
      reasons: [`Smith claimed assignment ${parsed.assignmentId}, lane is ${input.assignmentId}`],
      candidate,
    }
  }
  if (input.runnerDiff && parsed && parsed.candidateSha !== candidate.candidateSha) {
    return {
      ok: false,
      reasons: [`Smith claimed SHA ${parsed.candidateSha} but worktree HEAD is ${candidate.candidateSha}`],
      candidate,
    }
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
