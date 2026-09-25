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

  const snapshot = await rustApiTechCockpit(selectedRequested)
  const stories = snapshot.stories ?? []
  const executions = snapshot.executions ?? []
  const activeWork = snapshot.activeWork ?? []
  const engineRuns = snapshot.engineRuns ?? []
  const ledger = snapshot.ledger ?? null
  const queuedCards = snapshot.queuedCards ?? []
  const flights = snapshot.recentFlights ?? []
  const stagingFlight = snapshot.stagingFlight ?? null
  const stagingItems = snapshot.stagingItems ?? []

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

  const selectedRuns = snapshot.selectedRuns ?? []
  const recorderInstanceId = snapshot.recorderInstanceId ?? null
  const hold = snapshot.hold ?? null

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
    return NextResponse.json({ ok: false, error: 'Operating the Engineering Cockpit requires TECH access.' }, { status: 401 })
  }
  const body = (await req.json().catch(() => null)) as TechCommandBody | null
  if (!body?.action) return badCommand('Missing TECH Cockpit command.')
  const result = await rustApiTechCommand(body as Record<string, unknown>)
  return NextResponse.json(result)
}

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/tech.POST', route: '/api/portal/rust-ui/tech' },
  POSTHandler,
)
