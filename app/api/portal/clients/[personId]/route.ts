import { NextRequest, NextResponse } from "next/server"

import { getClientById } from "@/db/clients"

// ---------------------------------------------------------------------------
// CLIENTS — full canonical Client detail for one person (working-pane read).
// The restored ClientManager loads the selected person's detail independently
// so the detail pane never requires loading every Person.
// ---------------------------------------------------------------------------

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  try {
    const client = await getClientById(personId)
    return NextResponse.json({ client })
  } catch {
    return NextResponse.json({ client: null })
  }
}
