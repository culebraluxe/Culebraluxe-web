import { NextRequest, NextResponse } from "next/server"

import { getClientContactHistory } from "@/db/contact-history"
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — contact history for a selected canonical Person.
// Server-side paginated (SQL ORDER BY occurred_at DESC + LIMIT/OFFSET), ~20/page.
// ---------------------------------------------------------------------------

async function GETHandler(
  req: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  const url = req.nextUrl.searchParams
  const page = Math.max(1, parseInt(url.get("page") ?? "1", 10) || 1)
  const pageSize = Math.max(1, Math.min(50, parseInt(url.get("pageSize") ?? "20", 10) || 20))
  const recent = url.get("recent") === "true"

  try {
    const result = await getClientContactHistory(personId, { page, pageSize, recent })
    return NextResponse.json(result)
  } catch {
    return NextResponse.json({ rows: [], total: 0, page, pageSize, recent })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]/history', route: '/api/portal/clients/[personId]/history' },
  GETHandler,
)
