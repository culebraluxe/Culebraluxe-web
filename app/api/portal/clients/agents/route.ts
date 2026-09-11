import { NextResponse } from "next/server"

import { listAssignableAgents } from "@/db/person-admin"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — assignable agents for the New/Edit client forms. A small bounded
// list fetched once by the ClientManager working pane.
// ---------------------------------------------------------------------------

async function GETHandler() {
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
