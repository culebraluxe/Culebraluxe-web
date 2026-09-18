import { NextRequest, NextResponse } from 'next/server'

import { recordError } from '@/db/app-error'
import { sql as errorSql } from '@/db/client'
import { withApiHandler } from '@/lib/error-capture-seam'

import { createDiagnosticThrottle, refuseDiagnosticWrite } from '../diagnostic-throttle'

// ---------------------------------------------------------------------------
// MOVE TRACE — what the BOARD sees, reported from the browser.
//
// This exists because of a failure mode that cost the captain six rounds: the drag moved the card on
// screen and NOTHING reached the database, and every server-side check (the row, the error table, the
// deploy verification) was silent about it. The only place that knows what the widget's store actually
// did is the browser, so the board reports what it observes: how many cards it can see, and every
// column change it detects.
//
// It is anonymous and deliberately tiny - a label, a card id, two column names - recorded at `info`
// through the standard capture seam (observed facts, not control flow), so `/portal/tech/app-errors`
// and the `app_error` table can answer "did the drag reach the app at all?" without asking anyone to
// open a console. It stores no comment, no user, no free text.
// ---------------------------------------------------------------------------

export const dynamic = 'force-dynamic'

const MAX_FIELD = 64
const MAX_BODY_BYTES = 16_384

// One throttle PER ENDPOINT: a flood of move-traces must not spend the budget
// the client-error endpoint needs.
const throttle = createDiagnosticThrottle({
  windowMs: 60_000,
  maxWrites: 30,
  maxBodyBytes: MAX_BODY_BYTES,
})

function clip(value: unknown): string {
  return String(value ?? '').slice(0, MAX_FIELD)
}

// Best-effort source token for the in-memory bound only. It is NEVER persisted:
// the recorded refusal carries the count and the reason, nothing that identifies
// the caller. A spoofable header is acceptable because this is a bound, not auth.
function sourceToken(req: NextRequest): string {
  const forwarded = req.headers.get('x-forwarded-for')
  if (forwarded) {
    const first = forwarded.split(',')[0]?.trim()
    if (first) return first
  }
  return req.headers.get('x-real-ip')?.trim() || 'anonymous'
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const now = Date.now()
  const source = sourceToken(req)
  const declared = Number(req.headers.get('content-length'))
  const declaredBytes = Number.isFinite(declared) && declared > 0 ? declared : 0

  const refuse = (status: 429 | 413) =>
    refuseDiagnosticWrite({
      throttle,
      endpoint: 'api/portal/move-trace',
      now,
      status,
      record: (record) => recordError(record, errorSql),
    })

  // Refuse an oversized body BEFORE reading it into memory.
  if (declaredBytes > MAX_BODY_BYTES) {
    throttle.checkDiagnosticWrite({ source, bodyBytes: declaredBytes, now })
    return refuse(413)
  }

  const raw = await req.text().catch(() => '')
  const bodyBytes = Math.max(declaredBytes, byteLength(raw))
  const verdict = throttle.checkDiagnosticWrite({ source, bodyBytes, now })
  if (!verdict.allowed) return refuse(verdict.status)

  let body: Record<string, unknown> = {}
  try {
    body = JSON.parse(raw) as Record<string, unknown>
  } catch {
    /* a malformed trace is not worth an error row; this route is diagnostics */
  }

  const phase = clip(body.phase) || 'unknown'
  const cardId = clip(body.cardId)
  const from = clip(body.from)
  const to = clip(body.to)
  const detail = clip(body.detail)

  // RECORDED WITH AN EXPLICIT EXECUTOR. `captureServerLog` depends on the registered seam, and in this
  // route's context that was not registered - so the endpoint answered 200 and wrote NOTHING, which is
  // exactly the silence this whole trace was built to end (measured: a probe POST returned 200 and left
  // no row). An explicit executor cannot be unregistered.
  await recordError(
    {
      kind: 'INFO',
      operation: 'cockpit.move-trace',
      message:
        `${phase}` +
        (cardId ? ` card=${cardId}` : '') +
        (from || to ? ` ${from} -> ${to}` : '') +
        (detail ? ` (${detail})` : ''),
      route: '/portal/tech',
      level: 'info',
    },
    errorSql,
  ).catch(() => {
    // A failure to trace must never fail the board.
  })

  return NextResponse.json({ ok: true })
}

export const POST = withApiHandler(
  { label: 'api/portal/move-trace', route: '/api/portal/move-trace' },
  POSTHandler,
)
