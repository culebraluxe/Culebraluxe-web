import { NextResponse } from "next/server"

import { listAssignableAgents } from "@/db/person-admin"

// ---------------------------------------------------------------------------
// CLIENTS — assignable agents for the New/Edit client forms. A small bounded
// list fetched once by the ClientManager working pane.
// ---------------------------------------------------------------------------

export async function GET() {
  try {
    const agents = await listAssignableAgents()
    return NextResponse.json(agents)
  } catch {
    return NextResponse.json([])
  }
}
