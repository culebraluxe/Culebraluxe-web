import { StoryBoard, StoryBoardNotReady } from "@/components/portal/story-board"
import {
  listStoryboardStories,
  listStoryExecutionSummaries,
} from "@/db/storyboard"
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
  const [stories, executions] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
  ])

  if (!stories) {
    return <StoryBoardNotReady />
  }

  // Attach the Forge execution projection (latest work item + latest run) so
  // the board can show what is Running, Done, Error/Cancelled, and when the
  // latest run happened — without conflating it with story status.
  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))

  const model = buildStoryBoardModel(withExecution)
  const filter = parseStoryBoardFilter(params)
  const visibleStories = filterStories(model.stories, filter)
  // OPS-08: bounded Next Work projection derived from the authoritative board.
  const nextWork = selectNextWork(withExecution)

  return (
    <StoryBoard
      model={model}
      filter={filter}
      visibleStories={visibleStories}
      nextWork={nextWork}
    />
  )
}
