import { NextResponse, type NextRequest } from 'next/server'

import { CommandDispatcherImpl } from '@/lib/commands/dispatcher'
import { createCommandRegistry } from '@/lib/commands/register'
import { PostgresCommandReceiptRepository } from '@/db/command-receipt-repository'
import { neonTx } from '@/db/tx'
import { SignatureApplication } from '@/lib/signature/application'
import {
  BOLD_SIGN_SIGNATURE_HEADER,
  BoldSignSignatureProvider,
  verifyBoldSignWebhookSignature,
} from '@/lib/signature/boldsign'
import {
  loadBoldSignConfig,
  type BoldSignConfig,
} from '@/lib/signature/boldsign/config'
import { SignatureReconciliationHandler } from '@/lib/signature/reconciliation'

// ---------------------------------------------------------------------------
// DOC-04/05 — BoldSign webhook production endpoint.
//
// Thin integration route (receive -> read RAW body -> authenticate -> parse ->
// hand off -> respond). It does NOT implement any signing business logic: after
// authenticity is established it hands the raw body to the existing DOC-03
// application router (lib/signature/application.ts), which verifies + dedupes +
// normalizes via the existing BoldSign adapter (lib/signature/boldsign) and then
// dispatches the neutral status command and DOC-05 reconciliation.
//
// All BoldSign vocabulary stays at the provider boundary — nothing provider-
// specific leaks into canonical document / form / deal / workflow models.
// ---------------------------------------------------------------------------

export const runtime = 'nodejs'

const NON_ACTIONABLE_ERROR_MARKERS = [
  'unknown envelope',
  'no neutral lifecycle mapping',
  'not valid JSON',
  'missing event.id',
  'missing event.eventType',
  'missing data.documentId',
]

function isNonActionable(message: string): boolean {
  return NON_ACTIONABLE_ERROR_MARKERS.some((marker) => message.includes(marker))
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown webhook processing error.'
}

/**
 * Detect BoldSign's webhook Verify handshake. The FIRST operation in POST: the
 * `x-boldsign-event` header is `Verification` for the initial handshake, which
 * is UNSIGNED and requires NO CulebraLuxe BoldSign environment variables. It is
 * acknowledged (200) before any body parsing, signature validation, config
 * loading, DB access, or application construction — so Verify passes even when
 * every BOLDSIGN_* env key is absent. Verification is never a lifecycle event.
 */
const BOLD_SIGN_EVENT_HEADER = 'x-boldsign-event'

// Module-scoped lazy singletons: config + the composed application are built once
// per serverless instance. `loadBoldSignConfig` FAILS CLOSED (throws) when any
// required BoldSign env key is missing; the thrown error names only the keys,
// never values, and is never echoed to a public caller.
let config: BoldSignConfig | null = null
let application: SignatureApplication | null = null

function getConfig(): BoldSignConfig {
  config ??= loadBoldSignConfig(process.env)
  return config
}

function getApplication(): SignatureApplication {
  if (!application) {
    const provider = new BoldSignSignatureProvider({ config: getConfig() })
    const dispatcher = new CommandDispatcherImpl({
      registry: createCommandRegistry(),
      receipts: new PostgresCommandReceiptRepository(),
      run: neonTx,
    })
    const reconciler = new SignatureReconciliationHandler({
      provider,
      run: neonTx,
    })
    application = new SignatureApplication({ dispatcher, provider, reconciler })
  }
  return application
}

export async function POST(request: NextRequest) {
  // FIRST operation — BoldSign Verify handshake. Check the `x-boldsign-event`
  // header BEFORE any body parsing, signature validation, config loading, DB
  // access, or application construction. The handshake is unsigned and must
  // succeed even when every BOLDSIGN_* env var is absent.
  if (request.headers.get(BOLD_SIGN_EVENT_HEADER) === 'Verification') {
    return new Response(null, { status: 200 })
  }

  // Read the RAW body before any JSON parsing: BoldSign signs the exact bytes
  // (`<timestamp>.<rawBody>`), so verification MUST operate on the verbatim body.
  const rawBody = await request.text()

  const signatureHeader = request.headers.get(BOLD_SIGN_SIGNATURE_HEADER)
  if (!signatureHeader) {
    // Unverified request — reject, never process.
    return NextResponse.json(
      { ok: false, error: 'Missing BoldSign signature header.' },
      { status: 401 },
    )
  }

  let cfg: BoldSignConfig
  try {
    cfg = getConfig()
  } catch (error) {
    // Fail closed on missing/invalid configuration; log the key NAMES (never
    // values) server-side and return a generic response to the public caller.
    console.error('[boldsign-webhook] configuration error', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook is not configured.' },
      { status: 500 },
    )
  }

  // Authenticate: verify the HMAC-SHA256 signature over the RAW body against the
  // webhook signing secret (constant-time compare + timestamp tolerance). This is
  // the existing DOC-04 verification function. Any failure rejects the request.
  try {
    verifyBoldSignWebhookSignature(
      rawBody,
      signatureHeader,
      cfg.webhookSecret,
      Math.floor(Date.now() / 1000),
      cfg.webhookToleranceSeconds,
    )
  } catch (error) {
    console.warn('[boldsign-webhook] signature verification failed', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Invalid BoldSign signature.' },
      { status: 401 },
    )
  }

  let app: SignatureApplication
  try {
    app = getApplication()
  } catch (error) {
    console.error('[boldsign-webhook] application configuration error', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook is not configured.' },
      { status: 500 },
    )
  }

  // Hand off to the existing application router: it re-verifies (harmless), dedupes
  // by provider event id, normalizes to a neutral event, dispatches the canonical
  // status command, and runs DOC-05 reconciliation post-commit for completed events.
  try {
    await app.handleWebhook(rawBody, signatureHeader, {})
    return NextResponse.json({ ok: true }, { status: 200 })
  } catch (error) {
    const message = errorMessage(error)
    if (isNonActionable(message)) {
      // The signature verified (authentic BoldSign) but this is not a processable
      // signing event for a known envelope — e.g. BoldSign's webhook Verify/test
      // probe, a benign notification with no neutral lifecycle meaning, or an
      // envelope this application did not create. Acknowledge (200) so Verify
      // passes and benign deliveries are not retried; nothing was processed.
      // Reconciliation via status polling (lib/signature/application.ts
      // refreshStatus) remains the convergence backstop for genuine events.
      console.warn('[boldsign-webhook] non-actionable event acknowledged', {
        error: message,
      })
      return NextResponse.json({ ok: true, acknowledged: true }, { status: 200 })
    }
    // Infrastructure / reconciliation failure: surface a retryable error so BoldSign
    // redelivers the event (all downstream paths are idempotent).
    console.error('[boldsign-webhook] event processing failed', {
      error: message,
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook processing failed.' },
      { status: 500 },
    )
  }
}
