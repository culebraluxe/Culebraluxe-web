import { after, NextResponse, type NextRequest } from 'next/server'

import { refreshClientReadModels } from '@/db/client-read-models'
import {
  loadMetaWhatsAppConfiguration,
  loadWhatsAppVerifyToken,
} from '@/lib/whatsapp-cloud/config'
import { processMetaWhatsAppWebhook } from '@/lib/whatsapp-cloud/application'
import type { MetaWhatsAppWebhookPayload } from '@/lib/whatsapp-cloud/types'
import {
  verifyMetaWhatsAppHandshake,
  verifyMetaWhatsAppSignature,
} from '@/lib/whatsapp-cloud/verify'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown WhatsApp webhook error.'
}

export async function GET(request: NextRequest) {
  let expectedToken: string
  try {
    expectedToken = loadWhatsAppVerifyToken()
  } catch (error) {
    console.error('[whatsapp-webhook] verification is not configured', {
      error: errorMessage(error),
    })
    return new Response('not configured', { status: 500 })
  }

  const challenge = verifyMetaWhatsAppHandshake({
    mode: request.nextUrl.searchParams.get('hub.mode'),
    token: request.nextUrl.searchParams.get('hub.verify_token'),
    challenge: request.nextUrl.searchParams.get('hub.challenge'),
    expectedToken,
  })
  return challenge === null
    ? new Response('forbidden', { status: 403 })
    : new Response(challenge, {
        status: 200,
        headers: { 'content-type': 'text/plain; charset=utf-8' },
      })
}

export async function POST(request: NextRequest) {
  const rawBody = await request.text()

  let config: ReturnType<typeof loadMetaWhatsAppConfiguration>
  try {
    config = loadMetaWhatsAppConfiguration()
  } catch (error) {
    console.error('[whatsapp-webhook] configuration error', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook is not configured.' },
      { status: 500 },
    )
  }

  if (!verifyMetaWhatsAppSignature(
    rawBody,
    request.headers.get('x-hub-signature-256'),
    config.appSecret,
  )) {
    return NextResponse.json(
      { ok: false, error: 'Invalid WhatsApp signature.' },
      { status: 401 },
    )
  }

  let payload: MetaWhatsAppWebhookPayload
  try {
    payload = JSON.parse(rawBody) as MetaWhatsAppWebhookPayload
  } catch {
    return NextResponse.json(
      { ok: false, error: 'Invalid WhatsApp payload.' },
      { status: 400 },
    )
  }

  try {
    const result = await processMetaWhatsAppWebhook({ payload, config })
    if (result.retryableFailure) {
      return NextResponse.json(
        { ok: false, error: 'Webhook processing is temporarily unavailable.' },
        { status: 503 },
      )
    }

    // Meta should receive its acknowledgement promptly. The durable inbox,
    // canonical interaction and relationship evidence are committed before the
    // response; materialized Client read models refresh after the response.
    if (result.relationshipProjected > 0) {
      after(async () => {
        try {
          await refreshClientReadModels()
          console.info('[whatsapp-webhook] Client read models refreshed', {
            relationshipProjected: result.relationshipProjected,
          })
        } catch (error) {
          // Do not ask Meta to replay a webhook whose durable business writes
          // already succeeded. A later event/sync can safely refresh again.
          console.error('[whatsapp-webhook] Client read model refresh failed', {
            error: errorMessage(error),
          })
        }
      })
    }

    return NextResponse.json(
      {
        ok: true,
        accepted: result.eventCount,
        relationshipProjected: result.relationshipProjected,
      },
      { status: 200 },
    )
  } catch (error) {
    console.error('[whatsapp-webhook] event processing failed', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook processing failed.' },
      { status: 500 },
    )
  }
}
