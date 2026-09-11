import { after, NextResponse, type NextRequest } from 'next/server'

import { captureServerError } from '@/lib/server-error-capture'
import { landWhatsapp } from '@/db/landing'
import { sql } from '@/db/client'
import { refreshClientReadModels } from '@/db/client-read-models'
import {
  loadMetaWhatsAppConfiguration,
  loadWhatsAppVerifyToken,
} from '@/lib/whatsapp-cloud/config'
import { processMetaWhatsAppWebhook } from '@/lib/whatsapp-cloud/application'
import type { MetaWhatsAppWebhookPayload, MetaWhatsAppMessage } from '@/lib/whatsapp-cloud/types'
import {
  verifyMetaWhatsAppHandshake,
  verifyMetaWhatsAppSignature,
} from '@/lib/whatsapp-cloud/verify'
import { withApiHandler } from '@/lib/error-capture-seam'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown WhatsApp webhook error.'
}

function isFixturePayload(payload: MetaWhatsAppWebhookPayload): boolean {
  return payload.entry?.some((entry) =>
    entry.changes?.some((change) =>
      change.value?.messages?.some((message) =>
        message.id?.startsWith('wamid.CULEBRALUXE_FIXTURE.'),
      ),
    ),
  ) ?? false
}

async function GETHandler(request: NextRequest) {
  let expectedToken: string
  try {
    expectedToken = loadWhatsAppVerifyToken()
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/webhook', error, { route: '/api/integrations/whatsapp/webhook' })
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

async function POSTHandler(request: NextRequest) {
  const rawBody = await request.text()

  let config: ReturnType<typeof loadMetaWhatsAppConfiguration>
  try {
    config = loadMetaWhatsAppConfiguration()
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/webhook', error, { route: '/api/integrations/whatsapp/webhook' })
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
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/webhook', error, { route: '/api/integrations/whatsapp/webhook' })
    return NextResponse.json(
      { ok: false, error: 'Invalid WhatsApp payload.' },
      { status: 400 },
    )
  }

  try {
    // GOLDEN RULE: all input lands in its own L table first — the RAW Meta
    // payload, before any normalization. Replay-safe on (source_account, wamid),
    // so a Meta redelivery lands nothing new. Landing failures must NOT stop
    // processing: the inbound message still has to reach the CRM.
    try {
      for (const entry of payload.entry ?? []) {
        for (const change of entry.changes ?? []) {
          const value = change.value
          const sourceAccount = value?.metadata?.phone_number_id ?? config.phoneNumberId ?? null
          const batches: Array<['incoming' | 'outgoing', MetaWhatsAppMessage[] | undefined]> = [
            ['incoming', value?.messages],
            ['outgoing', value?.message_echoes],
          ]
          for (const [direction, list] of batches) {
            for (const message of list ?? []) {
              if (!message.id) continue
              const mediaId =
                message.image?.id ??
                message.video?.id ??
                message.audio?.id ??
                message.document?.id ??
                message.sticker?.id ??
                null
              await landWhatsapp(
                {
                  sourceAccount,
                  sourceMessageId: message.id,
                  conversationId: message.context?.id ?? null,
                  fromAddress: message.from ?? null,
                  toAddress: message.to ?? null,
                  direction,
                  messageType: message.type ?? null,
                  text: message.text?.body ?? null,
                  mediaId,
                  sentAt: message.timestamp
                    ? new Date(Number(message.timestamp) * 1000).toISOString()
                    : null,
                  raw: message,
                },
                sql,
              )
            }
          }
        }
      }
    } catch (error) {
      captureServerError('/api/integrations/whatsapp/webhook', error, {
        route: '/api/integrations/whatsapp/webhook',
      })
    }

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

    const fixture = isFixturePayload(payload)
    return NextResponse.json(
      {
        ok: true,
        accepted: result.eventCount,
        relationshipProjected: result.relationshipProjected,
        ...(fixture
          ? {
              fixtureOutcomes: result.outcomes.map((outcome) => ({
                outcome: outcome.outcome,
                ...('reason' in outcome ? { reason: outcome.reason } : {}),
                ...('resolvedPersonId' in outcome && outcome.resolvedPersonId
                  ? { resolvedPersonId: outcome.resolvedPersonId }
                  : {}),
                ...('interactionId' in outcome && outcome.interactionId
                  ? { interactionId: outcome.interactionId }
                  : {}),
              })),
            }
          : {}),
      },
      { status: 200 },
    )
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/webhook', error, { route: '/api/integrations/whatsapp/webhook' })
    console.error('[whatsapp-webhook] event processing failed', {
      error: errorMessage(error),
    })
    return NextResponse.json(
      { ok: false, error: 'Webhook processing failed.' },
      { status: 500 },
    )
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/integrations/whatsapp/webhook', route: '/api/integrations/whatsapp/webhook' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/integrations/whatsapp/webhook', route: '/api/integrations/whatsapp/webhook' },
  POSTHandler,
)
