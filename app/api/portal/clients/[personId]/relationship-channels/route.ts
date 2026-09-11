import { NextRequest, NextResponse } from "next/server"

import { getClientRelationshipChannels } from "@/db/relationship-channels"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — source-grain relationship channels for a selected canonical Person.
// The PRIMARY Client History panel reads ONE bounded row per communication
// source from mv_client_relationship_channels (never per-message / per-burst).
// ---------------------------------------------------------------------------

async function GETHandler(
  _req: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  try {
    const channels = await getClientRelationshipChannels(personId)
    return NextResponse.json({ channels })
  } catch {
    return NextResponse.json({ channels: [] })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]/relationship-channels', route: '/api/portal/clients/[personId]/relationship-channels' },
  GETHandler,
)
