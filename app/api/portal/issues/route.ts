import { NextRequest, NextResponse } from "next/server"

import { getIssueQueue } from "@/db/issues"
import type {
  IssueResponsibility,
  IssueState,
} from "@/lib/issue-types"

// ---------------------------------------------------------------------------
// ISSUE QUEUE — bounded server-side paging for the OPPS issue dashboard.
// Returns only the requested page (50 default) with a separate total; the
// queue is never shipped whole to the browser. `scope` filters by the
// deterministic Support/OPPS responsibility rule (default OPERATIONS_EXCEPTION).
// ---------------------------------------------------------------------------

const VALID_SCOPES: IssueResponsibility[] = [
  "OPERATIONS_EXCEPTION",
  "SUPPORT_EXCEPTION",
]
const VALID_STATES: IssueState[] = ["OPEN", "RESOLVED"]

function intParam(value: string | null, fallback: number, min: number, max: number) {
  const parsed = parseInt(value ?? "", 10)
  if (Number.isNaN(parsed)) return fallback
  return Math.max(min, Math.min(max, parsed))
}

export async function GET(req: NextRequest) {
  const params = req.nextUrl.searchParams
  const scope = VALID_SCOPES.includes(params.get("scope") as IssueResponsibility)
    ? (params.get("scope") as IssueResponsibility)
    : "OPERATIONS_EXCEPTION"
  const state = VALID_STATES.includes(params.get("state") as IssueState)
    ? (params.get("state") as IssueState)
    : "OPEN"
  const page = intParam(params.get("page"), 1, 1, Number.MAX_SAFE_INTEGER)
  const pageSize = intParam(params.get("pageSize"), 50, 1, 50)

  try {
    const result = await getIssueQueue({ scope, state, page, pageSize })
    return NextResponse.json(result)
  } catch {
    // Queue seam unavailable -> empty page (safe).
    return NextResponse.json({ rows: [], total: 0, page, pageSize, scope, state })
  }
}
