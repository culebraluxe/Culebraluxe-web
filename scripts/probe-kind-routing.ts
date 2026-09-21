// ---------------------------------------------------------------------------
// PROBE — the kind router, on a real Postgres, NET ZERO.
//
//   APP_ENV=dev node --env-file=.env.local --import tsx scripts/probe-kind-routing.ts
//
// What it proves that the unit test cannot: the columns exist after migration 179, the batch row's
// `model_policy` really defaults to `cheap`, and the copy at dispatch really lands on the work item
// the STORY TRIGGER created — the trigger is the part a fake executor stands in for.
//
// It runs against DEV by default and REFUSES PROD unless you pass `--prod`, because firing a batch
// writes `Ready` status on real stories and PROD has a worker that would claim them.
//
// Net zero: every story is restored to the status it had, the queued items are withdrawn, and the
// probe's own batch rows are deleted. The closing step compares the end state to the start state and
// says so out loud, because "restored" is a claim that should be checkable.
// ---------------------------------------------------------------------------

import { withdrawQueuedAgentWork, listActiveAgentWorkForStory } from '@/legacy/db/agent-work'
import {
  fireStagingBatch,
  getStagingBatch,
  listStagingBatchItems,
  setStagedItemKind,
  setStagingBatchPolicy,
  stageStoryForBatch,
  unstageStoryForBatch,
} from '@/legacy/db/forge-batch'
import { listStoryboardStories, getStoryboardStory, setStoryboardStatus } from '@/legacy/db/storyboard'
import { interactiveSql } from '@/lib/neon-interactive'
import { describeControlPlane } from '@/lib/execution-target'

const ALLOW_PROD = process.argv.includes('--prod')

/** The two stories the probe will move. Chosen deterministically, restored at the end. */
async function pickStories(): Promise<Array<{ id: string; status: string }>> {
  const stories = (await listStoryboardStories()) ?? []
  const safe = stories
    .filter(
      (story) =>
        /^ENG-FORGE-/.test(String(story.id)) &&
        ['Complete', 'Planned', 'Deferred'].includes(String(story.status)),
    )
    .sort((a, b) => String(a.id).localeCompare(String(b.id)))
    .slice(0, 2)
    .map((story) => ({ id: String(story.id), status: String(story.status) }))
  if (safe.length < 2) throw new Error('need two ENG-FORGE stories in a safe status to probe with')
  return safe
}

async function columnsFor(table: string): Promise<string[]> {
  const rows = await interactiveSql`
    select column_name from information_schema.columns
    where table_schema = 'public' and table_name = ${table}
  `
  return rows.map((row: Record<string, unknown>) => String(row.column_name))
}

function check(label: string, ok: boolean, detail: string): boolean {
  console.log(`  ${ok ? 'ok  ' : 'FAIL'} ${label} — ${detail}`)
  return ok
}

