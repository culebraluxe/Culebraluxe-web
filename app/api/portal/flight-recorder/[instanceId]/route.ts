import { NextRequest, NextResponse } from 'next/server'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { isUuidLike } from '@/lib/deal-admin'
import { withApiHandler } from '@/lib/error-capture-seam'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { rustApiRead, RustApiError } from '@/lib/rust-api/client'

export const dynamic = 'force-dynamic'

// FLIGHT RECORDER — stable Next edge, Rust-owned read model.
//
// The browser/Yew contract stays unchanged: this route still returns the canonical transaction JSON directly.
// Authentication remains tech.access at the portal edge; the Rust service independently resolves the actor and audits
// the workflow read. No legacy Flight Recorder database reader participates in the live path.
async function GETHandler(
  _req: NextRequest,
  { params }: { params: Promise<{ instanceId: string }> },
) {
  const { instanceId } = await params

  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'tech.access')
  if (!access.ok) {
    return NextResponse.json(
      { error: 'unauthorized', detail: 'Reading an engine trace requires TECH access.' },
      { status: 401 },
    )
  }

  if (!isUuidLike(instanceId)) {
    return NextResponse.json(
      {
        error: 'invalid_instance_id',
        detail:
          '"' +
          instanceId.slice(0, 64) +
          '" is not a process instance id (expected a UUID). The link or button that opened this screen carried something else — a story id, a run id, or an empty value — so there is no trace to read.',
      },
      { status: 400 },
    )
  }

  try {
    const answer = await rustApiRead<Record<string, unknown>>(
      ('/v1/flight-recorder/' + encodeURIComponent(instanceId)) as `/v1/${string}`,
    )
    return NextResponse.json(answer.value)
  } catch (error) {
    if (error instanceof RustApiError) {
      if (error.status === 404) {
        return NextResponse.json({ error: 'not_found' }, { status: 404 })
      }
      if (error.status === 400) {
        return NextResponse.json(
          { error: 'invalid_instance_id', detail: error.message },
          { status: 400 },
        )
      }
      if (error.status === 401 || error.status === 403) {
        return NextResponse.json(
          { error: 'unauthorized', detail: error.message },
          { status: error.status },
        )
      }
      return NextResponse.json(
        { error: 'flight_recorder_unavailable', detail: error.message },
        { status: error.status >= 500 ? error.status : 503 },
      )
    }
    throw error
  }
}

export const GET = withApiHandler(
  { label: '/api/portal/flight-recorder/[instanceId]', route: '/api/portal/flight-recorder/[instanceId]' },
  GETHandler,
)
