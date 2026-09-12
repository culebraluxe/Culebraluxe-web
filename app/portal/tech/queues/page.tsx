import { Suspense } from "react"
import { redirect } from "next/navigation"

import { EngineeringQueuesPage } from "@/components/portal/tech/engineering-line"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import {
  listActiveWork,
  listStoryExecutionSummaries,
  listStoryRuns,
  listStoryboardStories,
} from "@/db/storyboard"
import { buildStoryBoardCockpit, buildStoryBoardModel } from "@/lib/storyboard-data"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// ENGINEERING QUEUES — the working screen.
//
// REAL data, loaded the same way the TECH cockpit already loads it, because all
// four queues already have a home and no new table is needed:
//   WORK BENCH  — `storyboard_active_work` (listActiveWork, ordered by work_order)
//   ENGINE READY / RUNNING — `agent_work_item` states (wiring next)
//   RESULTS     — `storyboard_story_run` (result_status IS the outcome; 454 attempts)
//   story log   — buildStoryBoardCockpit(model): the four buckets, grouped
//   tiles       — the same cockpit projection, so the strip cannot disagree with
//                 the boxes underneath it.
//
// The middle queue CARDS are still the fixture sample (labelled on screen); the
// bench, the tiles and the story log are live.
//
// When this earns its place it REPLACES /portal/tech, and command-center +
// command-console + runtime-inspector come down with it. Grok's Engineering Line
// reference at /portal/tech/line stays frozen and untouched for comparison.
export default async function EngineeringQueuesRoute({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const params = await searchParams
  const requestedId = typeof params.story === "string" ? params.story : null

  const [stories, executions, activeWork] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
    listActiveWork(),
  ])
  if (!stories) return <StoryBoardNotReady />

  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))

  const model = buildStoryBoardModel(withExecution)
  const cockpit = buildStoryBoardCockpit(model)

  // The Work Bench always has a story selected, so the detail pane is never a
  // blank rectangle: an explicit ?story wins, otherwise the first bench item.
  const fallbackId = activeWork[0]?.id ?? withExecution[0]?.id ?? null
  const validId =
    requestedId && withExecution.some((s) => s.id === requestedId) ? requestedId : fallbackId
  const selectedStory = validId
    ? (withExecution.find((s) => s.id === validId) ?? null)
    : null
  const selectedIsActive = Boolean(validId && activeWork.some((s) => s.id === validId))
  const runs = validId ? await listStoryRuns(validId) : []
  const freshness = `${new Date().toISOString().slice(0, 16).replace("T", " ")} UTC`

  return (
    <Suspense
      fallback={
        <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
          Loading queues…
        </div>
      }
    >
      <EngineeringQueuesPage
        cockpit={cockpit}
        activeWork={activeWork}
        selectedStory={selectedStory}
        selectedIsActive={selectedIsActive}
        runs={runs}
        freshness={freshness}
      />
    </Suspense>
  )
}
