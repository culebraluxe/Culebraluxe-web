import { NextRequest, NextResponse } from 'next/server'

import { recordRustAppDiagnostic } from '@/lib/rust-api/diagnostics'
import { withApiHandler } from '@/lib/error-capture-seam'

import {
  DIAGNOSTIC_POLICY,
  createDiagnosticThrottle,
  refuseDiagnosticWrite,
} from '../diagnostic-throttle'

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

// One throttle PER ENDPOINT. State is per serverless instance; the bound is a
// bound, not a fleet-wide quota (global limiting is an explicit non-goal). The
// bound itself lives in DIAGNOSTIC_POLICY, shared with the sibling route.
const throttle = createDiagnosticThrottle(DIAGNOSTIC_POLICY)

// Best-effort source token for the in-memory bound only — NEVER persisted. The
// recorded refusal carries the count and the reason, no caller identity.
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
      endpoint: 'api/portal/client-error',
      now,
      status,
      record: (record) => recordRustAppDiagnostic({
        kind: record.kind ?? 'diagnostic-refusal',
        operation: record.operation ?? 'api/portal/client-error',
        message: record.message,
        route: record.route ?? '/api/portal/client-error',
        level: record.level ?? 'warn',
        code: null,
        meta: {},
      }),
    })

  // Refuse an oversized body BEFORE reading it into memory.
  if (declaredBytes > DIAGNOSTIC_POLICY.maxBodyBytes) {
    throttle.checkDiagnosticWrite({ source, bodyBytes: declaredBytes, now })
    return refuse(413)
  }

  const raw = await req.text().catch(() => '')
  const bodyBytes = Math.max(declaredBytes, byteLength(raw))
  const verdict = throttle.checkDiagnosticWrite({ source, bodyBytes, now })
  if (!verdict.allowed) return refuse(verdict.status)

  // Parse defensively: this route exists to receive reports about failures, so a malformed body
  // must be recorded as a report with no message rather than becoming a failure of its own.
  let body: Record<string, unknown> = {}
  try {
    body = JSON.parse(raw) as Record<string, unknown>
  } catch {
    /* a malformed report is recorded with no message below, not refused */
  }

  const message = typeof body.message === 'string' ? body.message.slice(0, MAX_MESSAGE) : null
  const digest = typeof body.digest === 'string' ? body.digest.slice(0, 120) : null
  const path = typeof body.path === 'string' ? body.path.slice(0, 300) : null

  await recordRustAppDiagnostic({
    kind: 'portal-boundary',
    operation: 'portal.boundary',
    route: path ?? '/portal',
    code: digest,
    message: message ?? 'portal route failed with no message',
    level: 'warn',
    meta: { digest, reportedBy: 'app/portal/error.tsx' },
  })

  return NextResponse.json({ ok: true })
}

export const POST = withApiHandler(
  { label: '/api/portal/client-error', route: '/api/portal/client-error' },
  POSTHandler,
)
