import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import type { ClientRelationshipChannel } from "@/lib/portal/types"
import { coreServices } from "@/lib/service-runtime"
import { captureServerError } from '@/lib/server-error-capture'
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
  } catch (err) {
    // An empty channel list reads as "this client has no channels" and hides a failed read.
    captureServerError('/api/portal/clients/[personId]/relationship-channels', err, {
      route: '/api/portal/clients/[personId]/relationship-channels',
    })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]/relationship-channels', route: '/api/portal/clients/[personId]/relationship-channels' },
  GETHandler,
)
