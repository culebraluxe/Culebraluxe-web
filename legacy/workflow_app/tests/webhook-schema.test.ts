import assert from 'node:assert/strict'
import { createHmac } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { NextRequest } from 'next/server'

import { POST } from '@/app/api/integrations/whatsapp/webhook/route'
import {
  metaWhatsAppWebhookPayloadSchema,
  validateMetaWhatsAppWebhookPayload,
} from '@/lib/whatsapp-cloud/schema'
import type { MetaWhatsAppWebhookPayload } from '@/lib/whatsapp-cloud/types'

const APP_SECRET = 'boundary-schema-test-secret'
const PHONE_NUMBER_ID = '1234567890'
const OWNED_PHONE = '+17875550000'

process.env.WHATSAPP_APP_SECRET = APP_SECRET
process.env.WHATSAPP_PHONE_NUMBER_ID = PHONE_NUMBER_ID
process.env.WHATSAPP_OWNED_PHONE_E164 = OWNED_PHONE

function signedRequest(rawBody: string, signature?: string): NextRequest {
  const value =
    signature ??
    `sha256=${createHmac('sha256', APP_SECRET).update(rawBody).digest('hex')}`
  return new NextRequest(
    'http://localhost/api/integrations/whatsapp/webhook',
    {
      method: 'POST',
      body: rawBody,
      headers: {
        'content-type': 'application/json',
        'x-hub-signature-256': value,
      },
    },
  )
}

const VALID_PAYLOAD = {
  object: 'whatsapp_business_account',
  entry: [
    {
      id: 'waba-1',
      changes: [
        {
          field: 'messages',
          value: {
            messaging_product: 'whatsapp',
            metadata: {
              display_phone_number: OWNED_PHONE,
              phone_number_id: PHONE_NUMBER_ID,
            },
            messages: [
              {
                from: '17875551212',
                id: 'wamid.BOUNDARY_TEST_001',
                timestamp: '1787980800',
                type: 'text',
                text: { body: 'hello' },
              },
            ],
          },
        },
      ],
    },
  ],
}

const MALFORMED_BODY = JSON.stringify({
  object: 'whatsapp_business_account',
  entry: 'not-an-array',
})

test('malformed payload returns 400 without touching the domain', async () => {
  const response = await POST(signedRequest(MALFORMED_BODY))
  assert.equal(response.status, 400)
  const body = (await response.json()) as { ok: boolean; error: string }
  assert.equal(body.ok, false)
  assert.equal(body.error, 'Invalid WhatsApp payload.')
})

test('signature verification runs before payload handling', async () => {
  const response = await POST(
    signedRequest(MALFORMED_BODY, `sha256=${'a'.repeat(64)}`),
  )
  assert.equal(response.status, 401)
})

test('payload type is inferred from the schema', () => {
  const typed: MetaWhatsAppWebhookPayload =
    metaWhatsAppWebhookPayloadSchema.parse(VALID_PAYLOAD)
  assert.equal(typed.object, 'whatsapp_business_account')
  assert.equal(typed.entry?.[0]?.changes?.[0]?.value?.messages?.[0]?.text?.body, 'hello')
})

test('fence drives a malformed payload', () => {
  const malformed: unknown[] = [
    null,
    'not-an-object',
    42,
    { object: 'whatsapp_business_account', entry: 'not-an-array' },
    { entry: [{ changes: [{ value: { messages: 'not-an-array' } }] }] },
    { entry: [{ changes: [{ value: { metadata: 'not-an-object' } }] }] },
  ]
  for (const value of malformed) {
    const result = validateMetaWhatsAppWebhookPayload(value)
    assert.equal(result.ok, false, `expected rejection for ${JSON.stringify(value)}`)
  }
  assert.equal(validateMetaWhatsAppWebhookPayload(VALID_PAYLOAD).ok, true)
})

test('repository records external input is unknown until validated', () => {
  const phrase = 'unknown until a runtime schema validates it'
  for (const path of ['AGENTS.md', 'docs/agent/MEMORY.md']) {
    const content = readFileSync(path, 'utf8')
    assert.ok(content.includes(phrase), `${path} must record the rule`)
  }
})
