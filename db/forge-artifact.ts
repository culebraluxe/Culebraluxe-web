import type { QueryExecutor, QueryRow } from './query-executor'
import type { AssayEvidence } from '../agent-runtime/assay-evidence'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type ForgeToolArtifact = QueryRow & {
  id: string
  story_id: string
  story_run_id: string | null
  tool: string
  kind: string
  verdict: string | null
  summary: string | null
  sha: string | null
  created_at: unknown
}

export type RecordToolArtifactInput = {
  storyId: string
  storyRunId?: string | null
  tool: string
  kind: string
  verdict?: string | null
  summary?: string | null
  /** Optional machine detail (e.g. findings list) — small, bounded, never secrets. */
  detail?: Record<string, unknown> | null
  sha?: string | null
}

/** One flat child row keyed to a story run (the execution unit). */
export async function recordToolArtifact(
  input: RecordToolArtifactInput,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_tool_artifact (story_id, story_run_id, tool, kind, verdict, summary, detail, sha)
    values (
      ${input.storyId}, ${input.storyRunId ?? null}, ${input.tool}, ${input.kind},
      ${input.verdict ?? null}, ${input.summary ?? null},
      ${input.detail ? JSON.stringify(input.detail) : null}::jsonb, ${input.sha ?? null}
    )
    returning id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
  `
  return rows[0] as ForgeToolArtifact
}

export async function listToolArtifactsForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
    from forge_tool_artifact
    where story_id = ${storyId}
    order by created_at desc
  `
  return rows as ForgeToolArtifact[]
}

export async function listToolArtifactsForRun(
  storyRunId: string,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
    from forge_tool_artifact
    where story_run_id = ${storyRunId}
    order by created_at desc
  `
  return rows as ForgeToolArtifact[]
}

/**
 * Persist the QA assay verdict AND ITS REASON as a child artifact.
 *
 * The AssayEvidence already carries the complete diagnosis (verdict, failureCode,
 * failureDetail, policyViolations, per-command exit codes/tails), but until now only
 * the derived booleans (qa_passed/failure_class) reached the database — so a QA FAIL
 * could not be explained after the fact. That cost real time: WS-05 failed QA four
 * times with CODE_DEFECT while all three frozen commands passed locally in the same
 * worktree at the same SHA, and there was no durable record of WHY.
 *
 * Best-effort by design: a failed write must never change the QA verdict.
 */
export async function recordAssayEvidenceArtifact(
  input: {
    storyId: string
    storyRunId?: string | null
    evidence: AssayEvidence
  },
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const { evidence } = input
  const reason =
    evidence.verdict === 'PASS'
      ? 'every frozen command passed'
      : evidence.failureCode ??
        evidence.policyViolations[0] ??
        evidence.failureDetail ??
        'assay failed without a recorded code'
  return recordToolArtifact(
    {
      storyId: input.storyId,
      storyRunId: input.storyRunId ?? null,
      tool: 'assay',
      kind: 'qa-assay-evidence',
      verdict: evidence.verdict,
      summary: `${evidence.verdict}: ${reason}`.slice(0, 1000),
      detail: {
        verdict: evidence.verdict,
        failureCode: evidence.failureCode,
        failureDetail: evidence.failureDetail,
        candidateSha: evidence.candidateSha,
        verifiedSha: evidence.verifiedSha,
        candidateMatchesVerified: evidence.candidateSha === evidence.verifiedSha,
        requiredCommands: evidence.requiredCommands,
        policyViolations: evidence.policyViolations.slice(0, 8),
        commandResults: evidence.commandResults.slice(0, 12).map((result) => ({
          command: result.command,
          exitCode: result.exitCode,
          signal: result.signal,
          timedOut: result.timedOut,
          durationMs: result.durationMs,
          tests: result.tests,
          stderrTail: result.stderrTail.slice(-400),
        })),
        startedAt: evidence.startedAt,
        endedAt: evidence.endedAt,
      },
      sha: evidence.verifiedSha ?? evidence.candidateSha ?? null,
    },
    execute,
  )
}

/** Persist a static-gate verdict (from runStaticGate) as a child artifact. */
export async function recordStaticGateArtifact(
  input: {
    storyId: string
    storyRunId?: string | null
    sha?: string | null
    archOk: boolean
    archErrorCount: number
    semgrepRan: boolean
    semgrepFindingCount: number
    workspace?: string | null
  },
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const summary =
    `${input.archOk ? 'architecture clean' : `${input.archErrorCount} architecture violation(s)`}` +
    (input.semgrepRan
      ? `; semgrep ${input.semgrepFindingCount === 0 ? 'clean' : `${input.semgrepFindingCount} finding(s)`}`
      : '')
  return recordToolArtifact(
    {
      storyId: input.storyId,
      storyRunId: input.storyRunId ?? null,
      tool: 'static-gate',
      kind: 'architecture-security',
      verdict: input.archOk ? 'PASS' : 'FAIL',
      summary,
      detail: {
        archOk: input.archOk,
        archErrorCount: input.archErrorCount,
        semgrepRan: input.semgrepRan,
        semgrepFindingCount: input.semgrepFindingCount,
        workspace: input.workspace ?? null,
      },
      sha: input.sha ?? null,
    },
    execute,
  )
}
