// ---------------------------------------------------------------------------
// Forge story reset / recovery — the operations we kept doing by hand after a
// run was killed mid-chain. Two modes:
//
//   reset   (default) -> ABORT any active engine instance for the story,
//                        obsolete its open tasks, cancel open agent_work_items,
//                        and set the story back to Planned for a clean fresh run.
//   recover            -> RELEASE stale claims so an existing instance can RESUME:
//                        open (reserved/in_progress) engine tasks back to ready,
//                        cancel leftover Running/Claimed agent_work_items.
//
// Usage:
//   node --env-file=.env.local --import tsx scripts/forge-story-reset.ts \
//        <story-id> [reset|recover] [dev|prod] [--force]
//
// Safe: only touches the named story's engine rows. PROD is explicit (default
// target follows APP_ENV) and is REFUSED unless --force is passed. Always run
// reset when you want a clean re-run; use recover to continue a partial run
// whose worker died.
// ---------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'
import { resolveStoryResetConfig } from './forge-story-reset-config'

const config = resolveStoryResetConfig(process.argv, process.env)
if (!config.ok) {
  console.error(config.error)
  process.exit(2)
}
const { story, mode, target, url } = config

async function main(): Promise<void> {
  const pool = new Pool({ connectionString: url })
  try {
    if (mode === 'reset') {
      // 1. Abort any non-terminal engine instance for the story.
      const inst = await pool.query(
        `update process_instances set status='aborted', updated_at=now()
         where subject_id=$1 and status in ('active','running','reserved','suspended')
         returning id`,
        [story],
      )
      console.log(`[reset] aborted engine instances: ${inst.rows.length}`)
      // 2. Obsolete open tasks under those instances.
      const tasks = await pool.query(
        `update tasks set status='obsolete', updated_at=now()
         where status in ('created','ready','reserved','in_progress')
           and process_instance_id in (
             select id from process_instances where subject_id=$1
           )
         returning id`,
        [story],
      )
      console.log(`[reset] obsoleted open tasks: ${tasks.rows.length}`)
      // 3. Cancel open agent_work_items for the story.
      const items = await pool.query(
        `update agent_work_item set state='Cancelled', claimed_by=null, started_at=null,
                finished_at=now(), updated_at=now()
         where story_id=$1 and state in ('Ready','Claimed','Running')
         returning id`,
        [story],
      )
      console.log(`[reset] cancelled open work items: ${items.rows.length}`)
      // 4. Return the story to Planned.
      const s = await pool.query(
        `update storyboard_story set status='Planned', completion=0, updated_at=now()
         where id=$1 returning id, status`,
        [story],
      )
      console.log(`[reset] story status: ${s.rows[0]?.status ?? 'NOT FOUND'}`)
    } else {
      // recover: release stale claims on the existing instance so it resumes.
      const tasks = await pool.query(
        `update tasks set status='ready', assignee=null, claimed_at=null, updated_at=now()
         where status in ('reserved','in_progress')
           and process_instance_id in (
             select id from process_instances where subject_id=$1 and status in ('active','running')
           )
         returning id`,
        [story],
      )
      console.log(`[recover] released tasks to ready: ${tasks.rows.length}`)
      const items = await pool.query(
        `update agent_work_item set state='Cancelled', claimed_by=null, started_at=null,
                finished_at=now(), updated_at=now()
         where story_id=$1 and state in ('Claimed','Running')
         returning id`,
        [story],
      )
      console.log(`[recover] cancelled stale running work items: ${items.rows.length}`)
    }

    const instLeft = await pool.query(
      `select count(*)::int c from process_instances
       where subject_id=$1 and status in ('active','running','reserved','suspended')`,
      [story],
    )
    const taskLeft = await pool.query(
      `select count(*)::int c from tasks
       where status in ('created','ready','reserved','in_progress')
         and process_instance_id in (select id from process_instances where subject_id=$1)`,
      [story],
    )
    const itemLeft = await pool.query(
      `select count(*)::int c from agent_work_item
       where story_id=$1 and state in ('Ready','Claimed','Running')`,
      [story],
    )
    console.log(
      `remaining: instances=${instLeft.rows[0].c} openTasks=${taskLeft.rows[0].c} openWorkItems=${itemLeft.rows[0].c}`,
    )
  } finally {
    await pool.end()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
