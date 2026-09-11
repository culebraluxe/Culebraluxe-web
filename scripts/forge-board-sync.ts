// ---------------------------------------------------------------------------
// ENG-FORGE-SYNC-01 — ship-time board sync (adapter).
//
// Derives, from durable evidence, what the board should say about shipped work,
// and (with --apply) writes it to PROD. The decisions themselves live in
// workflow_app/forge/forge-board-sync.ts (pure, unit-tested); this file is the
// git + database + CLI edge around it.
//
//   pnpm forge:board-sync                    # dry run (default)
//   pnpm forge:board-sync --apply            # write
//   pnpm forge:board-sync --story <ID> --apply
//
// Idempotent (an already-Complete story is never re-noted). No work is ever
// re-executed and no number is ever estimated — absent evidence is recorded as
// absent.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { Pool } from '@neondatabase/serverless'

import { resolveDbTarget } from '../db/database-gateway'
import type { QueryExecutor, QueryRow } from '../db/query-executor'
import {
  assertForgeExecutionTarget,
  classifyShipCommits,
  deriveBoardSync,
} from '../workflow_app/forge/forge-board-sync'

const args = process.argv.slice(2)
const APPLY = args.includes('--apply')
/** Releasing a Hold/Deferred park is a human decision, so it needs its own flag. */
const RELEASE_HELD = args.includes('--release-held')
/**
 * Operator override for stories whose acceptance is NOT met despite a shipping
 * commit — e.g. a delivered slice of a larger story. Exclusion can only ever
 * AVOID a write; it can never fabricate evidence.
 */
const excludeIndex = args.indexOf('--exclude')
const EXCLUDED: ReadonlySet<string> = new Set(
  excludeIndex >= 0
    ? (args[excludeIndex + 1] ?? '')
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean)
    : [],
)
const storyIndex = args.indexOf('--story')
const STORY_FILTER = storyIndex >= 0 ? (args[storyIndex + 1] ?? null) : null
const BRANCH = process.env.SYNC_BRANCH ?? 'main'

type StoryRow = {
  id: string
  status: string
  completion: number | null
  notes: string | null
  run_count: number
  item_count: number
  run_envs: string[] | null
  item_envs: string[] | null
}

