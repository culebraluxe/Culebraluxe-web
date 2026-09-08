import type { QueryExecutor, QueryRow } from './query-executor'

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
