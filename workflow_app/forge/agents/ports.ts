/**
 * ADD. Injected effects. Nothing in this package spawns a process, reads env,
 * or touches the DB. The runner supplies these.
 */
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
}

export type StaticSlice = {
  archRan: boolean
  archOk: boolean
  archErrors: string[]
  /** Ride-along instruments: recorded, never verdict-flipping. */
  semgrepFindings?: string[]
  knipFindings?: string[]
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
