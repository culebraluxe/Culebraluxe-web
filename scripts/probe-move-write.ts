// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// Reproduce the ACTION's write sequence for one story against PROD, so a throw in the DB path shows its
// real message instead of React's minified #441. This performs the user's intended move (bench -> open).
import { setActiveWork, setStoryboardStatus, listActiveWork, getStoryboardStory } from '@/legacy/db/storyboard'

const STORY = process.argv[2] ?? 'ENG-FORGE-V5-21'

async function main() {
  const before = await getStoryboardStory(STORY)
  const onBench = (await listActiveWork()).some((s) => s.id === STORY)
  console.log(`before: status=${before?.status} onBench=${onBench}`)
  try {
    console.log('step 1: clear the bench intent')
    await setActiveWork(STORY, false, null)
    console.log('step 2: write the status (OPEN = In Progress)')
    await setStoryboardStatus(STORY, 'In Progress')
    const after = await getStoryboardStory(STORY)
    const stillBench = (await listActiveWork()).some((s) => s.id === STORY)
    console.log(`RESULT: status=${after?.status} onBench=${stillBench}`)
  } catch (error) {
    console.log('THREW:', String((error as Error)?.message ?? error))
    console.log(String((error as Error)?.stack ?? '').split('\n').slice(0, 6).join('\n'))
  }
}

void main()
