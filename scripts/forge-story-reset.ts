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
//   clean              -> PRE-TEST HYGIENE, control-plane wide, no story needed: cancel
//                        stale open work items, interrupt stale engine claims, abort stale
//                        instances, obsolete their open tasks. Only claims older than
//                        --stale-minutes (default 15) are touched, so a live peer survives.
//
// Usage:
//   node --import tsx --env-file=.env.local scripts/forge-story-reset.ts \
//        <story-id> [reset|recover] [--force]
//   node --import tsx --env-file=.env.local scripts/forge-story-reset.ts \
//        clean [--stale-minutes N] [--force]
//
// Safe: only touches the named story's engine rows. The DATABASE TARGET IS NOT A
// CHOICE: the pool manager (ForgeDB + the ONE environment declaration) decides it,
// and this tool refuses to run anywhere that is not PROD. --force is still required,
// because a destructive act should be deliberate. Always run reset when you want a
// clean re-run; use recover to continue a partial run whose worker died.
// ---------------------------------------------------------------------------
import { forgeDb } from '../db/forge-db'
import { recoverStaleForgeEngineClaims } from '../db/forge-engine-recovery'
import { resolveStoryResetConfig } from './forge-story-reset-config'

const config = resolveStoryResetConfig(process.argv, process.env)
if (!config.ok) {
  console.error(config.error)
  process.exit(2)
}
const { story, mode, target, staleMinutes } = config

async function main(): Promise<void> {
  const pool = forgeDb.forTarget(target)
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
      // 4. CLOSE THE ENGINE TASK EXECUTIONS THIS RESET JUST KILLED.
      //
      // Without this step a reset left every in-flight `forge_engine_task_execution` row in
      // 'claimed' forever. Those rows are the junk that made test results untrustworthy:
      // 15 of them accumulated in one afternoon (9 lead_pre, 4 architect, 2 fast_smith),
      // hours stale, and the next run had to be read against a control plane that was
      // still holding the previous run's claims. A story reset that leaves claims behind
      // is not a reset.
      const execs = await pool.query(
        `update forge_engine_task_execution
            set status='interrupted', last_error='story reset', updated_at=now()
          where status in ('claimed','running')
            and process_instance_id in (
              select id from process_instances where subject_id=$1
            )
          returning task_id`,
        [story],
      )
      console.log(`[reset] interrupted engine task executions: ${execs.rows.length}`)
      // 5. Return the story to Planned.
      const s = await pool.query(
        `update storyboard_story set status='Planned', completion=0, updated_at=now()
         where id=$1 returning id, status`,
        [story],
      )
      console.log(`[reset] story status: ${s.rows[0]?.status ?? 'NOT FOUND'}`)
    } else if (mode === 'clean') {
      // CLEAN = pre-test hygiene for the CONTROL PLANE, not for one story's chain.
      //
      // Every run reads the same tables, so a leftover claim from an earlier death is not
      // merely untidy: it can hold the single-active lock, mis-attribute a candidate, or
      // let a stale decision be read as this run's. Clearing it is part of the setup, so
      // it has to be one command and it has to be safe to run while peers are working.
      //
      // SAFETY: only claims OLDER THAN --stale-minutes are touched (default 15). A live
      // run by another worker is younger than that and survives untouched, which is why
      // this can be run on a shared control plane at all.
      const cutoffMinutes = staleMinutes
      const staleCutoff = new Date(Date.now() - cutoffMinutes * 60_000)

      // a. Open work items that have not moved since the cutoff.
      const items = await pool.query(
        `update agent_work_item
            set state='Cancelled', claimed_by=null, started_at=null,
                finished_at=now(), updated_at=now()
          where state in ('Ready','Claimed','Running','Paused')
            and coalesce(updated_at, created_at) <= $1
          returning id`,
        [staleCutoff],
      )
      console.log(`[clean] cancelled stale open work items: ${items.rows.length}`)

      // b. Stale engine claims, through the SAME recovery path the engine uses: it locks
      //    the owning instance, CASes the row to interrupted, and releases its work item.
      //    Reusing it means the sweep cannot drift from what recovery means.
      const recovered = await recoverStaleForgeEngineClaims({
        staleMs: cutoffMinutes * 60_000,
        limit: 500,
      })
      console.log(
        `[clean] interrupted stale engine claims: ${recovered.filter((r) => r.recovered).length}` +
          ` (skipped ${recovered.filter((r) => !r.recovered).length})`,
      )

      // c. Instances that have not moved since the cutoff.
      const inst = await pool.query(
        `update process_instances set status='aborted', updated_at=now()
          where status in ('active','running','reserved','suspended')
            and coalesce(updated_at, created_at) <= $1
          returning id`,
        [staleCutoff],
      )
      console.log(`[clean] aborted stale engine instances: ${inst.rows.length}`)

      // d. Open workflow rows under instances that are now terminal.
      const tasks = await pool.query(
        `update tasks set status='obsolete', updated_at=now()
          where status in ('created','ready','reserved','in_progress')
            and process_instance_id in (
              select id from process_instances
              where status not in ('active','running','reserved','suspended')
            )
          returning id`,
      )
      console.log(`[clean] obsoleted open tasks under terminal instances: ${tasks.rows.length}`)

      // e. Any claim still open on an instance that is no longer running. Recovery above
      //    is heartbeat-bounded and paginated (limit 500); this is the mop-up for the
      //    rest, and it is what makes "clean" a guarantee rather than a best effort.
      const execs = await pool.query(
        `update forge_engine_task_execution
            set status='interrupted', last_error='clean sweep', updated_at=now()
          where status in ('claimed','running')
            and process_instance_id in (
              select id from process_instances
              where status not in ('active','running','reserved','suspended')
            )
          returning task_id`,
      )
      console.log(`[clean] interrupted orphaned engine claims: ${execs.rows.length}`)
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

    // The post-condition, printed every time. A sweep you cannot read the result of is a
    // sweep you will run twice and trust neither time.
    if (mode === 'clean') {
      const instLeft = await pool.query(
        `select count(*)::int c from process_instances
          where status in ('active','running','reserved','suspended')`,
      )
      const taskLeft = await pool.query(
        `select count(*)::int c from tasks
          where status in ('created','ready','reserved','in_progress')`,
      )
      const itemLeft = await pool.query(
        `select count(*)::int c from agent_work_item
          where state in ('Ready','Claimed','Running','Paused')`,
      )
      const execLeft = await pool.query(
        `select count(*)::int c from forge_engine_task_execution
          where status in ('claimed','running')`,
      )
      console.log(
        `remaining (control plane): instances=${instLeft.rows[0].c} openTasks=${taskLeft.rows[0].c} ` +
          `openWorkItems=${itemLeft.rows[0].c} activeEngineClaims=${execLeft.rows[0].c}`,
      )
      return
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
       where story_id=$1 and state in ('Ready','Claimed','Running','Paused')`,
      [story],
    )
    const execLeft = await pool.query(
      `select count(*)::int c from forge_engine_task_execution
       where status in ('claimed','running')
         and process_instance_id in (select id from process_instances where subject_id=$1)`,
      [story],
    )
    console.log(
      `remaining: instances=${instLeft.rows[0].c} openTasks=${taskLeft.rows[0].c} ` +
        `openWorkItems=${itemLeft.rows[0].c} activeEngineClaims=${execLeft.rows[0].c}`,
    )
  } finally {
    await pool.end()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
