import { NextResponse, type NextRequest } from 'next/server'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { withApiHandler } from '@/lib/error-capture-seam'
import {
  clearActiveWork,
  listActiveWork,
  listStoryExecutionSummaries,
  listStoryIdsWithStatus,
  listStoryRuns,
  listStoryboardStories,
  setActiveWork,
  setStoryboardStatus,
} from '@/legacy/db/storyboard'
import {
  latestForgeInstanceForStory,
  listEngineLedgerStats,
  listEngineQueuedCards,
  listEngineRunCards,
} from '@/legacy/db/forge-engine-task-execution'
import {
  cancelForgeBatch,
  fireStagingBatch,
  getStagingBatch,
  listForgeBatches,
  listStagingBatchItems,
  scheduleStagingBatch,
} from '@/legacy/db/forge-batch'
import { latestOpenForgeHold } from '@/legacy/db/forge-hold'
import { setAgentWorkDispatchOptions } from '@/legacy/db/agent-work'
import { buildStoryBoardCockpit, buildStoryBoardModel } from '@/lib/storyboard-data'
import { buildSorterCards, SORTER_COLUMNS } from '@/lib/sorter-board'
import { ENGINE_DISPATCH_STATUS, STATUS_BY_BUCKET } from '@/lib/story-moves'

function storyPayload(story: any) {
  return {
    id: String(story.id ?? ''),
    workstream: String(story.workstream ?? ''),
    operatingSurface: story.operatingSurface ?? null,
    title: String(story.title ?? ''),
    priority: String(story.priority ?? ''),
    status: String(story.status ?? ''),
    notes: story.notes ?? null,
    batch: story.batch ?? null,
    goal: story.goal ?? null,
    scope: story.scope ?? null,
    dependencies: story.dependencies ?? null,
    preconditions: story.preconditions ?? null,
    architectBrief: story.architectBrief ?? null,
    contextRefs: story.contextRefs ?? null,
    acceptanceCriteria: story.acceptanceCriteria ?? null,
    postconditions: story.postconditions ?? null,
    completion: Number(story.completion ?? 0),
    updatedAt: String(story.updatedAt ?? ''),
  }
}

function runPayload(run: any) {
  return {
    id: String(run.id ?? ''),
    startedAt: String(run.startedAt ?? ''),
    endedAt: run.endedAt ?? null,
    resultStatus: run.resultStatus ?? null,
    runType: run.runType ?? null,
    agentRuntime: run.agentRuntime ?? null,
    completion: run.completion == null ? null : Number(run.completion),
    notes: run.notes ?? null,
    commitHash: run.commitHash ?? null,
    testsSummary: run.testsSummary ?? null,
    executionEnvironment: run.executionEnvironment ?? null,
    runPhase: run.runPhase ?? null,
    leadDecision: run.leadDecision ?? null,
    modelUsed: run.modelUsed ?? null,
    costWidgets: run.costWidgets == null ? null : Number(run.costWidgets),
  }
}

