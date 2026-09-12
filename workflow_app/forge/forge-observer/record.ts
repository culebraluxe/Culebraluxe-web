// ---------------------------------------------------------------------------
// Typed record helpers. Call sites should not build raw TraceEvent objects.
// ---------------------------------------------------------------------------

import { filesOf, type TraceEvent, type TraceIdentity, type TraceSink } from './types'

export type RecordBase = TraceIdentity

function emit(
  sink: TraceSink,
  base: RecordBase,
  rest: Omit<TraceEvent, keyof TraceIdentity | 'seq' | 'atIso'> & { atIso?: string },
): TraceEvent {
  return sink.append({
    ...base,
    ...rest,
    paths: filesOf(rest.paths),
  })
}

export function recordRunStart(sink: TraceSink, base: RecordBase, detail?: TraceEvent['detail']): TraceEvent {
  return emit(sink, base, { kind: 'run.start', detail })
}

export function recordGitCommit(
  sink: TraceSink,
  base: RecordBase,
  input: { sha: string | null; paths: string[]; changed: boolean },
): TraceEvent {
  return emit(sink, base, {
    kind: 'git.commit',
    sha: input.sha ?? undefined,
    paths: input.paths,
    verdict: input.changed ? 'allow' : 'watch',
    reason: input.changed ? undefined : 'worktree still at base; no worker commit',
  })
}

export function recordScopeCheck(
  sink: TraceSink,
  base: RecordBase,
  input: { sha: string; paths: string[]; violations: string[] },
): TraceEvent {
  const deny = input.violations.length > 0
  return emit(sink, base, {
    kind: 'scope.check',
    sha: input.sha,
    paths: deny ? input.violations : input.paths,
    verdict: deny ? 'deny' : 'allow',
    reason: deny
      ? `candidate ${input.sha.slice(0, 12)} touched files outside assignment: ${input.violations.join(', ')}`
      : undefined,
    detail: { violationCount: input.violations.length, pathCount: input.paths.length },
  })
}

export function recordHold(
  sink: TraceSink,
  base: RecordBase,
  input: { reasons: string[]; sha?: string; retryHash?: string | null },
): TraceEvent {
  return emit(sink, base, {
    kind: 'hold',
    sha: input.sha,
    verdict: 'deny',
    reason: input.reasons.join('; '),
    detail: {
      reasonCount: input.reasons.length,
      // RETRY_UNCHANGED_INPUT reads this (forge-alerts/rules.ts). Only set when a
      // real miss list produced it: a constant hash on every HOLD would make two
      // unrelated HOLDs look like a repeated no-op retry.
      ...(input.retryHash ? { retryHash: input.retryHash } : {}),
    },
  })
}

export function recordRunEnd(
  sink: TraceSink,
  base: RecordBase,
  input: { status: 'completed' | 'failed' | 'interrupted'; sha?: string },
): TraceEvent {
  return emit(sink, base, {
    kind: 'run.end',
    sha: input.sha,
    verdict: input.status === 'completed' ? 'allow' : 'watch',
    detail: { status: input.status },
  })
}

/** Observer may record an alert event after Alerts evaluates. It does not fire alerts. */
export function recordAlert(
  sink: TraceSink,
  base: RecordBase,
  input: { code: string; severity: string; reason: string },
): TraceEvent {
  return emit(sink, base, {
    kind: 'alert',
    verdict: input.severity === 'hold-recommend' ? 'deny' : 'watch',
    reason: `${input.code}: ${input.reason}`,
    detail: { code: input.code, severity: input.severity },
  })
}

/**
 * Hash the retry input so Alerts can detect "retry did not change the input".
 * Stable, order-insensitive on miss reasons.
 */
export function retryInputHash(input: {
  missReasons: string[]
  packetHash?: string | null
  assignmentId?: string | null
}): string {
  const misses = [...input.missReasons].map((s) => s.trim()).filter(Boolean).sort()
  return [
    misses.join('|') || 'none',
    (input.packetHash ?? '').trim() || 'no-packet',
    (input.assignmentId ?? '').trim() || 'no-assignment',
  ].join('::')
}
