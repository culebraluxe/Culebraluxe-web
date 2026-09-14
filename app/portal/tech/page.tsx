import Link from "next/link"
import { redirect } from "next/navigation"

import { EngineeringQueuesPage } from "@/components/portal/tech/engineering-line"
import { ForgeConvergenceView } from "@/components/portal/tech/forge-convergence-view"
import { StoryBoardNotReady } from "@/components/portal/story-board"
import { listForgeConvergence } from "@/db/forge-convergence"
import {
  latestForgeInstanceForStory,
  listEngineRunCards,
  listEngineLedgerStats,
  listEngineQueuedCards,
} from "@/db/forge-engine-task-execution"
import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { cockpitVersionLabel } from "@/lib/cockpit-version"
import { listForgeBatches } from "@/db/forge-batch"
import {
  buildStoryBoardCockpit,
  buildStoryBoardModel,
} from "@/lib/storyboard-data"
import type { HistoryStory } from "@/components/portal/tech/engineering-cockpit"
import {
  listActiveWork,
  listStoryExecutionSummaries,
  listStoryRuns,
  listStoryboardStories,
} from "@/db/storyboard"

import { latestOpenForgeHold } from "@/db/forge-hold"

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
  // The Flight Recorder's key: the engine instance that ran (or is running) the selected
  // story. Null when the engine has not touched it, and the screen then hides the link
  // instead of offering an empty console.
  const recorderInstanceId = validId
    ? await latestForgeInstanceForStory(validId).catch(() => null)
    : null
  const freshness =
    withExecution.reduce((m, s) => (s.updatedAt > m ? s.updatedAt : m), "") ||
    new Date().toISOString()

  // WHICH STORIES HAVE ACTUALLY BEEN RUN?
  //
  // Every story already carries its execution summary here (one query, already loaded), so the
  // answer costs nothing — and without it the operator has to guess an id out of 323 rows to find
  // the handful the engine has touched. Newest run first, capped: this is a signpost, not a report.
  const historyStories: HistoryStory[] = withExecution
    .filter((s) => s.execution)
    .map((s) => ({
      id: s.id,
      title: s.title,
      latestRunAt: s.execution?.latestRunAt ?? null,
      latestRunResult: s.execution?.latestRunResult ?? null,
    }))
    .sort((a, b) => (b.latestRunAt ?? '').localeCompare(a.latestRunAt ?? ''))
    // The signpost is capped; the badge set below is NOT, or a bench story with real history
    // would go unmarked just because it was not among the newest twelve.
    .slice(0, 12)
  const historyStoryIds = withExecution.filter((s) => s.execution).map((s) => s.id)
  // The total, so the signpost can say how many exist rather than implying its capped list is all.
  const historyTotal = historyStoryIds.length

  // ENGINE BATCH: stories staged for the next group handed to Forge. Staged means status 'Batched',
  // which dispatches nothing - `sendEngineBatchAction` is the deliberate act that queues them.
  const batchStories = (stories ?? [])
    .filter((s) => s.status === 'Batched')
    .map((s) => ({
      id: s.id,
      title: s.title,
      status: s.status,
      priority: s.priority,
      completion: s.completion,
    }))

  // The engine's lanes are read from the engine ledger. A failure here is reported IN THE LANE
  // rather than silently leaving fixture cards standing in for engine output; the read is also
  // captured by the database gateway on the way through.
  const engineRuns = await listEngineRunCards(24).catch(() => null)
  // The stats strip and the ENGINE QUEUED lane read the ledger and the work items directly. Null
  // means the read failed, and the screen says so instead of showing numbers it did not measure.
  const ledgerStats = await listEngineLedgerStats().catch(() => null)
  const queuedCards = await listEngineQueuedCards(20).catch(() => null)
  // The engine's current stop for the story on screen, so a parked story reads as parked.
  const hold = validId ? await latestOpenForgeHold(validId).catch(() => null) : null

  return (
    <div className="min-h-screen bg-[#0b1220]">
      <div className="flex justify-end gap-4 px-4 pt-3 lg:px-6">
        <Link
          href="/portal/tech/flight-recorder"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/55 hover:text-[var(--portal-navy)]"
        >
          Flight Recorder →
        </Link>
        <Link
          href="/portal/tech/runs"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/55 hover:text-[var(--portal-navy)]"
        >
          Run Outcomes →
        </Link>
        <Link
          href="/portal/tech/app-errors"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/55 hover:text-[var(--portal-navy)]"
        >
          App Error Capture →
        </Link>
      </div>
      <EngineeringQueuesPage
        cockpit={cockpit}
        versionLabel={cockpitVersionLabel()}
        batches={await listForgeBatches(5)}
        activeWork={activeQueue}
        selectedStory={selectedStory}
        selectedIsActive={selectedIsActive}
        runs={runs}
        freshness={freshness}
        recorderInstanceId={recorderInstanceId}
        historyStories={historyStories}
        historyStoryIds={historyStoryIds}
        historyTotal={historyTotal}
        engineRuns={engineRuns}
        ledgerStats={ledgerStats}
        queuedCards={queuedCards}
        hold={hold}
        batchStories={batchStories}
      />
      <div className="px-5 pb-8">
        <ForgeConvergenceView items={convergence} />
      </div>
    </div>
  )
}
