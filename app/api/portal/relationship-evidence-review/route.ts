import { NextRequest, NextResponse } from "next/server"
import { getRelationshipEvidenceReview } from "@/db/relationship-evidence"
import type { ReviewState } from "@/lib/relationship-intel/contracts"

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

export async function GET(req: NextRequest) {
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
  } catch {
    // Evidence seam not yet migrated/populated -> empty review (safe).
    return NextResponse.json({ rows: [], total: 0 })
  }
}
