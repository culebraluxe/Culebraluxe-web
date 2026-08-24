import { NextResponse, type NextRequest } from 'next/server'

import { createCommandDispatcher } from '@/lib/commands'
import { runAgreementExecutionRecovery } from '@/lib/agreements/recovery'

// ---------------------------------------------------------------------------
// CRM-27 (BLOCKER 3) — durable agreement-execution recovery scheduler hook.
//
// Runs one durable recovery pass: discover completed signature evidence lacking
// an agreement_execution marker and re-drive each through the canonical
// `agreement.execution.claim` command (idempotent, atomic marker/receipt/outbox).
// This lets a fully-executed agreement be recognised without waiting for another
// provider webhook, and survives webhook/process failure.
//
// This is the SCHEDULE ENTRY POINT (a Vercel cron / external authenticated
// scheduler triggers it). The schedule itself is a deployment concern, not code:
// the route FAILS CLOSED unless the `AGREEMENT_EXECUTION_RECOVERY_KEY` env var is
// configured, and requires that secret on the `x-recovery-key` header. No second
// queue or event store is created — this reuses the canonical command + outbox
// seam.
// ---------------------------------------------------------------------------

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const RECOVERY_KEY_HEADER = 'x-recovery-key'

export async function POST(request: NextRequest) {
  const expected = process.env.AGREEMENT_EXECUTION_RECOVERY_KEY
  if (!expected) {
    return NextResponse.json(
      { ok: false, error: 'Recovery is not configured (missing AGREEMENT_EXECUTION_RECOVERY_KEY).' },
      { status: 503 },
    )
  }
  const provided = request.headers.get(RECOVERY_KEY_HEADER)
  if (provided !== expected) {
    return NextResponse.json({ ok: false, error: 'Unauthorized.' }, { status: 401 })
  }

  try {
    const summary = await runAgreementExecutionRecovery({
      dispatcher: createCommandDispatcher(),
    })
    return NextResponse.json({ ok: true, summary }, { status: 200 })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return NextResponse.json({ ok: false, error: message }, { status: 500 })
  }
}
