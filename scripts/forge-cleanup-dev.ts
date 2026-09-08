// -----------------------------------------------------------------------------
// Forge DEV cleanup — prune orphaned engine rows + stale agent worktrees so
// accumulated garbage doesn't interfere with real runs (the "clean the stale
// stuff" discipline). DEV-only, DRY-RUN by default:
//   APP_ENV=development node --env-file=.env.local scripts/forge-cleanup-dev.ts            # dry run
//   APP_ENV=development node --env-file=.env.local scripts/forge-cleanup-dev.ts --apply    # execute
// Refuses to run when APP_ENV would target production.
// -----------------------------------------------------------------------------
import { execFileSync } from 'node:child_process'
import { existsSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { resolveDbTarget } from '../db/client'

const APPLY = process.argv.includes('--apply')
const DAYS = Number(process.env.FORGE_CLEANUP_DAYS ?? '7')
const STALE_MS = Number.isFinite(DAYS) && DAYS > 0 ? DAYS * 86400_000 : 7 * 86400_000

function log(action: string, msg: string): void {
  console.log(`${APPLY ? '[apply]' : '[dry]  '} ${action}: ${msg}`)
}

async function run() {
  const target = resolveDbTarget()
  if (target === 'prod') {
    console.error('refusing: forge-cleanup must run against DEV (APP_ENV != production).')
    process.exit(1)
  }
  console.log(`target=${target} mode=${APPLY ? 'apply' : 'dry-run'} stale_worktree_days=${DAYS}`)

  const { sql } = await import('../db/client')

  // 1. Orphaned engine rows (parents already gone) — the true accumulation.
  const orphanInstances = await sql`
    select p.id from process_instances p
    left join storyboard_story s on s.id = p.subject_id
    where s.id is null
  `
  log('process_instances orphaned', `${orphanInstances.length}`)
  for (const row of orphanInstances) {
    const id = String(row.id)
    if (APPLY) {
      await sql`delete from forge_workflow_evidence where process_instance_id = ${id}`
      await sql`delete from process_events where process_instance_id = ${id}`
      await sql`delete from process_instances where id = ${id}`
    }
  }

  const orphanRuns = await sql`
    select count(*)::int as n from storyboard_story_run r
    left join storyboard_story s on s.id = r.story_id
    where s.id is null
  `
  log('story_run orphaned', String(orphanRuns[0]?.n ?? 0))
  const orphanWork = await sql`
    select count(*)::int as n from agent_work_item w
    left join storyboard_story s on s.id = w.story_id
    where s.id is null
  `
  log('agent_work_item orphaned', String(orphanWork[0]?.n ?? 0))
  const orphanArt = await sql`
    select count(*)::int as n from forge_tool_artifact a
    left join storyboard_story s on s.id = a.story_id
    where s.id is null
  `
  log('tool_artifact orphaned', String(orphanArt[0]?.n ?? 0))

  if (APPLY) {
    await sql`delete from forge_tool_artifact where story_id not in (select id from storyboard_story)`
    await sql`delete from agent_work_item where story_id not in (select id from storyboard_story)`
    await sql`delete from storyboard_story_run where story_id not in (select id from storyboard_story)`
  }

  // 2. Stale agent worktrees under the shared worktrees root.
  const root = join(process.cwd(), '..', 'Culebraluxe-worktrees')
  if (existsSync(root)) {
    const listed = execFileSync('git', ['worktree', 'list'], { cwd: process.cwd(), encoding: 'utf8' })
      .split('\n')
      .map((l) => l.split(/\s+/)[0])
      .filter(Boolean)
    let pruned = 0
    for (const wt of listed) {
      if (!wt.includes('/Culebraluxe-worktrees/')) continue
      let age = Infinity
      try {
        age = Date.now() - statSync(wt).mtimeMs
      } catch {
        /* missing dir */
      }
      if (age > STALE_MS) {
        pruned += 1
        if (APPLY) {
          try {
            execFileSync('git', ['worktree', 'remove', '--force', wt], { cwd: process.cwd(), stdio: 'ignore' })
          } catch {
            log('worktree unregister', wt)
            execFileSync('git', ['worktree', 'prune'], { cwd: process.cwd(), stdio: 'ignore' })
          }
        } else {
          log('worktree stale', wt)
        }
      }
    }
    if (APPLY) execFileSync('git', ['worktree', 'prune'], { cwd: process.cwd(), stdio: 'ignore' })
    log('stale worktrees', `${pruned} (>${DAYS}d)`)
  } else {
    log('worktrees root missing', root)
  }

  console.log(APPLY ? 'cleanup applied.' : 'dry-run complete — pass --apply to execute.')
}

run().then(
  () => process.exit(0),
  (e) => {
    console.error(String((e as Error)?.message ?? e).slice(0, 600))
    process.exit(1)
  },
)
