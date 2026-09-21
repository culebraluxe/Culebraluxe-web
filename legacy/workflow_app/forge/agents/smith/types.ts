/**
 * Smith lane types.
 *
 * The door constants have ONE definition — the live `forge-serial-doors` module —
 * and are re-exported here for the agent layer rather than restated.
 */
import { NO_ASSIGNMENT_REASON, SCOPE_MISS_PREFIX, SERIAL_SMITH_NODES } from '@/legacy/workflow_app/forge/forge-serial-doors'

export { NO_ASSIGNMENT_REASON, SCOPE_MISS_PREFIX, SERIAL_SMITH_NODES }

/** Lead assignment as Smith sees it. Smith does not invent this. */
export type SmithChunk = {
  id: number
  preconditions: string[]
  scope: string[]
  postconditions: string[]
  classes: string[]
  risks: string[]
  proof: string
}

export type SmithAssignment = {
  id: string
  findingIds: string[]
  chunks: SmithChunk[]
  /** Union of chunk.scope — the lock. */
  allowedScope: string[]
  prohibitedScope: string[]
}

export const SMITH_CANDIDATE_PREFIX = 'SMITH_CANDIDATE:'

export type SmithCandidate = {
  version: 1
  assignmentId: string
  /** Exact commit Smith produced. */
  candidateSha: string
  /** Merge-base Smith diffed against. */
  mergeBase: string
  changedPaths: string[]
}
