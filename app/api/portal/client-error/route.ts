import { NextRequest, NextResponse } from 'next/server'

import { sql } from '@/db/client'
import { recordError } from '@/db/app-error'
import { withApiHandler } from '@/lib/error-capture-seam'

// ---------------------------------------------------------------------------
// CLIENT FAILURES IN /portal/* — the one seam that used to be silent.
//
// `app/portal/error.tsx` is the only error boundary for the whole portal, and it rendered a calm
// "Portal temporarily unavailable" screen while capturing NOTHING. When an operator reported that
// screen there was no row in `app_error`, no route, no digest, no message — nothing to diagnose
// from, and the first question ("was it even the app, or the platform?") could not be answered.
// An error boundary that swallows its error is the same defect as any other unnamed refusal.
//
// The boundary posts here because it cannot reach the database itself (it must stay
// auth-free and presentation-only, and the failure it is reporting may BE an auth or database
// failure — an authenticated capture path would fail exactly when it is needed). So this route is
// deliberately anonymous, and therefore keeps what it will store small and boring: a truncated
// message and the path, at `warn`, with no stack and no client-supplied detail beyond that. The
// worst an abuser can do is add noise to the error log, which is worth it for a portal whose
// operator-reported failures were previously invisible.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

const MAX_MESSAGE = 500

async function POSTHandler(req: NextRequest): Promise<NextResponse> {
  // Parse defensively: this route exists to receive reports about failures, so a malformed body
  // must be recorded as a report with no message rather than becoming a failure of its own.
  const body = (await req
    .json()
    .catch(() => ({}))) as Record<string, unknown>

  const message = typeof body.message === 'string' ? body.message.slice(0, MAX_MESSAGE) : null
  const digest = typeof body.digest === 'string' ? body.digest.slice(0, 120) : null
  const path = typeof body.path === 'string' ? body.path.slice(0, 300) : null

  // The executor is passed EXPLICITLY. `app_error` keeps its writer injected to avoid an import
  // cycle, and `recordError` throws when nothing registered one — but `captureError` swallows
  // throws by design (it must never break the operation it observes), so a capture call in a
  // bundle whose module graph lacks `db/client` records NOTHING, silently. Measured while building
  // this route: the same capture wrote zero rows. Passing `sql` removes the dependency on load
  // order entirely, and the catch below keeps the best-effort contract for the reporter.
  await recordError(
    {
      kind: 'portal-boundary',
      route: path ?? '/portal',
      code: digest,
      message: message ?? 'portal route failed with no message',
      level: 'warn',
      meta: { digest, reportedBy: 'app/portal/error.tsx' },
    },
    sql,
  ).catch(() => {
    /* reporting is best-effort: a failure to record must not become a failure of the boundary */
  })

  return NextResponse.json({ ok: true })
}

export const POST = withApiHandler(
  { label: '/api/portal/client-error', route: '/api/portal/client-error' },
  POSTHandler,
)
