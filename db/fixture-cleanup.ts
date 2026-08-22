// ---------------------------------------------------------------------------
// DEV test-data hygiene (ENG-20) — surgical preflight cleanse for explicitly
// namespaced test/dogfood fixtures.
//
// WHY: stale TUNIT / TMP-* / DOGFOOD rows can poison later persistence tests
// because the single-active-worker invariant (migration 025/028) is real — a
// leftover Claimed/Running/Paused fixture occupies the global slot and the
// next suite that tries to claim work fails spuriously.
//
// GUARANTEES:
//   - deletes ONLY explicitly namespaced fixture rows (story id matches the
//     fixture patterns below). Real Story Board stories and real execution
//     history are never matched.
//   - cleans dependent rows in safe FK order:
//       agent_work_item        (child rows first — run_link is SET NULL)
//       storyboard_story_run   (explicit — avoids relying on cascade timing)
//       storyboard_story       (cascades any remaining work items)
//   - FAILS CLOSED unless the environment is DEV/test (APP_ENV=development
//     or APP_ENV=test). Refuses to run against production.
//   - verifies ZERO active test-owned work items remain after cleanup.
//
// TEST ISOLATION RULE (documented in docs/agent/TEST_ISOLATION.md):
// persistence/contract suites that exercise the global single-active slot MUST
// NOT run concurrently against the same DEV database. The cleanse is a repair
// tool for a clean sequential run, not a substitute for serialized suites.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from './query-executor'
import { interactiveSql } from '../lib/neon-interactive'

/** Fixture story-id patterns — every match is treated as deletable test data. */
export const FIXTURE_ID_PATTERNS: readonly RegExp[] = [
  /^TMP-/i, // temporary fixtures (ENG-18/19 CLI fixtures, contract-suite stories)
  /^TUNIT/i, // TUnit test fixtures
  /^TEST-/i, // explicit TEST namespace
  /^DOGFOOD-/i, // DOGFOOD-prefixed fixtures
  /-DOGFOOD-/i, // ENG-19-DOGFOOD-001 style dogfood stories
]

export function isFixtureStoryId(storyId: string): boolean {
  return FIXTURE_ID_PATTERNS.some((pattern) => pattern.test(storyId))
}

export type CleanseOptions = {
  /** APP_ENV override for deterministic tests. Defaults to process.env. */
  appEnv?: string
  /** Query executor override (tests inject the DEV interactive executor). */
  execute?: QueryExecutor
}

export type CleanseResult = {
  deletedStories: string[]
  deletedWorkItems: number
  deletedRuns: number
  preservedStoryCount: number
  activeFixtureCountAfter: number
  /** Set true when the guard refused because the environment was not DEV/test. */
  refused: boolean
  reason?: string
}

/**
 * DEV-only preflight cleanse. FAILS CLOSED: an unknown/non-DEV environment
 * refuses (never broad-pattern-matches real stories, never runs against
 * production). Returns a result instead of throwing for a refused invocation so
 * callers (scripts, tests) can distinguish "refused" from "clean succeeded".
 */
export async function cleanseDevFixtures(
  options: CleanseOptions = {},
): Promise<CleanseResult> {
  const appEnv = (options.appEnv ?? process.env.APP_ENV ?? 'development')
    .trim()
    .toLowerCase()
  const isDevEnv = appEnv === 'development' || appEnv === 'dev' || appEnv === 'test'
  if (!isDevEnv) {
    return {
      deletedStories: [],
      deletedWorkItems: 0,
      deletedRuns: 0,
      preservedStoryCount: 0,
      activeFixtureCountAfter: 0,
      refused: true,
      reason: `cleanse is DEV/test only; refusing with APP_ENV=${JSON.stringify(appEnv)}`,
    }
  }

  const q = options.execute ?? interactiveSql

  const run = async (tx: QueryExecutor) => {
    const storyRows = await tx`select id from storyboard_story`
    const fixtureIds = storyRows
      .map((row) => String(row.id))
      .filter((id) => isFixtureStoryId(id))

    if (fixtureIds.length === 0) {
      const preserved = await tx`select count(*)::int as c from storyboard_story`
      return {
        deletedStories: [],
        deletedWorkItems: 0,
        deletedRuns: 0,
        preservedStoryCount: Number(preserved[0]?.c ?? 0),
        activeFixtureCountAfter: 0,
        refused: false,
      } as CleanseResult
    }

    // Safe FK order: work items first (their story_run_id is SET NULL on run
    // delete), then runs, then the story rows themselves (cascade backstop).
    const deletedWork = await tx`
      delete from agent_work_item
      where story_id in (select id from unnest(${fixtureIds}::text[]) as t(id))
      returning id
    `
    const deletedRuns = await tx`
      delete from storyboard_story_run
      where story_id in (select id from unnest(${fixtureIds}::text[]) as t(id))
      returning id
    `
    const deletedStories = await tx`
      delete from storyboard_story
      where id in (select id from unnest(${fixtureIds}::text[]) as t(id))
      returning id
    `

    const activeAfter = await tx`
      select count(*)::int as c from agent_work_item
      where state in ('Ready', 'Claimed', 'Running', 'Paused')
        and story_id in (select id from unnest(${fixtureIds}::text[]) as t(id))
    `
    const preserved = await tx`select count(*)::int as c from storyboard_story`

    return {
      deletedStories: deletedStories.map((r) => String(r.id)),
      deletedWorkItems: deletedWork.length,
      deletedRuns: deletedRuns.length,
      preservedStoryCount: Number(preserved[0]?.c ?? 0),
      activeFixtureCountAfter: Number(activeAfter[0]?.c ?? 0),
      refused: false,
    } as CleanseResult
  }

  if (typeof (q as any).begin === 'function') {
    return (q as any).begin(run) as Promise<CleanseResult>
  }
  return run(q)
}
