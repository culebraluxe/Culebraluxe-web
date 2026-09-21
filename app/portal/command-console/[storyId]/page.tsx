import { notFound } from 'next/navigation'

import { StoryExecutionCockpit } from '@/components/portal/story-execution-cockpit'
import { getStoryExecutionCockpit } from '@/lib/command-console-data'

export const dynamic = 'force-dynamic'

// ---------------------------------------------------------------------------
// Story Execution Cockpit (ENG-20) — child/detail screen over one story's
// canonical control-plane data (storyboard_story + agent_work_item +
// storyboard_story_run). Server-rendered; the client component is a read
// projection (no new state machine).
// ---------------------------------------------------------------------------

export default async function StoryExecutionCockpitPage({
  params,
}: {
  params: Promise<{ storyId: string }>
}) {
  const { storyId } = await params
  const model = await getStoryExecutionCockpit(storyId)

  if (!model.ready || !model.story) {
    notFound()
  }

  return <StoryExecutionCockpit model={model} />
}
