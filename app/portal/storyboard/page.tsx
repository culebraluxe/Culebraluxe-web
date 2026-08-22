import { StoryBoard, StoryBoardNotReady } from "@/components/portal/story-board"
import { listStoryboardStories } from "@/db/storyboard"
import {
  buildStoryBoardModel,
  filterStories,
  parseStoryBoardFilter,
  selectNextWork,
} from "@/lib/storyboard-data"

export const dynamic = "force-dynamic"

export default async function StoryBoardPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
  const params = await searchParams
  const stories = await listStoryboardStories()

  if (!stories) {
    return <StoryBoardNotReady />
  }

  const model = buildStoryBoardModel(stories)
  const filter = parseStoryBoardFilter(params)
  const visibleStories = filterStories(model.stories, filter)
  // OPS-08: bounded Next Work projection derived from the authoritative board.
  const nextWork = selectNextWork(stories)

  return (
    <StoryBoard
      model={model}
      filter={filter}
      visibleStories={visibleStories}
      nextWork={nextWork}
    />
  )
}
