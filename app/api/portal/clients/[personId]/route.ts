import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { rustApiRead } from '@/lib/rust-api/client'
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — authenticated transport bridge for one canonical Client detail.
// The restored ClientManager loads the selected person's detail independently
// so the detail pane never requires loading every Person.
// ---------------------------------------------------------------------------

async function GETHandler(
  _req: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  // Authority matches the screen: portal.read.
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'portal.read')
  if (!access.ok) {
    return NextResponse.json(
      {
        error: 'unauthorized',
        detail: 'This portal data requires portal.read.',
      },
      { status: 401 },
    )
  }
  const { personId } = await params
  try {
    const result = await rustApiRead<unknown>(
      `/v1/clients/${encodeURIComponent(personId)}`,
    )
    return NextResponse.json({ client: result.value })
  } catch (err) {
    // Fail loudly instead of returning an empty client, which reads as "no such client" and hides an
    // outage. The gateway already logged the typed failure server-side.
    captureServerError('/api/portal/clients/[personId]', err, { route: '/api/portal/clients/[personId]' })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]', route: '/api/portal/clients/[personId]' },
  GETHandler,
)
