import type { QueryExecutor, QueryRow } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type ForgePhaseArtifact = QueryRow & {
  id: string
  story_id: string
  story_run_id: string | null
  process_instance_id: string | null
  node_id: string
  role: string
  phase: string | null
  sha: string | null
  model: string | null
  verdict: string | null
  raw_output: string | null
  structured: unknown
  created_at: unknown
}

export type RecordPhaseArtifactInput = {
  storyId: string
  storyRunId?: string | null
  processInstanceId?: string | null
  nodeId: string
  role: string
  phase?: string | null
  sha?: string | null
  model?: string | null
  verdict?: string | null
  /** Full model/packet output — the durable token-spend record. */
  rawOutput?: string | null
  /** Structured summary (findings, etc.) — bounded, never secrets. */
  structured?: Record<string, unknown> | null
}

/** Persist one completed role phase so a future run/reviewer can re-read it. */
export async function recordPhaseArtifact(
  input: RecordPhaseArtifactInput,
  execute?: QueryExecutor,
): Promise<ForgePhaseArtifact> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_phase_artifact (
      story_id, story_run_id, process_instance_id, node_id, role, phase, sha, model, verdict, raw_output, structured
    ) values (
      ${input.storyId}, ${input.storyRunId ?? null}, ${input.processInstanceId ?? null},
      ${input.nodeId}, ${input.role}, ${input.phase ?? null}, ${input.sha ?? null},
      ${input.model ?? null}, ${input.verdict ?? null}, ${input.rawOutput ?? null},
      ${input.structured ? JSON.stringify(input.structured) : null}::jsonb
    )
    returning id, story_id, story_run_id, process_instance_id, node_id, role, phase,
      sha, model, verdict, raw_output, structured, created_at
  `
  return rows[0] as ForgePhaseArtifact
}

/** The full phase history for a story, newest first — the reviewable ledger. */
export async function listPhaseArtifactsForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgePhaseArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, process_instance_id, node_id, role, phase,
      sha, model, verdict, raw_output, structured, created_at
    from forge_phase_artifact
    where story_id = ${storyId}
    order by created_at desc
  `
  return rows as ForgePhaseArtifact[]
}

export async function listPhaseArtifactsForRun(
  storyRunId: string,
  execute?: QueryExecutor,
): Promise<ForgePhaseArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, process_instance_id, node_id, role, phase,
      sha, model, verdict, raw_output, structured, created_at
    from forge_phase_artifact
    where story_run_id = ${storyRunId}
    order by created_at desc
  `
  return rows as ForgePhaseArtifact[]
}
