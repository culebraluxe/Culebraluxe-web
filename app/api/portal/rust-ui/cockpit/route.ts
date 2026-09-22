import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiEngineCommand, rustApiRead } from '@/lib/rust-api/client'

type CockpitSnapshot = {
  activeClientCount: number
  liveDealCount: number
  upcomingCount: number
  underContractCount: number
  activeWorkflowCount: number
  blockedWorkflowCount: number
  overdueTasks: unknown[]
  tasksDueSoon: unknown[]
  recentInteractions: unknown[]
  featuredDeal: unknown | null
  pipeline: unknown[]
}

async function payload() {
  const result = await rustApiRead<CockpitSnapshot>('/v1/cockpit')
  return { cockpit: result.value }
}

async function GETHandler(): Promise<Response> {
  return NextResponse.json(await payload())
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const input = (await req.json()) as { action?: string; taskId?: string }
  if (input.action !== 'completeTask') {
    return NextResponse.json({ error: 'Unsupported Cockpit action.' }, { status: 400 })
  }
  const taskId = input.taskId?.trim()
  if (!taskId) {
    return NextResponse.json({ error: 'taskId is required.' }, { status: 400 })
  }

  await rustApiEngineCommand('/v1/engine/tasks/complete', {
    task: taskId,
    kind: 'application',
  })

  return NextResponse.json(await payload())
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/cockpit', route: '/api/portal/rust-ui/cockpit' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/cockpit', route: '/api/portal/rust-ui/cockpit' },
  POSTHandler,
)
