import Link from "next/link"
import { redirect } from "next/navigation"

import { EngineeringQueuesPage } from "@/components/portal/tech/engineering-line"
import { ForgeConvergenceView } from "@/components/portal/tech/forge-convergence-view"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import { listForgeConvergence } from "@/db/forge-convergence"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import {
  buildStoryBoardCockpit,
  buildStoryBoardModel,
} from "@/lib/storyboard-data"
import {
  listActiveWork,
  listStoryExecutionSummaries,
  listStoryRuns,
  listStoryboardStories,
} from "@/db/storyboard"

export const dynamic = "force-dynamic"

export default async function TechPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!access.ok) redirect(access.redirectTo)

  const params = await searchParams
  const selectedId = typeof params.story === "string" ? params.story : null

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
  const activeQueue = await listActiveWork()
  const convergence = await listForgeConvergence()

  // The Work Bench always has a story selected, so the detail pane (and the
  // GOOD TO GO / MOVE TO controls) are never a blank rectangle on arrival: an
  // explicit ?story wins, otherwise the first item on the bench. The old TECH page
  // had NO fallback — it only selected after a click — which is why the bench
  // arrived empty when this screen moved to the TECH root.
  const fallbackId = activeQueue[0]?.id ?? withExecution[0]?.id ?? null
  const validId =
    selectedId && withExecution.some((s) => s.id === selectedId)
      ? selectedId
      : fallbackId
  const selectedStory = validId
    ? (withExecution.find((s) => s.id === validId) ?? null)
    : null
  const selectedIsActive = validId
    ? activeQueue.some((s) => s.id === validId)
    : false
  const runs = validId ? await listStoryRuns(validId) : []
  const freshness =
    withExecution.reduce((m, s) => (s.updatedAt > m ? s.updatedAt : m), "") ||
    new Date().toISOString()

  return (
    <div className="min-h-screen bg-[#0b1220]">
      <div className="flex justify-end px-4 pt-3 lg:px-6">
        <Link
          href="/portal/tech/app-errors"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/55 hover:text-[var(--portal-navy)]"
        >
          App Error Capture →
        </Link>
      </div>
      <EngineeringQueuesPage
        cockpit={cockpit}
        activeWork={activeQueue}
        selectedStory={selectedStory}
        selectedIsActive={selectedIsActive}
        runs={runs}
        freshness={freshness}
      />
      <div className="px-5 pb-8">
        <ForgeConvergenceView items={convergence} />
      </div>
    </div>
  )
}
