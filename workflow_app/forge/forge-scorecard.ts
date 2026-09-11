// ---------------------------------------------------------------------------
// Forge run-effectiveness scorecard — the first Maestro idea pirated.
//
// Read-only, computed from durable facts only (storyboard_story_run,
// agent_work_item, workflow_execution_trace_event). No new store. No model
// self-report. It never gates anything.
//
// Two honesty rules (Maestro review, "measure before optimizing"):
//  1. A metric with no data reports null, never 0. Zero must mean measured zero.
//  2. A column that is not populated yet is reported as NOT CAPTURED, so nobody
//     reads a missing number as a good number.
// ---------------------------------------------------------------------------

import { sql } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'

const WINDOW_DAYS = 30

const rate = (part: number, whole: number): number | null =>
  whole > 0 ? Number((part / whole).toFixed(4)) : null
const num = (v: unknown, digits = 1): number | null =>
  v === null || v === undefined ? null : Number(Number(v).toFixed(digits))
const int = (v: unknown): number => Number(v ?? 0)

export type Scorecard = {
  windowDays: number
  outcomes: {
    runs: number
    complete: number
    hold: number
    failed: number
    interrupted: number
    cleanCompletionRate: number | null
    holdRate: number | null
    failureRate: number | null
    avgMinutes: number | null
    testsPassed: number
    testsFailed: number
  }
  routes: Array<{ route: string; runs: number }>
  splitHealth: { splitRuns: number; children: number; withCandidateSha: number }
  roles: Array<{
    role: string
    items: number
    errorRate: number | null
    avgAttempts: number | null
    avgMinutes: number | null
  }>
  observerEvents: Array<{ eventType: string; events: number }>
  telemetry: { runsWithTokens: number; runsWithCost: number; note: string }
}

export async function computeForgeScorecard(
  execute: QueryExecutor = sql,
  windowDays: number = WINDOW_DAYS,
): Promise<Scorecard> {
  const w = `${windowDays} days`

  const runs = (await execute`
    select
      count(*)::int as runs,
      count(*) filter (where result_status = 'Complete')::int as complete,
      count(*) filter (where result_status = 'Hold')::int as hold,
      count(*) filter (where result_status = 'Failed')::int as failed,
      count(*) filter (where result_status = 'Interrupted')::int as interrupted,
      count(*) filter (where tokens_input is not null or tokens_output is not null)::int as with_tokens,
      count(*) filter (where cost_usd is not null)::int as with_cost,
      coalesce(sum(tests_passed), 0)::int as tests_passed,
      coalesce(sum(tests_failed), 0)::int as tests_failed,
      avg(extract(epoch from (ended_at - started_at)) / 60.0) as avg_minutes
    from storyboard_story_run
    where started_at > now() - ${w}::interval
  `) as Array<Record<string, unknown>>

  const routes = (await execute`
    select coalesce(lead_decision, '(none)') as route, count(*)::int as runs
    from storyboard_story_run
    where started_at > now() - ${w}::interval
    group by 1 order by runs desc, route
  `) as Array<{ route: string; runs: number }>

  const split = (await execute`
    select
      count(*) filter (where lead_decision = 'SPLIT')::int as split_runs,
      coalesce(sum(lead_split_count), 0)::int as children
    from storyboard_story_run
    where started_at > now() - ${w}::interval
  `) as Array<{ split_runs: number; children: number }>

  const children = (await execute`
    select count(*)::int as with_candidate_sha
    from agent_work_item
    where role = 'smith'
      and candidate_shas is not null
      and candidate_shas::text not in ('', '[]', 'null')
      and created_at > now() - ${w}::interval
  `) as Array<{ with_candidate_sha: number }>

  const roles = (await execute`
    select
      role,
      count(*)::int as items,
      count(*) filter (where state = 'Error')::int as errors,
      avg(attempts) as avg_attempts,
      avg(extract(epoch from (finished_at - started_at)) / 60.0) as avg_minutes
    from agent_work_item
    where created_at > now() - ${w}::interval
    group by role order by items desc, role
  `) as Array<Record<string, unknown>>

  const observerEvents = (await execute`
    select event_type as event_type, count(*)::int as events
    from workflow_execution_trace_event
    where source_system = 'forge_observer'
      and occurred_at > now() - ${w}::interval
    group by 1 order by events desc, 1
  `) as Array<{ event_type: string; events: number }>

  const r = runs[0] ?? {}
  const total = int(r.runs)
  const withTokens = int(r.with_tokens)
  const withCost = int(r.with_cost)

  return {
    windowDays,
    outcomes: {
      runs: total,
      complete: int(r.complete),
      hold: int(r.hold),
      failed: int(r.failed),
      interrupted: int(r.interrupted),
      cleanCompletionRate: rate(int(r.complete), total),
      holdRate: rate(int(r.hold), total),
      failureRate: rate(int(r.failed), total),
      avgMinutes: num(r.avg_minutes),
      testsPassed: int(r.tests_passed),
      testsFailed: int(r.tests_failed),
    },
    routes,
    splitHealth: {
      splitRuns: int(split[0]?.split_runs),
      children: int(split[0]?.children),
      withCandidateSha: int(children[0]?.with_candidate_sha),
    },
    roles: roles.map((row) => ({
      role: String(row.role),
      items: int(row.items),
      errorRate: rate(int(row.errors), int(row.items)),
      avgAttempts: num(row.avg_attempts, 2),
      avgMinutes: num(row.avg_minutes),
    })),
    observerEvents: observerEvents.map((row) => ({ eventType: row.event_type, events: row.events })),
    telemetry: {
      runsWithTokens: withTokens,
      runsWithCost: withCost,
      note:
        withTokens > 0 || withCost > 0
          ? 'token/cost columns carry data'
          : 'NOT CAPTURED: tokens_input/tokens_output/cost_usd are null on every run in this window',
    },
  }
}
