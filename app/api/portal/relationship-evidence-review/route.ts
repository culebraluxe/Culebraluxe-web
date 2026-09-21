import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { getRelationshipEvidenceReview } from "@/legacy/db/relationship-evidence"
import type { ReviewState } from "@/lib/relationship-intel/contracts"
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// REL-INTEL — OPPS relationship-evidence review (occasional data stewardship).
// Filters by reconciliation outcome (exact / probable / ambiguous / unmatched /
// rejected / non_person / deferred) with optional search + pagination.
// Server-side bounded; never a Lisa-facing daily surface.
// ---------------------------------------------------------------------------

const VALID_REVIEW_STATES: ReviewState[] = [
  "unresolved",
  "exact_linked",
  "review_required",
  "ambiguous",
  "unmatched",
  "rejected",
  "non_person",
  "deferred",
]

async function GETHandler(req: NextRequest) {
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
  const params = req.nextUrl.searchParams
  const rawState = params.get("reviewState")
  const reviewState: ReviewState | "all" =
    rawState && VALID_REVIEW_STATES.includes(rawState as ReviewState)
      ? (rawState as ReviewState)
      : "all"
  const search = params.get("search") ?? ""
  const limit = Math.max(1, Math.min(100, parseInt(params.get("limit") ?? "50", 10) || 50))
  const offset = Math.max(0, parseInt(params.get("offset") ?? "0", 10) || 0)

  try {
    const result = await getRelationshipEvidenceReview({ reviewState, search, limit, offset })
    return NextResponse.json(result)
  } catch (err) {
    // An empty review reads as "nothing to review" and hides a failed read.
    captureServerError('/api/portal/relationship-evidence-review', err, {
      route: '/api/portal/relationship-evidence-review',
    })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/relationship-evidence-review', route: '/api/portal/relationship-evidence-review' },
  GETHandler,
)
