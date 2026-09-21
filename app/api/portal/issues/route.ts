import { NextRequest, NextResponse } from "next/server"
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

import { getIssueQueue } from "@/legacy/db/issues"
import type {
  IssueResponsibility,
  IssueState,
} from "@/lib/issue-types"
import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'

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
  } catch (err) {
    // An empty page reads as "there are no issues" and hides a failed queue read.
    captureServerError('/api/portal/issues', err, { route: '/api/portal/issues' })
    return NextResponse.json({ error: 'database_unavailable' }, { status: 503 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/issues', route: '/api/portal/issues' },
  GETHandler,
)
