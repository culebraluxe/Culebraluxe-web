/**
 * THE DISPATCH LEDGER (migration 175) — prediction in, outcome out.
 *
 * `forge-difficulty-scorer.ts` predicts p_success from eight features with hand-set weights
 * and says plainly that those weights are "provisional calibration food", to be replaced by
 * a fit learned from run history. It cannot be fitted, because until now the features, the
 * prediction and the gate verdict were computed at dispatch and discarded, and the outcome
 * was never joined to the unit that was assessed.
 *
 * This module writes both halves. RECORD is what the model saw and said; OUTCOME is what
 * happened to that same unit. A row with no outcome is a prediction awaiting its label, not
 * a failure — `null` means unmeasured, never zero.
 */
import type { QueryExecutor } from './query-executor'
import type { DifficultyFeatures } from '../workflow_app/forge/forge-difficulty-scorer'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type ForgeDispatchScoreInput = {
  storyId: string
  processInstanceId: string
  taskId: string
  nodeId: string
  attempt: number
  assignmentId: string
  /** 0 means "the assignment itself". */
  chunkId?: number
  features: DifficultyFeatures
  /** Which of the eight were measured rather than defaulted. */
  measured: ReadonlyArray<keyof DifficultyFeatures>
  scorerId: string
  logit: number
  pSuccess: number
  gate: 'reject' | 'flag' | 'dispatch'
  route?: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null
}

/** Record (or re-record) the prediction for one assessed unit. Idempotent per identity. */
export async function recordForgeDispatchScore(
  input: ForgeDispatchScoreInput,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const f = input.features
  await q`
    insert into forge_dispatch_score (
      story_id, process_instance_id, task_id, node_id, attempt, assignment_id, chunk_id,
      files_touched, loc_ratio, dep_depth, has_acceptance, context_ratio, historical_success,
      repo_size_bucket, generic_type, measured_features,
      scorer_id, logit, p_success, gate, route
    ) values (
      ${input.storyId}, ${input.processInstanceId}, ${input.taskId}, ${input.nodeId},
      ${input.attempt}, ${input.assignmentId}, ${input.chunkId ?? 0},
      ${f.filesTouched}, ${f.locRatio}, ${f.depDepth}, ${f.hasAcceptance}, ${f.contextRatio},
      ${f.historicalSuccess}, ${f.repoSizeBucket}, ${f.genericType},
      ${input.measured.map((m) => String(m))},
      ${input.scorerId}, ${input.logit}, ${input.pSuccess}, ${input.gate}, ${input.route ?? null}
    )
    on conflict (task_id, node_id, attempt, assignment_id, chunk_id) do update set
      files_touched = excluded.files_touched,
      loc_ratio = excluded.loc_ratio,
      dep_depth = excluded.dep_depth,
      has_acceptance = excluded.has_acceptance,
      context_ratio = excluded.context_ratio,
      historical_success = excluded.historical_success,
      repo_size_bucket = excluded.repo_size_bucket,
      generic_type = excluded.generic_type,
      measured_features = excluded.measured_features,
      scorer_id = excluded.scorer_id,
      logit = excluded.logit,
      p_success = excluded.p_success,
      gate = excluded.gate,
      route = coalesce(excluded.route, forge_dispatch_score.route),
      updated_at = now()
  `
}

export type ForgeDispatchOutcomeInput = {
  taskId: string
  nodeId: string
  attempt: number
  assignmentId: string
  chunkId?: number
  outcome: 'pass' | 'fail' | 'repair' | 'hold' | 'cancelled'
  repairs?: number | null
  turns?: number | null
  wallMs?: number | null
  costUsd?: number | null
  tokensInput?: number | null
  tokensOutput?: number | null
  filesChanged?: number | null
  locDelta?: number | null
  candidateSha?: string | null
  detail?: string | null
}

/**
 * Fill the outcome for an already-recorded prediction.
 *
 * Returns false when no prediction row exists — a fact about the ledger (an outcome for a
 * unit nobody scored, e.g. dispatched before this table existed), and the caller decides
 * whether to care. Deliberately not an exception: a missing label must never fail the run
 * that produced it.
 */
