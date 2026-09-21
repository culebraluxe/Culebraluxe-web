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
