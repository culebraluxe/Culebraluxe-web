import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { getClientContactHistory } from "@/db/contact-history"
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENTS — contact history for a selected canonical Person.
// Server-side paginated (SQL ORDER BY occurred_at DESC + LIMIT/OFFSET), ~20/page.
// ---------------------------------------------------------------------------

async function GETHandler(
  req: NextRequest,
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
  const url = req.nextUrl.searchParams
  const page = Math.max(1, parseInt(url.get("page") ?? "1", 10) || 1)
  const pageSize = Math.max(1, Math.min(50, parseInt(url.get("pageSize") ?? "20", 10) || 20))
  const recent = url.get("recent") === "true"

  try {
    const result = await getClientContactHistory(personId, { page, pageSize, recent })
    return NextResponse.json(result)
  } catch (err) {
    // An empty page reads as "this client has no history" and hides a failed read.
    captureServerError('/api/portal/clients/[personId]/history', err, {
      route: '/api/portal/clients/[personId]/history',
    })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]/history', route: '/api/portal/clients/[personId]/history' },
  GETHandler,
)
