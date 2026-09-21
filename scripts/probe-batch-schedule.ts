// PROOF, on PROD, that a SCHEDULED batch fires itself and that STAGING fires nothing.
// Net zero: the probe's own batch rows are deleted and the story is restored exactly.
import { listActiveAgentWorkForStory, withdrawQueuedAgentWork } from '@/legacy/db/agent-work'
import {
  cancelForgeBatch,
  createForgeBatch,
  fireDueForgeBatches,
  listForgeBatches,
  listForgeBatchesForStory,
} from '@/legacy/db/forge-batch'
import { getStoryboardStory, setStoryboardStatus } from '@/legacy/db/storyboard'
import { interactiveSql } from '@/lib/neon-interactive'

const STORY = 'ENG-FORGE-DOCTOR-01'
const LABEL = 'PROBE-net-zero'

const state = async () => ({
  status: (await getStoryboardStory(STORY))?.status,
  items: (await listActiveAgentWorkForStory(STORY)).length,
})

async function main() {
  const before = await state()
  const existing = await listForgeBatches(20)
  console.log(`ORIGINAL: ${JSON.stringify(before)} | batches in the table: ${existing.length}`)

  // 1. SCHEDULE IN THE PAST so it is due immediately — the same row a 02:00 night run would write.
  const past = new Date(Date.now() - 60_000).toISOString()
  const batch = await createForgeBatch({
    storyIds: [STORY],
    label: LABEL,
    scheduledFor: past,
    note: 'PROBE',
  })
  console.log(
    `scheduled: id=${batch.id} status=${batch.status} stories=${batch.storyCount} for=${batch.scheduledFor}`,
  )
  const afterSchedule = await state()
  console.log('  STAGING FIRES NOTHING:', JSON.stringify(afterSchedule), '(items must still be', before.items, ')')

  // 2. THE UNATTENDED PASS — exactly what agent:work calls on every 3-minute tick.
  const fired = await fireDueForgeBatches()
  for (const f of fired) console.log(`  fired batch=${f.batchId} queued=${f.queued} failed=${f.failed.length}`)
  const afterFire = await state()
  console.log('after fire:', JSON.stringify(afterFire), '(one work item expected)')
  console.log('  story batch history:', JSON.stringify(await listForgeBatchesForStory(STORY)))

  // 3. NET ZERO — withdraw the dispatch, restore the story, delete the probe's batch rows.
  const { withdrawn, live } = await withdrawQueuedAgentWork(STORY)
  console.log(`withdraw: withdrawn=${withdrawn} live=${live}`)
  await setStoryboardStatus(STORY, before.status ?? 'In Progress')
  await interactiveSql`delete from forge_batch_item where batch_id = ${batch.id}`
  await interactiveSql`delete from forge_batch where id = ${batch.id}`
  const restored = await state()
  const remaining = await listForgeBatches(20)
  console.log(`RESTORED: ${JSON.stringify(restored)} | batches in the table: ${remaining.length}`)
  console.log(`PROBE ROWS LEFT: ${remaining.filter((b) => b.label === LABEL).length}`)
}

void main()
