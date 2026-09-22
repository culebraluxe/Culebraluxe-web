import { NextResponse, type NextRequest } from 'next/server'

import { getActivityFeed } from '@/legacy/db/activity-feed'
import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// A PORTAL SCREEN'S PAYLOAD, IN THE SCREEN'S OWN SHAPE.
//
// WHY THIS IS NOT THE ROWS ROUTE. That route answers with `{ id, cells, badge }` — a column list, which is the right
// transport for a table and the wrong one for a screen. The Activity feed renders a channel, a direction, the person,
// the summary, and the property or deal a line belongs to, and the person and deal are LINKS, so they need their ids.
// Flattening those into five cells threw away everything but the words, which is why the ported screens looked like
// generic lists: they were being handed generic lists.
//
// ONE CASE PER PORTED SCREEN, added as the screen arrives, and each case reads the same read model the live TypeScript
// screen reads — so the two cannot disagree about the data.
//
// AUTHENTICATED, unlike the public page feed: this answers with the book's data, so it asks for the acting portal user
// and refuses a request without one. Read-only.
// ---------------------------------------------------------------------------

async function GETHandler(req: NextRequest): Promise<Response> {
  const actor = await getPortalActingUser()
  if (!actor) {
    return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })
  }

  const screen = req.nextUrl.searchParams.get('screen') ?? ''
  switch (screen) {
    case 'activity': {
      // The same call the live screen makes, with the same limit, and the read model's own field names — `direction`,
      // `personId`, `dealId` and `occurredAtLabel` are all things the screen renders.
      const entries = await getActivityFeed(50)
      return NextResponse.json({
        activity: entries.map((entry) => ({
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
    default: {
      // NO SILENT EMPTY SCREEN. A screen that asks for a payload nobody serves is a wiring mistake — the Rust side only
      // asks for a DTO when `is_ported_portal_screen` names the screen — so it fails loudly and names the screen.
      return NextResponse.json(
        { error: `no portal payload source for screen '${screen}'` },
        { status: 400 },
      )
    }
  }
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/page', route: '/api/portal/rust-ui/page' },
  GETHandler,
)
