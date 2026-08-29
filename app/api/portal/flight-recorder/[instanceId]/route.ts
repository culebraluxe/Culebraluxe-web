import { NextRequest, NextResponse } from "next/server"

import { getFlightRecorderTransaction } from "@/workflow_app/flight-recorder-read"

// FLIGHT RECORDER — the canonical transaction read model backing the Grok view.
// Loads the business transaction, its workflow instance(s), their exact persisted
// definitions, and the real trace evidence with node mapping. Runtime Inspector
// remains a separate engineering surface.
export const dynamic = "force-dynamic"

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ instanceId: string }> },
) {
  const { instanceId } = await params
  try {
    const tx = await getFlightRecorderTransaction(instanceId)
    if (!tx) {
      return NextResponse.json({ error: "not_found" }, { status: 404 })
    }
    return NextResponse.json(tx)
  } catch (err) {
    console.error("[flight-recorder] read failed:", err)
    return NextResponse.json(
      { error: "flight_recorder_unavailable" },
      { status: 503 },
    )
  }
}