function flightPayload(batch: any) {
  return {
    id: String(batch.id ?? ''),
    label: batch.label ?? null,
    status: String(batch.status ?? ''),
    scheduledFor: batch.scheduledFor ?? null,
    firedAt: batch.firedAt ?? null,
    createdAt: String(batch.createdAt ?? ''),
    modelPolicy: String(batch.modelPolicy ?? 'cheap'),
    storyCount: Number(batch.storyCount ?? 0),
    queuedCount: Number(batch.queuedCount ?? 0),
    skippedCount: Number(batch.skippedCount ?? 0),
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'tech.access')
  if (!access.ok) {
    return NextResponse.json(
      { error: 'Reading the Engineering Cockpit requires TECH access.' },
      { status: 401 },
    )
  }

  const selectedRequested = new URL(req.url).searchParams.get('selected')

  const [stories, executions, activeWork, engineRuns, ledger, queuedCards, flights, stagingFlight, stagingItems] =
    await Promise.all([
      listStoryboardStories(),
      listStoryExecutionSummaries(),
      listActiveWork(),
      listEngineRunCards(30).catch(() => null),
      listEngineLedgerStats().catch(() => null),
      listEngineQueuedCards(30).catch(() => null),
      listForgeBatches(4).catch(() => []),
      getStagingBatch().catch(() => null),
      listStagingBatchItems().catch(() => []),
    ])

  if (!stories) {
    return NextResponse.json({
      tech: {
        ready: false,
        activeWork: [],
        selectedRuns: [],
        sorterCards: [],
        sorterColumns: [],
        engineRuns: [],
        queuedCards: [],
        recentFlights: [],
        recentHistory: [],
      },
    })
  }

  const executionByStory = new Map(executions.map((item) => [item.storyId, item]))
  const withExecution = stories.map((story) => ({
    ...story,
    execution: executionByStory.get(story.id) ?? null,
  }))
  const board = buildStoryBoardModel(withExecution)
  const cockpit = buildStoryBoardCockpit(board)

  const fallbackId = activeWork[0]?.id ?? withExecution[0]?.id ?? null
  const selectedId =
    selectedRequested && withExecution.some((story) => story.id === selectedRequested)
      ? selectedRequested
      : fallbackId
  const selectedStory = selectedId
    ? withExecution.find((story) => story.id === selectedId) ?? null
    : null

  const selectedRuns = selectedId ? await listStoryRuns(selectedId).catch(() => []) : []
  const recorderInstanceId = selectedId
    ? await latestForgeInstanceForStory(selectedId).catch(() => null)
    : null
  const hold = selectedId ? await latestOpenForgeHold(selectedId).catch(() => null) : null

  const kindByStory = new Map(stagingItems.map((item) => [item.storyId, item.kind]))
  const batchStories = stories
    .filter((story) => story.status === 'Batched')
    .map((story) => ({ ...story, kind: kindByStory.get(story.id) ?? null }))

  const engineSorterCards = (engineRuns ?? []).map((run) => {
    const live =
      !run.stale &&
      (run.status === 'running' || run.status === 'claimed' || run.status === 'queued')
    return {
      id: run.storyId + '#' + String(run.attempts),
      storyId: run.storyId,
      title: run.title,
      status: run.status,
      priority: run.status === 'failed' ? 'HIGH' : 'MEDIUM',
      completion: run.status === 'completed' ? 100 : 0,
      queue: live ? 'running' : 'results',
    }
  })

  const sorterCards = buildSorterCards({
    panels: cockpit.panels as any,
    activeWork,
    batchStories,
    engineRuns: engineSorterCards,
    queuedCards: queuedCards ?? [],
  })

  const recentHistory = withExecution
    .filter((story) => story.execution?.latestRunAt)
    .map((story) => ({
      id: story.id,
      title: story.title,
      latestRunAt: story.execution?.latestRunAt ?? null,
      latestRunResult: story.execution?.latestRunResult ?? null,
    }))
    .sort((left, right) => (right.latestRunAt ?? '').localeCompare(left.latestRunAt ?? ''))
    .slice(0, 4)

  const freshness =
    withExecution.reduce(
      (latest, story) => (story.updatedAt > latest ? story.updatedAt : latest),
      '',
    ) || new Date().toISOString()

  return NextResponse.json({
    tech: {
      ready: true,
      totalStories: cockpit.kpis.total,
      openCount: cockpit.kpis.open,
      backlogCount: cockpit.kpis.backlog,
      closedCount: cockpit.kpis.complete,
      completionPercent: cockpit.kpis.completionPercent,
      activeWork: activeWork.map(storyPayload),
      selectedStory: selectedStory ? storyPayload(selectedStory) : null,
      selectedRuns: selectedRuns.slice(0, 8).map(runPayload),
      recorderInstanceId,
      hold,
      sorterCards,
      sorterColumns: SORTER_COLUMNS.map((column) => ({
        id: column.id,
        label:
          column.id === 'batch'
            ? 'FLIGHT STAGING'
            : column.id === 'engine'
              ? 'ENGINE RUN Q'
              : column.label,
      })),
      engineRuns: engineRuns ?? [],
      queuedCards: queuedCards ?? [],
      engineReadOk: engineRuns !== null,
      queueReadOk: queuedCards !== null,
      ledger: ledger
        ? {
            totalAttempts: ledger.totalAttempts,
            stories: ledger.stories,
            completed: ledger.latest.completed,
            failed: ledger.latest.failed,
            interrupted: ledger.latest.interrupted,
            worstStoryId: ledger.worstOffender?.storyId ?? null,
            worstAttempts: ledger.worstOffender?.attempts ?? null,
            asOf: ledger.asOf,
          }
        : null,
      stagingFlight: stagingFlight ? flightPayload(stagingFlight) : null,
      recentFlights: flights.map(flightPayload),
      recentHistory,
      freshness,
    },
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/tech', route: '/api/portal/rust-ui/tech' },
  GETHandler,
)


type TechCommandBody = {
  action?: string
  storyId?: string
  stopAfter?: string
  scheduledFor?: string
  batchId?: string
  target?: string
}

function badCommand(message: string, status = 400): Response {
  return NextResponse.json({ ok: false, error: message }, { status })
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'tech.access')
  if (!access.ok) {
    return NextResponse.json(
      { ok: false, error: 'Operating the Engineering Cockpit requires TECH access.' },
      { status: 401 },
    )
  }

  const body = (await req.json().catch(() => null)) as TechCommandBody | null
  if (!body?.action) return badCommand('Missing TECH Cockpit command.')

  const actorId = access.actor.appUserId
  const storyId = String(body.storyId ?? '').trim()

  switch (body.action) {
    case 'clearWorkbench': {
      const cleared = await clearActiveWork()
      return NextResponse.json({
        ok: true,
        message: 'Cleared ' + cleared + ' stor' + (cleared === 1 ? 'y' : 'ies') + ' from the Workbench. Story statuses were not changed.',
      })
    }

    case 'goodToGo': {
      if (!storyId) return badCommand('Missing story id.')
      // Same handoff as the proven sorter move: once the human gives the story to Forge, it is no longer daily intent.
      await setActiveWork(storyId, false, actorId)
      await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)
      return NextResponse.json({
        ok: true,
        message: storyId + ' handed to Forge. Ready queued a real work item.',
      })
    }

    case 'scopedRun': {
      if (!storyId) return badCommand('Missing story id.')
      const stopAfter = String(body.stopAfter ?? '').trim()
      if (!['scout', 'architect', 'lead'].includes(stopAfter)) {
        return badCommand('Unsupported scoped stop: ' + stopAfter)
      }

      // Deliberately DO NOT clear storyboard_active_work. A scoped investigation belongs to the Workbench:
      // Forge goes only as far as requested, then the operator reads the findings before deciding what happens next.
      await setStoryboardStatus(storyId, ENGINE_DISPATCH_STATUS)
      const updated = await setAgentWorkDispatchOptions(storyId, {
        stopAfter,
        launchIntent: null,
      })
      if (updated === 0) {
        return badCommand(
          'No queued work item to scope — this story is already running.',
          409,
        )
      }
      return NextResponse.json({
        ok: true,
        message: storyId + ' queued through ' + stopAfter + '; it remains on the Workbench for review.',
      })
    }

    case 'moveWorkbench': {
      if (!storyId) return badCommand('Missing story id.')
      const target = String(body.target ?? '').trim()
      const status =
        target === 'backlog'
          ? STATUS_BY_BUCKET.backlog
          : target === 'closed'
            ? STATUS_BY_BUCKET.closed
            : target === 'next'
              ? STATUS_BY_BUCKET.next
              : null
      if (!status) return badCommand('Unsupported Workbench destination: ' + target)

      await setActiveWork(storyId, false, actorId)
      await setStoryboardStatus(storyId, status)
      return NextResponse.json({
        ok: true,
        message: storyId + ' moved to ' + (target === 'next' ? 'Next Version' : target) + '.',
      })
    }

    case 'launchFlight': {
      const result = await fireStagingBatch()
      if (!result) {
        return NextResponse.json({ ok: false, message: 'Nothing is staged in the current Flight.' })
      }
      const refused = result.failed.map((failure) => failure.storyId)
      return NextResponse.json({
        ok: refused.length === 0,
        message:
          refused.length === 0
            ? 'Flight launched: ' + result.queued + ' stor' + (result.queued === 1 ? 'y' : 'ies') + ' queued for Forge.'
            : 'Flight launched ' + result.queued + '; refused ' + refused.join(', ') + '.',
      })
    }

    case 'scheduleFlight': {
      const raw = String(body.scheduledFor ?? '').trim()
      const when = new Date(raw)
      if (!raw || Number.isNaN(when.getTime())) {
        return badCommand('The Flight time could not be read.')
      }

      const staged = await listStoryIdsWithStatus(STATUS_BY_BUCKET.batch ?? 'Batched')
      if (staged.length === 0) {
        return badCommand('Nothing is staged in the current Flight.')
      }

      const batch = await scheduleStagingBatch(
        when.toISOString(),
        actorId,
        'night run ' + when.toISOString().slice(0, 16).replace('T', ' '),
      )
      return NextResponse.json({
        ok: true,
        message:
          'Scheduled ' +
          batch.storyCount +
          ' stor' +
          (batch.storyCount === 1 ? 'y' : 'ies') +
          ' for ' +
          String(batch.scheduledFor ?? when.toISOString()) +
          '.',
      })
    }

    case 'cancelFlight': {
      const batchId = String(body.batchId ?? '').trim()
      if (!batchId) return badCommand('Missing Flight id.')
      const cancelled = await cancelForgeBatch(batchId)
      if (cancelled === 0) {
        return badCommand('That Flight is not waiting to fire.', 409)
      }
      return NextResponse.json({ ok: true, message: 'Scheduled Flight cancelled. Nothing was dispatched.' })
    }

    default:
      return badCommand('Unknown TECH Cockpit command: ' + body.action)
  }
}

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/tech.POST', route: '/api/portal/rust-ui/tech' },
  POSTHandler,
)