async function main(): Promise<void> {
  const plane = describeControlPlane()
  console.log(`probe-kind-routing — control plane: APP_ENV=${plane.appEnv} → ${plane.target} (${plane.declaredBy})`)
  // `target` is lowercase ('prod' | 'dev' | null); null means the environment did not declare one,
  // and an undeclared target is not PROD, so the probe's default (DEV) guard below is what applies.
  if (plane.target === 'prod' && !ALLOW_PROD) {
    console.log('REFUSING: target is PROD. Firing a batch queues real work for the live worker.')
    console.log('          Re-run with --prod if you mean it, or use: APP_ENV=dev …')
    return
  }

  // 1. THE SCHEMA (migration 179). The probe starts by refusing to read a missing column as a default.
  const [batchCols, itemCols, workCols] = await Promise.all([
    columnsFor('forge_batch'),
    columnsFor('forge_batch_item'),
    columnsFor('agent_work_item'),
  ])
  const hasRouting =
    batchCols.includes('model_policy') && itemCols.includes('kind') && workCols.includes('kind') && workCols.includes('model_policy')
  let ok = check(
    'migration 179 columns',
    hasRouting,
    hasRouting
      ? 'forge_batch.model_policy · forge_batch_item.kind · agent_work_item.kind/model_policy all present'
      : `missing: ${[
          !batchCols.includes('model_policy') ? 'forge_batch.model_policy' : null,
          !itemCols.includes('kind') ? 'forge_batch_item.kind' : null,
          !workCols.includes('kind') ? 'agent_work_item.kind' : null,
          !workCols.includes('model_policy') ? 'agent_work_item.model_policy' : null,
        ]
          .filter(Boolean)
          .join(', ')}`,
  )
  if (!hasRouting) {
    console.log('\nSTOP: migration 179 is not applied to this target. Apply it, then re-run.')
    process.exitCode = 1
    return
  }

  // 2. PREFLIGHT — never disturb a batch somebody else is building.
  const preExisting = (await getStagingBatch())?.storyCount ?? 0
  if (preExisting !== 0) {
    console.log(`REFUSING: the staging batch already holds ${preExisting} story(ies); this probe would disturb real work.`)
    process.exitCode = 1
    return
  }

  const before = await pickStories()
  console.log(`\nprobe stories: ${before.map((t) => `${t.id} (${t.status})`).join(', ')}`)
  const expectedKinds = ['fix', 'judgment'] as const

  // 3. STAGE TWO KINDS. Read the column default BEFORE anything sets a policy, so the `cheap` default
  //    is proven by the database rather than by the code that would apply it.
  for (const target of before) await setStoryboardStatus(target.id, 'Batched')
  const staged = await stageStoryForBatch(before[0].id)
  await stageStoryForBatch(before[1].id)
  await setStagedItemKind(before[0].id, expectedKinds[0])
  await setStagedItemKind(before[1].id, expectedKinds[1])

  const stagedBatch = await getStagingBatch()
  ok =
    check(
      'a new batch row defaults to model_policy=cheap',
      stagedBatch?.modelPolicy === 'cheap',
      `batch ${staged.batchId} says ${stagedBatch?.modelPolicy}`,
    ) && ok
  const roster = await listStagingBatchItems()
  ok =
    check(
      'the roster reads the kind per member',
      roster.length === 2 && roster.map((row) => row.kind).join(',') === expectedKinds.join(','),
      JSON.stringify(roster.map((row) => `${row.storyId}:${row.kind}`)),
    ) && ok

  // 4. THE FIRE — the copy at dispatch, through the real story trigger.
  await setStagingBatchPolicy('judgment')
  const fired = await fireStagingBatch()
  console.log(`  fired: ${JSON.stringify(fired)}`)
  ok = check('both members queued', fired?.queued === 2, `queued ${fired?.queued}`) && ok
  ok = check('both work items stamped at dispatch', fired?.stamped === 2, `stamped ${fired?.stamped}`) && ok

  for (let i = 0; i < before.length; i += 1) {
    const item = (await listActiveAgentWorkForStory(before[i].id))[0]
    ok =
      check(
        `${before[i].id} carries its own kind and the batch policy`,
        Boolean(item) && item.kind === expectedKinds[i] && item.modelPolicy === 'judgment',
        item ? `kind=${item.kind} policy=${item.modelPolicy} state=${item.state}` : 'no work item found',
      ) && ok
  }

  // 5. NET ZERO.
  for (const target of before) {
    const { withdrawn, live } = await withdrawQueuedAgentWork(target.id)
    if (live > 0) console.log(`  note: ${target.id} has a live (claimed/running) item; left alone`)
    console.log(`  withdraw ${target.id}: withdrawn=${withdrawn} live=${live}`)
    await unstageStoryForBatch(target.id)
    await setStoryboardStatus(target.id, target.status)
  }
  await interactiveSql`
    delete from forge_batch_item where batch_id in (
      select id from forge_batch where created_at > now() - interval '20 minutes'
    )
  `
  await interactiveSql`delete from forge_batch where created_at > now() - interval '20 minutes'`

  const after = await Promise.all(
    before.map(async (target) => ({
      id: target.id,
      status: String((await getStoryboardStory(target.id))?.status),
      items: (await listActiveAgentWorkForStory(target.id)).length,
    })),
  )
  const restored = after.every((row, i) => row.status === before[i].status && row.items === 0)
  check('net zero', restored, `after: ${after.map((row) => `${row.id}(${row.status}, ${row.items} items)`).join(', ')}`)
  const pass = ok && restored
  console.log(`\nprobe-kind-routing — ${pass ? 'PASS' : 'FAIL'}`)
  if (!pass) process.exitCode = 1
}

void main()
