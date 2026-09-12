import { Suspense } from "react"
import { redirect } from "next/navigation"

import { EngineeringQueuesPage } from "@/components/portal/tech/engineering-line"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import {
  listActiveWork,
  listStoryExecutionSummaries,
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
export default async function EngineeringQueuesRoute() {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const [stories, executions] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
  ])
  if (!stories) return <StoryBoardNotReady />

  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))

  const model = buildStoryBoardModel(withExecution)
  const cockpit = buildStoryBoardCockpit(model)
  const activeWork = await listActiveWork()

  return (
    <Suspense
      fallback={
        <div className="grid h-screen place-items-center bg-[#0b1220] text-sm text-slate-400">
          Loading queues…
        </div>
      }
    >
      <EngineeringQueuesPage cockpit={cockpit} activeWork={activeWork} />
    </Suspense>
  )
}