async function main() {
  // Part 3: fail closed on the environment, AND on the database the write would
  // actually reach. An intent flag is not enough — this tool must be incapable of
  // writing DEV, which is the precise confusion that produced the WS-01..12 mess.
  const target = assertForgeExecutionTarget(process.env.EXECUTION_ENV ?? 'PROD')
  const dbTarget = resolveDbTarget()
  if (dbTarget !== 'prod') {
    throw new Error(
      `refusing to run: the database target resolves to "${dbTarget}", not prod. ` +
        'A sync that reads PROD evidence and writes DEV board rows is worse than no sync. ' +
        'Run with APP_ENV=production (docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md section 0).',
    )
  }

  const url = process.env.DATABASE_URL_PROD
  if (!url) {
    throw new Error('DATABASE_URL_PROD is not configured; refusing to run (fail closed).')
  }
  const pool = new Pool({ connectionString: url })

  /**
   * Writes go through the SAME connection the evidence was read from, so the
   * reconcile can never diverge from what it verified. (Found the hard way: the
   * first version read PROD via this pool but wrote through the shared `sql`
   * executor, which routes by APP_ENV — so completions went to DEV while the
   * evidence came from PROD.)
   */
  const prodExecutor: QueryExecutor = async (strings: TemplateStringsArray, ...values: unknown[]) => {
    let text = ''
    strings.forEach((chunk, index) => {
      text += chunk
      if (index < values.length) text += `$${index + 1}`
    })
    const result = await pool.query(text, values)
    return result.rows as QueryRow[]
  }

  const gitLog = execFileSync('git', ['log', '--oneline', '-n', '5000', BRANCH], {
    encoding: 'utf8',
  })
  console.log(`target ${target} | ship evidence: git log --oneline ${BRANCH}`)
  console.log(APPLY ? 'mode: APPLY (writing)' : 'mode: dry run (pass --apply to write)')
  if (RELEASE_HELD) {
    console.log('release-held: ON — Hold/Deferred stories whose work shipped will be completed')
  }

  // Part 1: run provenance is part of the read, not an afterthought.
  const cols = (await pool.query(
    "select table_name, column_name from information_schema.columns where column_name = 'execution_environment' and table_name in ('storyboard_story_run','agent_work_item')",
  )).rows as Array<{ table_name: string }>
  const hasEnv = (t: string) => cols.some((c) => c.table_name === t)
  const runEnv = hasEnv('storyboard_story_run') ? 'r.execution_environment' : 'null'
  const itemEnv = hasEnv('agent_work_item') ? 'w.execution_environment' : 'null'

  const stories = (await pool.query(
    `
    select s.id, s.status, s.completion, s.notes,
      (select count(*)::int from storyboard_story_run r where r.story_id = s.id) as run_count,
      (select count(*)::int from agent_work_item w where w.story_id = s.id) as item_count,
      (select coalesce(array_agg(distinct coalesce(${runEnv}, '')), '{}')
         from storyboard_story_run r where r.story_id = s.id) as run_envs,
      (select coalesce(array_agg(distinct coalesce(${itemEnv}, '')), '{}')
         from agent_work_item w where w.story_id = s.id) as item_envs
    from storyboard_story s
    where s.status <> 'Complete' ${STORY_FILTER ? 'and s.id = $1' : ''}
    order by s.id
    `,
    STORY_FILTER ? [STORY_FILTER] : [],
  )).rows as StoryRow[]

  let completed = 0
  let unchanged = 0
  const warnings: string[] = []
  const heldShipped: string[] = []

  for (const story of stories) {
    if (EXCLUDED.has(story.id)) {
      console.log(`  ${story.id.padEnd(32)} ${story.status.padEnd(12)} excluded (left untouched)`)
      continue
    }
    const { shipping, docsOnly } = classifyShipCommits(story.id, gitLog)
    const decision = deriveBoardSync({
      story: { id: story.id, status: story.status, completion: story.completion },
      ship: { storyId: story.id, commits: shipping, docsOnly },
      runs: {
        runCount: story.run_count,
        itemCount: story.item_count,
        environments: [...(story.run_envs ?? []), ...(story.item_envs ?? [])],
      },
      now: new Date().toISOString(),
      releaseHeld: RELEASE_HELD,
    })

    if (decision.environmentWarning) warnings.push(`${story.id}: ${decision.environmentWarning}`)

    if (decision.action === 'no-change') {
      unchanged += 1
      if (decision.reason === 'held-shipped') heldShipped.push(`${story.id} (${story.status})`)
      console.log(
        `  ${story.id.padEnd(32)} ${story.status.padEnd(12)} no-change (${decision.reason})`,
      )
      continue
    }

    completed += 1
    console.log(
      `  ${story.id.padEnd(32)} ${story.status.padEnd(12)} -> Complete 100% ` +
        `(${decision.ship.commits.length} commit(s), runs=${story.run_count}, items=${story.item_count})`,
    )

    if (APPLY) {
      const { markForgeStoryShippedComplete } = await import('../db/forge-story-state')
      await markForgeStoryShippedComplete(story.id, decision.note ?? '', prodExecutor)
    }
  }

  console.log(
    `\n${stories.length} open story(s): ${completed} reconcile to Complete, ${unchanged} unchanged${APPLY ? ' (applied)' : ' (dry run, nothing written)'}`,
  )
  if (heldShipped.length) {
    console.log(
      '\nSHIPPED BUT HELD (a human decision — release the hold on purpose if the work is done):',
    )
    for (const entry of heldShipped) console.log(`  ${entry}`)
  }
  if (warnings.length) {
    console.log('\nNON-PROD RUN EVIDENCE (never silent):')
    for (const w of warnings) console.log(`  ${w}`)
  }
  await pool.end()
}

void main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error)
    process.exit(1)
  })
