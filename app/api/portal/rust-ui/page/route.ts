import { NextResponse, type NextRequest } from 'next/server'

import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import { accountingPayload, isAccountingScreen } from '@/lib/portal-rust-ui/accounting-payload'
import { isSupportScreen, supportPayload } from '@/lib/portal-rust-ui/support-payload'
import { rustApiRead, rustApiTechCockpit } from '@/lib/rust-api/client'
import { buildStoryBoardCockpit, buildStoryBoardModel } from '@/lib/storyboard-data'

// A portal screen's payload in the screen's own shape — never flattened generic cells.
async function GETHandler(req: NextRequest): Promise<Response> {
  const actor = await getPortalActingUser()
  if (!actor) return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })

  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  // Every Accounting screen reads the Rust service, so they are answered before the switch rather than as five arms.
  if (isAccountingScreen(screen)) {
    return NextResponse.json(
      await accountingPayload(screen, {
        from: req.nextUrl.searchParams.get('from'),
        to: req.nextUrl.searchParams.get('to'),
      }),
    )
  }
  // SUPPORT, the same way: four diagnostic screens, answered from the projections that already define them.
  if (isSupportScreen(screen)) {
    return NextResponse.json(
      await supportPayload(screen, req.nextUrl.searchParams.get('scope')),
    )
  }
  switch (screen) {
    case 'storyboard': {
      const snapshot = await rustApiTechCockpit()
      const stories = Array.isArray(snapshot?.stories) ? snapshot.stories : []
      const cockpit = buildStoryBoardCockpit(buildStoryBoardModel(stories))
      const panel = (bucket: 'open' | 'backlog' | 'closed' | 'next-version') => {
        const source = cockpit.panels[bucket]
        return {
          bucket: source.bucket,
          count: source.count,
          groups: source.groups.map((group) => ({
            group: group.group,
            stories: group.stories.map((story) => ({
              id: story.id,
              title: story.title,
              priority: story.priority,
              status: story.status,
              completion: story.completion,
            })),
          })),
        }
      }

      return NextResponse.json({
        storyboard: {
          kpis: cockpit.kpis,
          panels: {
            open: panel('open'),
            backlog: panel('backlog'),
            closed: panel('closed'),
            nextVersion: panel('next-version'),
          },
        },
      })
    }
    case 'activity': {
      const entries = await rustApiRead<Array<{
        id: string
        channel: string
        direction: string | null
        occurredAtLabel: string
        title: string | null
        summary: string | null
        personId: string | null
        personName: string | null
        propertyName: string | null
        dealId: string | null
        dealPropertyName: string | null
      }>>('/v1/activity?limit=50' as `/v1/${string}`)
      return NextResponse.json({
        activity: entries.value.map((entry) => ({
          id: entry.id,
          channel: entry.channel,
          direction: entry.direction,
          occurredAtLabel: entry.occurredAtLabel,
          title: entry.title,
          summary: entry.summary,
          personId: entry.personId,
          personName: entry.personName,
          propertyName: entry.propertyName,
          dealId: entry.dealId,
          dealPropertyName: entry.dealPropertyName,
        })),
      })
    }
    case 'workflows':
    case 'tech-flight-recorder': {
      const result = await rustApiRead<Record<string, unknown>>('/v1/workflows')
      return NextResponse.json({ workflows: result.value })
    }
    case 'workflow-record': {
      const scope = req.nextUrl.searchParams.get('scope')
      if (!scope) return NextResponse.json({ error: 'workflow-record requires scope.' }, { status: 400 })

      const result = await rustApiRead<Record<string, unknown>>(
        (`/v1/workflows/${encodeURIComponent(scope)}`) as `/v1/${string}`,
      )
      return NextResponse.json({ workflow: result.value })
    }
    default:
      return NextResponse.json({ error: `no portal payload source for screen '${screen}'` }, { status: 400 })
  }
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/page', route: '/api/portal/rust-ui/page' },
  GETHandler,
)
