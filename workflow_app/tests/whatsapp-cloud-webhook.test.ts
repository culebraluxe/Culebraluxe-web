import assert from 'node:assert/strict'
import { createHmac } from 'node:crypto'
import { test } from 'node:test'

import { adaptWhatsAppEvent } from '../../lib/crm-whatsapp-normalization'
import { mapWhatsAppEvent } from '../../lib/integration-inbox/mapper'
import { loadMetaWhatsAppConfiguration } from '../../lib/whatsapp-cloud/config'
import { parseMetaWhatsAppWebhook } from '../../lib/whatsapp-cloud/parse'
import type { MetaWhatsAppWebhookPayload } from '../../lib/whatsapp-cloud/types'
import {
  verifyMetaWhatsAppHandshake,
  verifyMetaWhatsAppSignature,
} from '../../lib/whatsapp-cloud/verify'

const OWNED_PHONE = '+17875550000'
const EXTERNAL_PHONE = '+17875551212'
const PHONE_NUMBER_ID = '1234567890'
const OBSERVED_AT = '2026-08-29T05:30:00.000Z'
const MESSAGE_BODY = 'Is the Culebra lot still available?'

function inboundPayload(): MetaWhatsAppWebhookPayload {
  return {
    object: 'whatsapp_business_account',
    entry: [{
      id: 'waba-1',
      changes: [{
        field: 'messages',
        value: {
          messaging_product: 'whatsapp',
          metadata: {
            display_phone_number: OWNED_PHONE,
            phone_number_id: PHONE_NUMBER_ID,
          },
          contacts: [{
            wa_id: EXTERNAL_PHONE.slice(1),
            profile: { name: 'Test Buyer' },
          }],
          messages: [{
            from: EXTERNAL_PHONE.slice(1),
            id: 'wamid.INBOUND_TEST_001',
            timestamp: '1787980800',
            type: 'text',
            text: { body: MESSAGE_BODY },
          }],
        },
      }],
    }],
  }
}

function echoPayload(): MetaWhatsAppWebhookPayload {
  return {
    object: 'whatsapp_business_account',
    entry: [{
      changes: [{
        field: 'smb_message_echoes',
        value: {
          messaging_product: 'whatsapp',
          metadata: { phone_number_id: PHONE_NUMBER_ID },
          message_echoes: [{
            from: OWNED_PHONE.slice(1),
            to: EXTERNAL_PHONE.slice(1),
            id: 'wamid.ECHO_TEST_001',
            timestamp: '1787980860',
            type: 'text',
            text: { body: 'Yes, I can show it Saturday.' },
          }],
        },
      }],
    }],
  }
}

test('Meta handshake and raw-body signature fail closed', () => {
  assert.equal(verifyMetaWhatsAppHandshake({
    mode: 'subscribe',
    token: 'expected-token',
    challenge: 'challenge-1',
    expectedToken: 'expected-token',
  }), 'challenge-1')
  assert.equal(verifyMetaWhatsAppHandshake({
    mode: 'subscribe',
    token: 'wrong-token',
    challenge: 'challenge-1',
    expectedToken: 'expected-token',
  }), null)

  const body = JSON.stringify(inboundPayload())
  const secret = 'test-app-secret'
  const signature = `sha256=${createHmac('sha256', secret).update(body).digest('hex')}`
  assert.equal(verifyMetaWhatsAppSignature(body, signature, secret), true)
  assert.equal(verifyMetaWhatsAppSignature(`${body} `, signature, secret), false)
  assert.equal(verifyMetaWhatsAppSignature(body, signature.slice(7), secret), false)
})

test('Meta inbound lowers to metadata-only ODS fact and canonical WhatsApp input', () => {
  const events = parseMetaWhatsAppWebhook({
    payload: inboundPayload(),
    phoneNumberId: PHONE_NUMBER_ID,
    ownedPhoneE164: OWNED_PHONE,
    observedAt: OBSERVED_AT,
  })
  assert.equal(events.length, 1)
  const event = events[0]
  assert.equal(event.source, 'whatsapp')
  assert.equal(event.sourceAccount, `meta-${PHONE_NUMBER_ID}`)
  assert.equal(event.direction, 'inbound')
  assert.equal(event.contactCandidates?.[0]?.value, EXTERNAL_PHONE)
  assert.equal(event.contactCandidates?.[0]?.displayName, 'Test Buyer')
  assert.equal(event.content, undefined)
  assert.doesNotMatch(JSON.stringify(event), new RegExp(MESSAGE_BODY))

  const adapted = adaptWhatsAppEvent(
    mapWhatsAppEvent(event, 'meta'),
    { provider: 'meta', ownedLines: [{ phone: OWNED_PHONE }] },
  )
  assert.equal(adapted.status, 'accepted')
  if (adapted.status !== 'accepted') return
  assert.equal(adapted.inboundEvent.source.system, `communications:meta:meta-${PHONE_NUMBER_ID}`)
  assert.equal(adapted.inboundEvent.source.externalId, 'whatsapp:wamid.INBOUND_TEST_001')
  assert.equal(adapted.inboundEvent.direction, 'inbound')
  assert.equal(adapted.inboundEvent.content?.summary, undefined)
  assert.equal(adapted.inboundEvent.rawMetadata.messageType, 'text')
})

test('Meta phone-app echo lowers as outbound without retaining its body', () => {
  const events = parseMetaWhatsAppWebhook({
    payload: echoPayload(),
    phoneNumberId: PHONE_NUMBER_ID,
    ownedPhoneE164: OWNED_PHONE,
    observedAt: OBSERVED_AT,
  })
  assert.equal(events.length, 1)
  assert.equal(events[0].direction, 'outbound')
  assert.equal(events[0].contactCandidates?.[0]?.value, EXTERNAL_PHONE)
  assert.equal(events[0].content, undefined)
  assert.doesNotMatch(JSON.stringify(events[0]), /show it Saturday/)
})

test('Meta payloads for another business phone are ignored', () => {
  const events = parseMetaWhatsAppWebhook({
    payload: inboundPayload(),
    phoneNumberId: 'different-phone-number-id',
    ownedPhoneE164: OWNED_PHONE,
    observedAt: OBSERVED_AT,
  })
  assert.deepEqual(events, [])
})

test('configuration requires every provider secret and strict owned E.164 phone', () => {
  assert.deepEqual(loadMetaWhatsAppConfiguration({
    WHATSAPP_APP_SECRET: 'secret',
    WHATSAPP_PHONE_NUMBER_ID: PHONE_NUMBER_ID,
    WHATSAPP_OWNED_PHONE_E164: OWNED_PHONE,
  } as NodeJS.ProcessEnv), {
    appSecret: 'secret',
    phoneNumberId: PHONE_NUMBER_ID,
    ownedPhoneE164: OWNED_PHONE,
  })
  assert.throws(() => loadMetaWhatsAppConfiguration({
    WHATSAPP_APP_SECRET: 'secret',
    WHATSAPP_PHONE_NUMBER_ID: PHONE_NUMBER_ID,
  } as NodeJS.ProcessEnv), /WHATSAPP_OWNED_PHONE_E164/)
})
