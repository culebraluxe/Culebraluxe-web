import { StoryBoard } from "@/components/portal/story-board"
import { buildStoryBoardModel } from "@/lib/storyboard-data"

export default function StoryBoardPage() {
  const model = buildStoryBoardModel()

  return <StoryBoard model={model} />
}
