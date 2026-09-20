import { NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { rustApiRead } from '@/lib/rust-api/client'
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — authenticated Rust bridge for assignable agents. A small bounded
// list fetched once by the ClientManager working pane.
// ---------------------------------------------------------------------------

async function GETHandler() {
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
  try {
    const result = await rustApiRead<unknown>('/v1/clients/agents')
    return NextResponse.json(result.value)
  } catch (err) {
    // An empty list reads as "there are no assignable agents" and hides a failed read. The form's
    // loader already falls back on a non-2xx response.
    captureServerError('/api/portal/clients/agents', err, { route: '/api/portal/clients/agents' })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/agents', route: '/api/portal/clients/agents' },
  GETHandler,
)