export async function recordForgeDispatchOutcome(
  input: ForgeDispatchOutcomeInput,
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`
    update forge_dispatch_score set
      outcome = ${input.outcome},
      repairs = coalesce(${input.repairs ?? null}, repairs),
      turns = coalesce(${input.turns ?? null}, turns),
      wall_ms = coalesce(${input.wallMs ?? null}, wall_ms),
      cost_usd = coalesce(${input.costUsd ?? null}, cost_usd),
      tokens_input = coalesce(${input.tokensInput ?? null}, tokens_input),
      tokens_output = coalesce(${input.tokensOutput ?? null}, tokens_output),
      files_changed = coalesce(${input.filesChanged ?? null}, files_changed),
      loc_delta = coalesce(${input.locDelta ?? null}, loc_delta),
      candidate_sha = coalesce(${input.candidateSha ?? null}, candidate_sha),
      outcome_detail = coalesce(${input.detail ?? null}, outcome_detail),
      updated_at = now()
    where task_id = ${input.taskId}
      and node_id = ${input.nodeId}
      and attempt = ${input.attempt}
      and assignment_id = ${input.assignmentId}
      and chunk_id = ${input.chunkId ?? 0}
    returning id
  `
  return rows.length > 0
}

export type ForgeDispatchScoreRow = {
  storyId: string
  processInstanceId: string
  nodeId: string
  assignmentId: string
  features: DifficultyFeatures
  /** Which of the eight were measured rather than defaulted. */
  measured: string[]
  scorerId: string
  logit: number
  pSuccess: number
  gate: string
  route: string | null
  outcome: string | null
  repairs: number | null
  turns: number | null
  createdAt: string
}

/**
 * The calibration set: predictions, optionally only those that carry a label.
 *
 * An empty slice is the honest answer when the ledger is young, so a fit that runs before
 * it has data reports emptiness rather than inventing a relationship.
 */
export async function listForgeDispatchScores(
  input: { storyId?: string; labelledOnly?: boolean; limit?: number } = {},
  execute?: QueryExecutor,
): Promise<ForgeDispatchScoreRow[]> {
  const q = execute ?? (await executor())
  const limit = Math.min(1000, Math.max(1, input.limit ?? 200))
  const rows = await q`
    select story_id, process_instance_id, node_id, assignment_id, scorer_id, logit, p_success,
           gate, route, outcome, repairs, turns, created_at, measured_features,
           files_touched, loc_ratio, dep_depth, has_acceptance, context_ratio,
           historical_success, repo_size_bucket, generic_type
    from forge_dispatch_score
    where (${input.storyId ?? null}::text is null or story_id = ${input.storyId ?? null})
      and (${input.labelledOnly === true} = false or outcome is not null)
    order by created_at desc
    limit ${limit}
  `
  return rows.map((raw) => {
    const row = raw as Record<string, unknown>
    return {
      storyId: String(row.story_id),
      processInstanceId: String(row.process_instance_id),
      nodeId: String(row.node_id),
      assignmentId: String(row.assignment_id),
      features: {
        filesTouched: Number(row.files_touched),
        locRatio: Number(row.loc_ratio),
        depDepth: Number(row.dep_depth),
        hasAcceptance: row.has_acceptance === true,
        contextRatio: Number(row.context_ratio),
        historicalSuccess: Number(row.historical_success),
        repoSizeBucket: Number(row.repo_size_bucket) as 0 | 1 | 2,
        genericType: row.generic_type === true,
      },
      measured: Array.isArray(row.measured_features) ? (row.measured_features as string[]) : [],
      scorerId: String(row.scorer_id),
      logit: Number(row.logit),
      pSuccess: Number(row.p_success),
      gate: String(row.gate),
      route: row.route == null ? null : String(row.route),
      outcome: row.outcome == null ? null : String(row.outcome),
      repairs: row.repairs == null ? null : Number(row.repairs),
      turns: row.turns == null ? null : Number(row.turns),
      createdAt:
        row.created_at instanceof Date
          ? row.created_at.toISOString()
          : String(row.created_at ?? ''),
    }
  })
}

