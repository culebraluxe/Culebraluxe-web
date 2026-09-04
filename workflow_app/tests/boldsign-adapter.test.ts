import test from 'node:test'
import assert from 'node:assert/strict'

import {
  BoldSignSignatureProvider,
  pdfAnchorToBoldSignBounds,
} from '../../lib/signature/boldsign/adapter'
import { BoldSignClient } from '../../lib/signature/boldsign/client'
import { loadBoldSignConfig, type BoldSignConfig } from '../../lib/signature/boldsign/config'
import { classifyBoldSignError, isTransientHttpStatus } from '../../lib/signature/boldsign/errors'
import { mapBoldSignWebhookEvent } from '../../lib/signature/boldsign/events'
import {
  parseBoldSignWebhookPayload,
  signBoldSignWebhook,
  verifyBoldSignWebhookSignature,
} from '../../lib/signature/boldsign/webhook'
import {
  BOLD_SIGN_DOCUMENT_STATUSES,
  mapProviderStatus,
} from '../../lib/signature/status-mapping'

const NOW = new Date('2026-08-22T00:00:00.000Z')
const NOW_SEC = Math.floor(NOW.getTime() / 1000)

function config(overrides: Partial<BoldSignConfig> = {}): BoldSignConfig {
  return {
    apiKey: 'test-api-key',
    baseUrl: 'https://boldsign.test',
    templateId: 'tpl-1',
    webhookSecret: 'webhook-secret',
    timeoutMs: 50,
    maxAttempts: 3,
    retryBaseDelayMs: 5,
    retryMaxDelayMs: 10,
    webhookToleranceSeconds: 300,
    ...overrides,
  }
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function webhookBody(
  eventId: string,
  eventType: string,
  documentId: string,
  status: string,
): string {
  return JSON.stringify({
    event: { id: eventId, eventType },
    data: { documentId, status },
  })
}

test('config: required secrets fail closed and values are never echoed', () => {
  const loaded = loadBoldSignConfig({
    BOLDSIGN_API_KEY: 'key',
    BOLDSIGN_BASE_URL: 'https://api.boldsign.com/',
    BOLDSIGN_WEBHOOK_SECRET: 'secret',
  })
  assert.equal(loaded.baseUrl, 'https://api.boldsign.com')
  assert.equal(loaded.templateId, '')
  assert.throws(() => loadBoldSignConfig({}), /BOLDSIGN_API_KEY/)
  try {
    loadBoldSignConfig({ BOLDSIGN_API_KEY: 'super-secret-value' })
    assert.fail('expected configuration failure')
  } catch (error) {
    assert.ok(!String(error).includes('super-secret-value'))
  }
})

test('status mapping is envelope-level and fails closed', () => {
  assert.equal(mapProviderStatus('bold-sign', 'InProgress'), 'sent')
  assert.equal(mapProviderStatus('bold-sign', 'Completed'), 'completed')
  assert.equal(mapProviderStatus('bold-sign', 'Declined'), 'declined')
  assert.equal(mapProviderStatus('bold-sign', 'Expired'), 'expired')
  assert.equal(mapProviderStatus('bold-sign', 'Revoked'), 'voided')
  assert.equal(mapProviderStatus('bold-sign', 'Draft'), 'requested')
  assert.equal(mapProviderStatus('bold-sign', 'BogusStatus'), 'error')
  assert.equal(BOLD_SIGN_DOCUMENT_STATUSES.length, 7)
})

test('webhook mapping keeps recipient Signed on active viewed plateau until Completed', () => {
  assert.equal(mapBoldSignWebhookEvent('Sent', 'InProgress'), 'sent')
  assert.equal(mapBoldSignWebhookEvent('Viewed', 'InProgress'), 'viewed')
  assert.equal(mapBoldSignWebhookEvent('Signed', 'InProgress'), 'viewed')
  assert.equal(mapBoldSignWebhookEvent('Completed', 'Completed'), 'completed')
  assert.equal(mapBoldSignWebhookEvent('Declined', 'Declined'), 'declined')
  assert.equal(mapBoldSignWebhookEvent('Revoked', 'Revoked'), 'voided')
  assert.equal(mapBoldSignWebhookEvent('Expired', 'Expired'), 'expired')
  assert.equal(mapBoldSignWebhookEvent('SendFailed', 'InProgress'), 'error')
  assert.equal(mapBoldSignWebhookEvent('Reminder', 'InProgress'), 'viewed')
  assert.throws(
    () => mapBoldSignWebhookEvent('Reassigned', 'InProgress'),
    /operator attention/,
  )
  assert.throws(
    () => mapBoldSignWebhookEvent('Reminder', 'BogusStatus'),
    /no neutral lifecycle mapping/,
  )
})

test('webhook HMAC verifies raw bytes and rejects replay/tampering', () => {
  const body = webhookBody('evt-1', 'Completed', 'env-1', 'Completed')
  const header = signBoldSignWebhook(body, 'webhook-secret', NOW_SEC)
  assert.doesNotThrow(() =>
    verifyBoldSignWebhookSignature(body, header, 'webhook-secret', NOW_SEC, 300),
  )
  assert.throws(() =>
    verifyBoldSignWebhookSignature(`${body} `, header, 'webhook-secret', NOW_SEC, 300),
  )
  const stale = signBoldSignWebhook(body, 'webhook-secret', NOW_SEC - 10_000)
  assert.throws(() =>
    verifyBoldSignWebhookSignature(body, stale, 'webhook-secret', NOW_SEC, 300),
    /tolerance/,
  )
})

test('webhook payload parser preserves provider event and envelope identity', () => {
  const parsed = parseBoldSignWebhookPayload(
    webhookBody('evt-1', 'Signed', 'env-1', 'InProgress'),
  )
  assert.equal(parsed.providerEventId, 'evt-1')
  assert.equal(parsed.eventType, 'Signed')
  assert.equal(parsed.envelopeId, 'env-1')
  assert.equal(parsed.documentStatus, 'InProgress')
  assert.throws(() => parseBoldSignWebhookPayload('not json'), /not valid JSON/)
})

test('legal envelope template creation is single-attempt on HTTP 500', async () => {
  let attempts = 0
  const delays: number[] = []
  const client = new BoldSignClient(config(), {
    fetchFn: async () => {
      attempts += 1
      return jsonResponse(500, { message: 'ambiguous provider failure' })
    },
    sleep: async (ms) => { delays.push(ms) },
  })
  await assert.rejects(
    () => client.sendEnvelopeFromTemplate({
      templateId: 'tpl-1',
      title: 'Agreement',
      message: null,
      roles: [{
        roleIndex: 1,
        signerName: 'Buyer',
        signerEmail: 'buyer@example.com',
        signerType: 'Signer',
      }],
    }),
    /HTTP 500/,
  )
  assert.equal(attempts, 1, 'never auto-resend an ambiguous legal-envelope create')
  assert.deepEqual(delays, [], 'no create retry/backoff is scheduled')
})

test('legal envelope direct-PDF creation is single-attempt on HTTP 500', async () => {
  let attempts = 0
  const client = new BoldSignClient(config(), {
    fetchFn: async () => {
      attempts += 1
      return jsonResponse(500, { message: 'ambiguous provider failure' })
    },
  })
  await assert.rejects(
    () => client.sendDocument({
      fileBytes: new Uint8Array([37, 80, 68, 70]),
      filename: 'agreement.pdf',
      mimeType: 'application/pdf',
      title: 'Agreement',
      message: null,
      signers: [{
        name: 'Buyer',
        emailAddress: 'buyer@example.com',
        signerType: 'Signer',
        signerOrder: 1,
        authenticationType: 'EmailOTP',
        formFields: [{
          fieldType: 'Signature',
          pageNumber: 1,
          bounds: { x: 50, y: 100, width: 200, height: 30 },
          isRequired: true,
          fontSize: 14,
        }],
      }],
      enableSigningOrder: true,
      completionCcEmails: ['lisa@culebraluxe.com'],
    }),
    /HTTP 500/,
  )
  assert.equal(attempts, 1, 'ambiguous direct-send outcome must not create a duplicate envelope')
})

test('legal envelope timeout is single-attempt and remains retryable for operator reconciliation', async () => {
  let attempts = 0
  const client = new BoldSignClient(config({ timeoutMs: 10 }), {
    fetchFn: async (_url, init) => {
      attempts += 1
      await new Promise<void>((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => {
          const error = new Error('aborted')
          error.name = 'AbortError'
          reject(error)
        })
      })
      return jsonResponse(200, {})
    },
  })
  let caught: unknown
  try {
    await client.sendEnvelopeFromTemplate({
      templateId: 'tpl-1', title: null, message: null,
      roles: [{ roleIndex: 1, signerName: 'A', signerEmail: 'a@example.com', signerType: 'Signer' }],
    })
  } catch (error) {
    caught = error
  }
  assert.ok(caught instanceof Error)
  assert.match(String(caught), /timed out/)
  assert.equal(attempts, 1)
  assert.equal(classifyBoldSignError(caught).retryable, true)
})

