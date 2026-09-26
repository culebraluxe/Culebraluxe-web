import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import type { ClientRelationshipChannel } from "@/lib/portal/types"
import { rustApiRead } from '@/lib/rust-api/client'
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

type RustCommsPanel = {
  sources: Array<Omit<ClientRelationshipChannel, 'personId'> & { label?: string }>
}

// ---------------------------------------------------------------------------
// CLIENTS — source-grain relationship channels for a selected canonical Person.
//
// The Client History panel reads ONE bounded row per communication source from
// the Rust COMMS service, which owns the warehouse read models and the channel
// vocabulary. Response shape is unchanged, so the panel component is untouched.
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
    const result = await rustApiRead<RustCommsPanel>(
      `/v1/comms/${encodeURIComponent(personId)}/panel?momentLimit=0`,
    )
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
