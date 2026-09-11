// ---------------------------------------------------------------------------
// Forge Execution Trace — event contract (ENG-FORGE-OBSERVER-01).
//
// Inspired by Shepherd (Yu et al., arXiv 2605.10913): treat a run as data.
// This module is NOT Shepherd. It does not fork workers, does not check out
// traces as a second workflow, and does not talk to OpenCode.
//
// Every event is append-only. STATE still lives in WorkflowEngine + Neon
// evidence. The observer may recommend; it may not completeTask.
// ---------------------------------------------------------------------------

export type TraceEventKind =
  | 'run.start'
  | 'tool.intend'
  | 'tool.result'
  | 'fs.write'
  | 'git.commit'
  | 'scope.check'
  | 'run.end'
  | 'hold'
  | 'alert'

export type TraceVerdict = 'allow' | 'deny' | 'watch'

export type TraceIdentity = {
  storyId: string
  processInstanceId: string
  taskId: string
  nodeId: string
  /** 1-based, matching SmithExecutionContract.identity.attempt */
  attempt: number
  worktreePath: string
  baseCommit: string
}

export type TraceEvent = TraceIdentity & {
  seq: number
  atIso: string
  kind: TraceEventKind
  tool?: string
  /** File-level paths (strip #symbol before record — same as scope overlap). */
  paths?: string[]
  sha?: string
  verdict?: TraceVerdict
  reason?: string
  /** Opaque payload: miss list, packet hash, assignment id, etc. */
  detail?: Record<string, string | number | boolean | null>
}

export type TraceSink = {
  append(event: Omit<TraceEvent, 'seq' | 'atIso'> & { atIso?: string }): TraceEvent
  list(storyId: string): TraceEvent[]
}

export function fileOf(path: string): string {
  return path.split('#')[0].replace(/\/+$/g, '')
}

export function filesOf(paths: readonly string[] | undefined): string[] {
  if (!paths?.length) return []
  return [...new Set(paths.map(fileOf).filter(Boolean))]
}
