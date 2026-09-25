import { NextResponse, type NextRequest } from 'next/server'

import { resolveRustApiBaseUrl } from '@/lib/rust-api/contract'
import { withApiHandler } from '@/lib/error-capture-seam'

export const runtime = 'nodejs'

const BOLD_SIGN_EVENT_HEADER = 'x-boldsign-event'
const BOLD_SIGN_SIGNATURE_HEADER = 'x-boldsign-signature'

function rustBaseUrl(): string | null {
  return resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
}

/**
 * Public provider edge only.
 *
 * BoldSign's initial Verification event is intentionally unsigned and is
 * acknowledged here before any body/config work. Every signed lifecycle event
 * is then forwarded byte-for-byte to the Rust signature transport, which owns
 * HMAC verification, dedupe, canonical envelope state and reconciliation.
 */
async function POSTHandler(request: NextRequest): Promise<Response> {
  if (request.headers.get(BOLD_SIGN_EVENT_HEADER) === 'Verification') {
    return new Response(null, { status: 200 })
  }

  const signature = request.headers.get(BOLD_SIGN_SIGNATURE_HEADER)
  if (!signature) {
    return NextResponse.json(
      { ok: false, error: 'Missing BoldSign signature header.' },
      { status: 401 },
    )
  }

  const base = rustBaseUrl()
  if (!base) {
    return NextResponse.json(
      { ok: false, error: 'Webhook service is unavailable.' },
      { status: 503 },
    )
  }

  const rawBody = await request.text()
  const response = await fetch(`${base}/api/integrations/boldsign/webhook`, {
    method: 'POST',
    headers: {
      [BOLD_SIGN_SIGNATURE_HEADER]: signature,
      'content-type':
        request.headers.get('content-type') ?? 'application/json',
    },
    body: rawBody,
    cache: 'no-store',
  })

  const body = await response.arrayBuffer()
  return new Response(body, {
    status: response.status,
    headers: {
      'content-type':
        response.headers.get('content-type') ?? 'application/json',
    },
  })
}

export const POST = withApiHandler(
  {
    label: '/api/integrations/boldsign/webhook',
    route: '/api/integrations/boldsign/webhook',
  },
  POSTHandler,
)
