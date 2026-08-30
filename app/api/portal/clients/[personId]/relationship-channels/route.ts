import { NextRequest, NextResponse } from "next/server"

import { getClientRelationshipChannels } from "@/db/relationship-channels"

// ---------------------------------------------------------------------------
// CLIENTS — source-grain relationship channels for a selected canonical Person.
// The PRIMARY Client History panel reads ONE bounded row per communication
// source from mv_client_relationship_channels (never per-message / per-burst).
// ---------------------------------------------------------------------------

export async function GET(
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
