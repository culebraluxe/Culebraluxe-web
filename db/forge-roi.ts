// ---------------------------------------------------------------------------
// FORGE ROI read model (Phase 4, Object 4) — NO new meter, NO new table.
//
// One query joins the finished work items (which carry the Phase 1 `kind` and `model_policy`, plus
// `started_at`/`finished_at`) to the run they produced (which carries the run's own `result_status` and the
// existing `cost_widgets` from migration 133). Everything the packet asks to "persist on finish" is already
// persisted in those two rows; this module just reads it back in the shape the rollup needs.
//
// The `left join` is deliberate: an item with no run still counts as an attempt, and its cost is null
// rather than zero. Reporting a missing cost as 0 would make coverage look complete, and coverage is the
// number that tells the captain how much of the rollup he can believe.
// ---------------------------------------------------------------------------

import { summarizeRoi, type RoiAttempt, type RoiSummary } from '../lib/forge-roi'
import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** The packet's window: "last 7 days". */
export const ROI_DEFAULT_WINDOW_DAYS = 7

/** Bounded so a bad argument cannot ask the database for every row ever finished. */
const MAX_WINDOW_DAYS = 90

function isoOrNull(value: unknown): string | null {
  if (value == null) return null
  if (value instanceof Date) return value.toISOString()
  const parsed = Date.parse(String(value))
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString()
}

/** `numeric` arrives from the driver as a string; the boundary normalizes it, callers never see it. */
function numberOrNull(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

export function mapRoiAttempt(row: Record<string, unknown>): RoiAttempt {
  return {
    kind: row.kind == null ? null : String(row.kind),
    modelPolicy: row.model_policy == null ? null : String(row.model_policy),
    state: String(row.state),
    startedAt: isoOrNull(row.started_at),
    finishedAt: isoOrNull(row.finished_at),
    resultStatus: row.result_status == null ? null : String(row.result_status),
    costWidgets: numberOrNull(row.cost_widgets),
  }
}

/**
 * Finished attempts inside the window, newest first.
 *
 * "Finished" means `finished_at is not null`, which covers Done, Error and Cancelled alike: a rollup that
 * only counted successes would report the cheap policy's failures as free.
 */
export async function listRoiAttempts(
  days: number = ROI_DEFAULT_WINDOW_DAYS,
  execute?: QueryExecutor,
): Promise<RoiAttempt[]> {
  const q = execute ?? (await executor())
  const window = Math.min(MAX_WINDOW_DAYS, Math.max(1, Math.trunc(days) || ROI_DEFAULT_WINDOW_DAYS))
  const rows = (await q`
    select w.kind, w.model_policy, w.state, w.started_at, w.finished_at,
           r.result_status, r.cost_widgets
    from agent_work_item w
    left join storyboard_story_run r on r.id = w.story_run_id
    where w.finished_at is not null
      and w.finished_at >= now() - (${window} || ' days')::interval
    order by w.finished_at desc
  `) as Record<string, unknown>[]
  return rows.map(mapRoiAttempt)
}

/** The strip's data: the rollup over the window. */
export async function listKindRoi(
  days: number = ROI_DEFAULT_WINDOW_DAYS,
  execute?: QueryExecutor,
): Promise<RoiSummary> {
  const attempts = await listRoiAttempts(days, execute)
  return summarizeRoi(attempts, Math.min(MAX_WINDOW_DAYS, Math.max(1, Math.trunc(days) || ROI_DEFAULT_WINDOW_DAYS)))
}
