// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// Probe: prove the SORTER's bench transitions through the same db calls the action makes,
// then restore the story exactly as found. PROD.
import { canMove, normalizeStoryBucket, type StoryBucket } from '@/lib/story-moves'
import {
  getStoryboardStory,
  listActiveWork,
  listStoryIdsWithStatus,
  setActiveWork,
  setStoryboardStatus,
} from '@/legacy/db/storyboard'

async function main() {
  const PAIRS: Array<[StoryBucket, StoryBucket]> = [
    ['backlog', 'engine'],
    ['backlog', 'batch'],
    ['bench', 'open'],
    ['bench', 'batch'],
    ['bench', 'engine'],
    ['next', 'batch'],
    ['next', 'engine'],
    ['open', 'engine'],
  ]
  for (const [from, to] of PAIRS) {
    console.log(`${canMove(from, to) ? 'ALLOW ' : 'REFUSE'} ${from} -> ${to}`)
  }
  // The drift that refused every drag out of NEXT VERSION: the view's id normalized to the rule's key.
  for (const raw of ['next-version', 'NEXT VERSION', 'work bench', 'engine-queue', 'nonsense']) {
    console.log(`normalize(${JSON.stringify(raw)}) = ${normalizeStoryBucket(raw)}`)
  }

  const onBench = async (id: string) => (await listActiveWork()).some((s) => s.id === id)
  const id = 'CRM-28'
  const original = await getStoryboardStory(id)
  const wasOnBench = await onBench(id)
  // The actor is a uuid (`storyboard_active_work.selected_by_app_user_id`); pass null to record no
  // actor rather than a string the column will reject (22P02).
  console.log(`\nbefore: status=${original?.status} bench=${wasOnBench}`)

  // bench -> open  (the action: clear the intent row, keep In Progress)
  await setActiveWork(id, true, null)
  console.log(
    `on bench: bench=${await onBench(id)} status=${(await getStoryboardStory(id))?.status}`,
  )
  await setActiveWork(id, false, null)
  await setStoryboardStatus(id, 'In Progress')
  console.log(`bench -> open: status=${(await getStoryboardStory(id))?.status} bench=${await onBench(id)}`)

  // bench -> batch  (the action: clear the intent row, stage as Batched)
  await setActiveWork(id, true, null)
  await setActiveWork(id, false, null)
  await setStoryboardStatus(id, 'Batched')
  const staged = await listStoryIdsWithStatus('Batched')
  console.log(
    `bench -> batch: status=${(await getStoryboardStory(id))?.status} bench=${await onBench(id)} staged=${staged.length}`,
  )

  // restore exactly: the status AND the bench intent the story actually had.
  await setStoryboardStatus(id, original?.status ?? 'Planned')
  if (wasOnBench) await setActiveWork(id, true, null)
  console.log(
    `RESTORED: status=${(await getStoryboardStory(id))?.status} bench=${await onBench(id)}`,
  )
}

void main()

