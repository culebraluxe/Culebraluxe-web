import { NextRequest, NextResponse } from "next/server"

import { captureServerError } from '@/lib/server-error-capture'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { getFlightRecorderTransaction, isProcessInstanceId } from "@/workflow_app/flight-recorder-read"
import { withApiHandler } from '@/lib/error-capture-seam'

// FLIGHT RECORDER — the canonical transaction read model backing the Grok view.
// Loads the business transaction, its workflow instance(s), their exact persisted
// definitions, and the real trace evidence with node mapping. Runtime Inspector
// remains a separate engineering surface.
export const dynamic = "force-dynamic"

async function GETHandler(
  _req: NextRequest,
  { params }: { params: Promise<{ instanceId: string }> },
) {
  const { instanceId } = await params

  // THE TRACE REQUIRES THE AUTHORITY THE SCREEN REQUIRES.
  //
  // This route had NO authorization check at all. Measured on 2026-09-14 against production:
  // `curl https://www.culebraluxe.com/api/portal/flight-recorder/<instance>` with no session
  // returned 200 and 71KB of engine trace - role payloads, model turns, commit SHAs, work item
  // ids - for anyone who could guess or learn an instance id. The page in front of it demands
  // `tech.access`; the data behind it demanded nothing. Same authority, checked here now.
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'tech.access')
  if (!access.ok) {
    return NextResponse.json(
      { error: 'unauthorized', detail: 'Reading an engine trace requires TECH access.' },
      { status: 401 },
    )
  }

  // A LINK THAT CARRIES SOMETHING OTHER THAN AN INSTANCE ID IS A 400, NOT A 503.
  //
  // This used to fall through to the read, where Postgres raised `22P02 invalid input syntax for
  // type uuid` and the handler answered "flight_recorder_unavailable" with a 503. The operator saw
  // an unavailable server while the real fault was the id in the link, and nothing in the response
  // (or in the error store, because operator input is control flow, not a server fault) said so.
  if (!isProcessInstanceId(instanceId)) {
    return NextResponse.json(
      {
        error: "invalid_instance_id",
        detail:
          `"${instanceId.slice(0, 64)}" is not a process instance id (expected a UUID). ` +
          "The link or button that opened this screen carried something else — a story id, a run " +
          "id, or an empty value — so there is no trace to read.",
      },
      { status: 400 },
    )
  }

  try {
    const tx = await getFlightRecorderTransaction(instanceId)
    if (!tx) {
      return NextResponse.json({ error: "not_found" }, { status: 404 })
    }
    return NextResponse.json(tx)
  } catch (err) {
    captureServerError('/api/portal/flight-recorder/[instanceId]', err, { route: '/api/portal/flight-recorder/[instanceId]' })
    console.error("[flight-recorder] read failed:", err)
    return NextResponse.json(
      { error: "flight_recorder_unavailable" },
      { status: 503 },
    )
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/flight-recorder/[instanceId]', route: '/api/portal/flight-recorder/[instanceId]' },
  GETHandler,
)
