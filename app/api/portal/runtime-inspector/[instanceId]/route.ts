import { NextRequest, NextResponse } from "next/server"

import { getRuntimeInspection } from "@/workflow_app/runtime-inspector-read"

// WORKFLOW RUNTIME INSPECTOR — overlay Flight Recorder trace evidence on the
// design-time topology for one workflow instance.
//
//   ?at=<ISO>   reconstruct the overlay at a past timestamp T (time machine;
//               visual replay only, never re-execution)
export const dynamic = "force-dynamic"

export async function GET(
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
