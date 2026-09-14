import { NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { listAssignableAgents } from "@/db/person-admin"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — assignable agents for the New/Edit client forms. A small bounded
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
    const agents = await listAssignableAgents()
    return NextResponse.json(agents)
  } catch {
    return NextResponse.json([])
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/agents', route: '/api/portal/clients/agents' },
  GETHandler,
)
