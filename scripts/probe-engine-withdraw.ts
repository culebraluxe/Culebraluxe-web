// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// PROOF, on PROD, that "pull the note back off ENGINE RUN Q" withdraws the queue entry it created.
// Net zero: a work item is created, then withdrawn, and the story's status is restored exactly.
import { withdrawQueuedAgentWork } from '@/legacy/db/agent-work'
import { getStoryboardStory, setStoryboardStatus } from '@/legacy/db/storyboard'

const STORY = 'ENG-FORGE-DOCTOR-01'

async function activeWorkItems(storyId: string) {
  const { listActiveAgentWorkForStory } = await import('@/legacy/db/agent-work')
  return listActiveAgentWorkForStory(storyId)
}

async function main() {
  const before = await getStoryboardStory(STORY)
  console.log(`before: status=${before?.status} activeWorkItems=${(await activeWorkItems(STORY)).length}`)

  // 1. HAND IT OVER — exactly what the board's "move to Run Q" writes.
  await setStoryboardStatus(STORY, 'Ready')
  const afterDispatch = await activeWorkItems(STORY)
  console.log(`after dispatch: status=Ready activeWorkItems=${afterDispatch.length}`)
  for (const w of afterDispatch) console.log(`  -> ${w.id} ${w.state}`)

  // 2. PULL IT BACK — exactly what moving the note out of ENGINE RUN Q now does.
  const { withdrawn, live } = await withdrawQueuedAgentWork(STORY)
  console.log(`withdraw: withdrawn=${withdrawn} live=${live}`)
  console.log(`after withdraw: activeWorkItems=${(await activeWorkItems(STORY)).length}`)

  // 3. RESTORE the story exactly as found.
  await setStoryboardStatus(STORY, before?.status ?? 'In Progress')
  const restored = await getStoryboardStory(STORY)
  console.log(`RESTORED: status=${restored?.status} activeWorkItems=${(await activeWorkItems(STORY)).length}`)
}

void main()
