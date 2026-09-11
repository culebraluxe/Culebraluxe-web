import { NextRequest, NextResponse } from "next/server"

import { getRuntimeInspection } from "@/workflow_app/runtime-inspector-read"
import { withApiHandler } from '@/lib/error-capture-seam'

// WORKFLOW RUNTIME INSPECTOR — overlay Flight Recorder trace evidence on the
// design-time topology for one workflow instance.
//
//   ?at=<ISO>   reconstruct the overlay at a past timestamp T (time machine;
//               visual replay only, never re-execution)
export const dynamic = "force-dynamic"

async function GETHandler(
  req: NextRequest,
  { params }: { params: Promise<{ instanceId: string }> },
) {
  const { instanceId } = await params
  const at = req.nextUrl.searchParams.get("at")
  const atIso = at && at !== "now" ? at : null
  try {
    const payload = await getRuntimeInspection(instanceId, atIso)
    if (!payload) {
      return NextResponse.json({ error: "not_found" }, { status: 404 })
    }
    return NextResponse.json(payload)
  } catch (err) {
    console.error("[runtime-inspector] read failed:", err)
    return NextResponse.json(
      { error: "runtime_inspector_unavailable" },
      { status: 503 },
    )
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/runtime-inspector/[instanceId]', route: '/api/portal/runtime-inspector/[instanceId]' },
  GETHandler,
)
