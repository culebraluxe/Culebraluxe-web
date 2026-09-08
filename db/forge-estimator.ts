import {
  estimateWork,
  type ModelGrade,
  type WorkComplexity,
  type WorkRisk,
  type WorkToolkit,
} from '../workflow_app/forge/forge-estimator'
import {
  applySurfaceMultiplier,
  ripwireSurfaceFromPack,
  surfaceMultiplier,
} from '../workflow_app/forge/forge-ripwire-surface'
import type { QueryExecutor, QueryRow } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type WorkEstimateInput = {
  storyId: string
  estimatorRole?: string | null
  modelGrade: ModelGrade
  workstream: string
  complexity: WorkComplexity
  toolkit: WorkToolkit
  seamsCount: number
  acceptanceCount: number
  /** Optional ripwire pack (V2) — its measured surface corrects the prior forecast. */
  ripwirePack?: string | null
}

export type WorkEstimateRow = QueryRow & {
  id: string
  points: number
  estimated_tokens: number
  estimated_cost_usd: number
  estimated_minutes: number
  estimated_sloc: number
  estimated_risk: WorkRisk
  status: string
  surface_score: number | null
  surface_files: number | null
  surface_ccx: number | null
}

/** Persist a forecast for a story. Actuals are filled later from the run. */
export async function createWorkEstimate(
  input: WorkEstimateInput,
  execute?: QueryExecutor,
): Promise<WorkEstimateRow> {
  const q = execute ?? (await executor())
  const prior = estimateWork({
    workstream: input.workstream,
    complexity: input.complexity,
    toolkit: input.toolkit,
    modelGrade: input.modelGrade,
    seamsCount: input.seamsCount,
    acceptanceCount: input.acceptanceCount,
  })

  // V2: correct the prior with ripwire's measured surface when a pack exists.
  const surface = input.ripwirePack ? ripwireSurfaceFromPack(input.ripwirePack) : null
  const score = surface ? surfaceMultiplier(surface) : 1
  const forecast = surface ? applySurfaceMultiplier(prior, score) : prior

  const rows = await q`
    insert into work_estimate (
      story_id, estimator_role, model_grade, workstream, complexity, toolkit,
      seams_count, acceptance_count, points, estimated_tokens, estimated_cost_usd,
      estimated_minutes, estimated_sloc, estimated_risk, status,
      surface_score, surface_files, surface_ccx
    ) values (
      ${input.storyId}, ${input.estimatorRole ?? null}, ${input.modelGrade}, ${input.workstream},
      ${input.complexity}, ${input.toolkit}, ${input.seamsCount}, ${input.acceptanceCount},
      ${forecast.points}, ${forecast.estimatedTokens}, ${forecast.estimatedCostUsd},
      ${forecast.estimatedMinutes}, ${forecast.estimatedSloc}, ${forecast.risk}, 'forecast',
      ${score}, ${surface?.files ?? null}, ${surface?.ccxTotal ?? null}
    )
    returning id, points, estimated_tokens, estimated_cost_usd, estimated_minutes,
      estimated_sloc, estimated_risk, status, surface_score, surface_files, surface_ccx
  `
  return rows[0] as WorkEstimateRow
}

export async function listWorkEstimates(
  storyId: string,
  execute?: QueryExecutor,
): Promise<WorkEstimateRow[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, points, estimated_tokens, estimated_cost_usd, estimated_minutes,
      estimated_sloc, estimated_risk, status
    from work_estimate
    where story_id = ${storyId}
    order by created_at desc
  `
  return rows as WorkEstimateRow[]
}

/** Real actuals already recorded on this story's runs (tokens / cost / minutes),
 * the calibration half of the estimator. Half the data is better than none. */
export async function runActualTotalsForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<{ tokens: number; costUsd: number; minutes: number } | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select coalesce(sum(coalesce(tokens_input,0) + coalesce(tokens_output,0)), 0)::int as tokens,
           coalesce(sum(cost_usd), 0) as cost_usd,
           coalesce(sum(extract(epoch from (coalesce(ended_at, started_at) - started_at)) / 60), 0) as minutes
    from storyboard_story_run
    where story_id = ${storyId}
  `
  const row = rows[0] as { tokens: number; cost_usd: number; minutes: number } | undefined
  if (!row) return null
  return { tokens: Number(row.tokens), costUsd: Number(row.cost_usd), minutes: Number(row.minutes) }
}
