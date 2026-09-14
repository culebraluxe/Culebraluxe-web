import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { getClientById } from "@/db/clients"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — full canonical Client detail for one person (working-pane read).
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
    const client = await getClientById(personId)
    return NextResponse.json({ client })
  } catch {
    return NextResponse.json({ client: null })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]', route: '/api/portal/clients/[personId]' },
  GETHandler,
)
