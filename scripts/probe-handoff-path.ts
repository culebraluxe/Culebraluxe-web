// PROOF, on PROD, of the two handoffs the captain called "the big thing": OPEN -> ENGINE RUN Q and
// WORK BENCE -> ENGINE RUN Q. Net zero: each handoff is withdrawn and the story restored exactly.
import { listActiveAgentWorkForStory, withdrawQueuedAgentWork } from '@/db/agent-work'
import {
  getStoryboardStory,
  listActiveWork as bench,
  setActiveWork,
  setStoryboardStatus,
} from '@/db/storyboard'

const STORY = 'ENG-FORGE-DOCTOR-01'

const state = async () => ({
  status: (await getStoryboardStory(STORY))?.status,
  bench: (await bench()).some((s) => s.id === STORY),
  items: (await listActiveAgentWorkForStory(STORY)).length,
})

async function handoffAndWithdraw(label: string) {
  console.log(`\n== ${label}`)
  console.log('  before    ', await state())
  // The action's writes for target 'engine': clear the bench intent (always), then dispatch.
  await setActiveWork(STORY, false, null)
  await setStoryboardStatus(STORY, 'Ready')
  console.log('  handoff   ', await state())
  const { withdrawn, live } = await withdrawQueuedAgentWork(STORY)
  console.log(`  withdraw   withdrawn=${withdrawn} live=${live}`, await state())
}

async function main() {
  const original = await state()
  console.log('ORIGINAL  ', original)

  // 1. OPEN -> ENGINE RUN Q (story starts off the bench, In Progress).
  await setActiveWork(STORY, false, null)
  await setStoryboardStatus(STORY, 'In Progress')
  await handoffAndWithdraw('OPEN -> ENGINE RUN Q')

  // 2. WORK BENCH -> ENGINE RUN Q (story starts ON the bench, status still In Progress).
  await setActiveWork(STORY, true, null)
  await setStoryboardStatus(STORY, 'In Progress')
  await handoffAndWithdraw('WORK BENCH -> ENGINE RUN Q')

  // RESTORE exactly what was found.
  await setStoryboardStatus(STORY, original.status ?? 'In Progress')
  if (original.bench) await setActiveWork(STORY, true, null)
  else await setActiveWork(STORY, false, null)
  console.log('\nRESTORED  ', await state(), `(original: ${JSON.stringify(original)})`)
}

void main()
