/**
 * ADD. Injected effects. Nothing in this package spawns a process, reads env,
 * or touches the DB. The runner supplies these.
 */
import type { AcceptanceMap, NegativeControl } from './qa/types'

export type BenchIntent = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null

export type RunnerDiff = {
  candidateSha: string
  mergeBase: string
  changedPaths: string[]
}

export type CommandResult = {
  command: string
  exitCode: number
  passed: boolean
  excerpt: string
  /**
   * THE EXECUTED OUTPUT beyond the 240-char excerpt, so the adjudicator can check that a mapped assertion
   * actually ran. The excerpt stays the bounded field that gets stored as evidence.
   */
  output?: string
}

export type StaticSlice = {
  archRan: boolean
  archOk: boolean
  archErrors: string[]
  /** Ride-along instruments: recorded, never verdict-flipping. */
  semgrepFindings?: string[]
  knipFindings?: string[]
  /**
   * Migration safety (squawk). A HARD gate like architecture: a false `migrationOk` must reach the QA
   * verdict, not vanish at this adapter. Optional so a slice that predates the field stays valid.
   * `migrationRan=false` with `migrationOk=false` is a check that could not run and fails closed.
   */
  migrationRan?: boolean
  migrationOk?: boolean
  migrationFindings?: string[]
  migrationRules?: string[]
}

export type ReleaseEvidencePort = {
  kind: 'deployment' | 'integration' | 'production_verification'
  success: boolean
  receiptId: string
  artifactSha: string | null
}

export type RoleEffectPorts = {
  existsOnBaseRef?: (baseRef: string, repoPath: string) => boolean
  runnerDiff?: RunnerDiff
  runCommand?: (command: string) => CommandResult
  runStatic?: () => StaticSlice
  /** Frozen assay commands from the story / accepted assignment. */
  assayCommands?: string[]
  /**
   * The acceptance-to-assertion mapping, frozen BEFORE the work by the Architect or the Lead.
   *
   * QA CONSUMES this; it never authors it — the lane that did the work does not get to choose what is
   * tested. ABSENT means the story's acceptance was never mapped, which QA reports as UNPROVEN (never a
   * pass): there is no assertion behind any clause.
   */
  acceptanceMap?: AcceptanceMap
  /**
   * The story's declared negative control, if any. QA runs `command` as the SAME fence with the
   * claimed behaviour withheld or inverted and requires at least one intended assertion to go red.
   * ABSENT preserves today's semantics exactly; a control that kills nothing makes the verdict
   * UNPROVEN, and one that cannot run is a FAIL.
   */
  negativeControl?: NegativeControl
  /**
   * NOT part of the required contract. Bench membership does NOT imply a launch
   * cap — every non-null value here is a cap, so deriving one would ban SPLIT for
   * every active story. It stays optional and unsupplied until Push-to-Line writes
   * an explicit `launch_intent` column (NULL | SOLO | SMITH | SPLIT | HOLD);
   * NULL means today's Lead behaviour.
   */
  benchIntent?: BenchIntent
  splitEnabled?: boolean
  maxSmiths?: number
  allowedProofs?: string[]
  evidenceRefs?: string[]
  repoDir?: string
  releaseEvidence?: ReleaseEvidencePort | null
  deploymentRequired?: boolean
  /** Batch-sliced story: recorded deferral, not a fake receipt. */
  deploymentDeferredToBatch?: number | null
  baseRef?: string
}