test('4xx create is single-attempt and non-retryable', async () => {
  let attempts = 0
  const client = new BoldSignClient(config(), {
    fetchFn: async () => {
      attempts += 1
      return jsonResponse(422, { message: 'invalid signer' })
    },
  })
  let caught: unknown
  try {
    await client.sendEnvelopeFromTemplate({
      templateId: 'tpl-1', title: null, message: null,
      roles: [{ roleIndex: 1, signerName: 'A', signerEmail: 'a@example.com', signerType: 'Signer' }],
    })
  } catch (error) {
    caught = error
  }
  assert.equal(attempts, 1)
  assert.equal(classifyBoldSignError(caught).retryable, false)
})

test('transient status vocabulary remains available for safe idempotent reads', () => {
  for (const status of [408, 429, 500, 502, 503, 504]) {
    assert.equal(isTransientHttpStatus(status), true)
  }
  for (const status of [400, 401, 403, 404, 422]) {
    assert.equal(isTransientHttpStatus(status), false)
  }
})

test('PDF bottom-left coordinates convert to BoldSign top-left bounds', () => {
  assert.deepEqual(
    pdfAnchorToBoldSignBounds({
      role: 'BUYER',
      slotId: 'BUYER:1',
      kind: 'signature',
      pageIndex: 1,
      pageWidth: 612,
      pageHeight: 792,
      rect: { x: 52, y: 170, width: 252, height: 34 },
      coordinateSpace: 'pdf-points-bottom-left',
    }),
    { x: 52, y: 588, width: 252, height: 34 },
  )
})

test('adapter type remains constructible behind the provider-neutral seam', () => {
  assert.equal(typeof BoldSignSignatureProvider, 'function')
})
