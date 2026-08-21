import { StoryBoard, StoryBoardNotReady } from "@/components/portal/story-board"
import { listStoryboardStories } from "@/db/storyboard"
import { buildStoryBoardModel } from "@/lib/storyboard-data"

export const dynamic = "force-dynamic"

export default async function StoryBoardPage() {
  const stories = await listStoryboardStories()

  if (!stories) {
    return <StoryBoardNotReady />
  }

  const model = buildStoryBoardModel(stories)

  return <StoryBoard model={model} />
}
