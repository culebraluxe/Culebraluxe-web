import { NextRequest, NextResponse } from "next/server"

import type { ClientRelationshipChannel } from "@/lib/portal/types"
import { coreServices } from "@/lib/service-runtime"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — source-grain relationship channels for a selected canonical Person.
//
// The Client History panel reads ONE bounded row per communication source from
// the COMMS service, which owns the warehouse read models and the channel
// vocabulary. It used to read mv_client_relationship_channels through a
// repository here, which is why `apple_calls` and `apple_facetime` reached the
// pane as unknown channels and rendered with a generic globe instead of the Call
// and Video icons. The mapping now happens once, in the service.
//
// Response shape is unchanged, so the panel component is untouched.
// ---------------------------------------------------------------------------

async function GETHandler(
  _req: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  try {
    if (!coreServices.comms) {
      return NextResponse.json({ channels: [] })
    }
    const result = await coreServices.comms.execute({
      operation: "comms.panel",
      payload: { personId, momentLimit: 0 },
      context: { actor: { id: "portal-api", kind: "user" }, correlationId: crypto.randomUUID() },
    })
    if (!result.ok) {
      return NextResponse.json({ channels: [] })
    }
    const channels: ClientRelationshipChannel[] = result.value.sources.map((source) => ({
      personId,
      source: source.source,
      channel: source.channel,
      firstObservedAt: source.firstObservedAt,
      lastContactAt: source.lastContactAt,
      lastInboundAt: source.lastInboundAt,
      lastOutboundAt: source.lastOutboundAt,
      inboundCount: source.inboundCount,
      outboundCount: source.outboundCount,
      totalCount: source.totalCount,
      lastDirection: source.lastDirection,
      twoWay: source.twoWay,
      lastContext: source.lastContext,
      lastContextAt: source.lastContextAt,
      lastContextType: source.lastContextType,
      lastContextDirection: source.lastContextDirection,
    }))
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
