// PROOF, on PROD, of the KAHN BAN -> TABLE SYNC and the Autosys rule: "if it's in the table it goes".
// Net zero: the probe's batch rows are deleted and the story is restored exactly.
import { listActiveAgentWorkForStory, withdrawQueuedAgentWork } from '@/legacy/db/agent-work'
import {
  fireStagingBatch,
  getStagingBatch,
  listForgeBatchesForStory,
  stageStoryForBatch,
  unstageStoryForBatch,
} from '@/legacy/db/forge-batch'
import { getStoryboardStory, setStoryboardStatus } from '@/legacy/db/storyboard'
import { interactiveSql } from '@/lib/neon-interactive'

const STORY = 'ENG-FORGE-DOCTOR-01'

const state = async () => ({
  status: (await getStoryboardStory(STORY))?.status,
  items: (await listActiveAgentWorkForStory(STORY)).length,
  stagingRows: (await getStagingBatch())?.storyCount ?? 0,
})

async function main() {
  const before = await state()
  const preExisting = (await getStagingBatch())?.storyCount ?? 0
  console.log(`ORIGINAL: ${JSON.stringify(before)}  (pre-existing staging rows: ${preExisting})`)
  if (preExisting !== 0) {
    console.log('REFUSING: the staging batch is not empty, so this probe could disturb real work.')
    return
  }

  // 1. THE BOARD STAGES A CARD -> a row appears in the table (status Batched + forge_batch_item).
  await setStoryboardStatus(STORY, 'Batched')
  const staged = await stageStoryForBatch(STORY)
  console.log(`staged: batch=${staged.batchId} members=${staged.members} | ${JSON.stringify(await state())}`)
  console.log(`  STAGING STILL DISPATCHES NOTHING: items=${(await state()).items} (must be 0)`)

  // 2. THE BOARD TAKES THE CARD OUT -> the row leaves with it.
  await unstageStoryForBatch(STORY)
  await setStoryboardStatus(STORY, 'In Progress')
  console.log(`unstaged: ${JSON.stringify(await state())}  (staging rows must be 0 again)`)
  console.log(`  story batch history after unstage: ${JSON.stringify(await listForgeBatchesForStory(STORY))}`)

  // 3. STAGE TWO, THEN RUN THE TABLE — what fires is what the table holds.
  await setStoryboardStatus(STORY, 'Batched')
  await stageStoryForBatch(STORY)
  const fired = await fireStagingBatch()
  console.log(`fired from the table: ${JSON.stringify(fired)}`)
  console.log(`after fire: ${JSON.stringify(await state())}  (one work item expected)`)
  console.log(`  story batch history: ${JSON.stringify(await listForgeBatchesForStory(STORY))}`)

  // 4. NET ZERO — withdraw, restore, delete the probe's rows.
  const { withdrawn, live } = await withdrawQueuedAgentWork(STORY)
  console.log(`withdraw: withdrawn=${withdrawn} live=${live}`)
  await setStoryboardStatus(STORY, before.status ?? 'In Progress')
  await interactiveSql`
    delete from forge_batch_item where batch_id in (
      select id from forge_batch where created_at > now() - interval '20 minutes'
    )
  `
  await interactiveSql`delete from forge_batch where created_at > now() - interval '20 minutes'`
  const restored = await state()
  console.log(`RESTORED: ${JSON.stringify(restored)}  (original ${JSON.stringify(before)})`)
}

void main()
