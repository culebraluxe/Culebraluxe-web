import { NextRequest, NextResponse } from 'next/server'

import { getActivityFeed } from '@/db/activity-feed'
import { AuthError } from '@/lib/auth/errors'
import { getPortalActingUser } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// ROWS FOR THE RUST UI.
//
// The browser never calls the Rust API and the WASM module never makes a request: it renders a model and asks for
// data through a DOM event, the TypeScript host fetches from *this* route, and the answer goes back into the model
// through `rows_loaded`. That is why this route exists in the application rather than in Rust — the session, the
// credentials and the permission checks are all here, where they already were.
//
// Shape (`Row` in rust/ui/src/model.rs, camelCase on both sides):
//   { id: string, cells: string[], badge?: string | null }
//
// The `cells` array is deliberately generic. This pass ports screen *structure* — route, heading, navigation, state
// boundary — so only the screens whose real columns have actually been read get real rows here. Everything else
// answers `[]` and the screen says "Nothing to show yet", which is honest: an invented column is a lie the next
// reader has to disprove.
//
// AUTHORITY: authenticated portal users only. A per-screen authority check (who may read expenses, who may read
// flight recorder) has to be decided per screen and is NOT yet applied here — so this route stays read-only, and any
// screen with a narrower audience must not be added to the list below until its authority is chosen.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

type RustUiRow = { id: string; cells: string[]; badge?: string | null }

function activityRows(
  entries: Awaited<ReturnType<typeof getActivityFeed>>,
): RustUiRow[] {
  return entries.map((entry) => ({
    id: entry.id,
    cells: [
      entry.title ?? '(no title)',
      entry.occurredAtLabel,
      entry.personName ?? entry.propertyName ?? entry.dealPropertyName ?? '—',
    ],
    badge: entry.channel,
  }))
}

async function GETHandler(req: NextRequest): Promise<Response> {
  try {
    await getPortalActingUser()
  } catch (error) {
    // Fail closed: a route that answers rows to anonymous callers is a data leak with extra steps.
    if (error instanceof AuthError) {
      return NextResponse.json({ error: 'Unauthorized.' }, { status: 401 })
    }
    throw error
  }

  const screen = req.nextUrl.searchParams.get('screen') ?? ''

  switch (screen) {
    case 'activity':
      return NextResponse.json(activityRows(await getActivityFeed(50)))
    default:
      return NextResponse.json([])
  }
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/rows', route: '/api/portal/rust-ui/rows' },
  GETHandler,
)
