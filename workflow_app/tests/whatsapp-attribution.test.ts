// APP-WHATSAPP-ATTRIBUTION-01
//
// Drives the pure Meta-envelope -> landing mapping from a realistic payload:
// envelope metadata, a contacts entry and an inbound text. No live request and
// no database — the mapping is pure, so the acceptance clauses are asserted on
// the mapped LandedWhatsapp directly.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  mapWhatsAppMessageToLanding,
  whatsappThreadKey,
} from '../../lib/whatsapp-cloud/attribution'
import type {
  MetaWhatsAppChangeValue,
  MetaWhatsAppMessage,
} from '../../lib/whatsapp-cloud/types'

const RECEIVING_LINE = '+17875550100'
const PHONE_NUMBER_ID = '109999888777'
const CONTACT_WA_ID = '17875551234'

function envelope(
  contacts?: MetaWhatsAppChangeValue['contacts'],
): MetaWhatsAppChangeValue {
  return {
    messaging_product: 'whatsapp',
    metadata: {
      display_phone_number: RECEIVING_LINE,
      phone_number_id: PHONE_NUMBER_ID,
    },
    contacts: contacts ?? [
      { wa_id: CONTACT_WA_ID, profile: { name: 'Ada Lovelace' } },
    ],
    messages: [],
  }
}

function inbound(
  id: string,
  timestamp: string,
  extra: Partial<MetaWhatsAppMessage> = {},
): MetaWhatsAppMessage {
  return {
    id,
    from: CONTACT_WA_ID,
    type: 'text',
    timestamp,
    text: { body: 'hola' },
    ...extra,
  }
}

test('inbound to_address is the receiving line and source_account keeps phone_number_id', () => {
  const landed = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.ONE', '1700000000'),
  })

  assert.equal(landed.toAddress, RECEIVING_LINE)
  assert.equal(landed.sourceAccount, PHONE_NUMBER_ID)
  assert.equal(landed.fromAddress, CONTACT_WA_ID)
  assert.notEqual(landed.toAddress, CONTACT_WA_ID)
  assert.equal(landed.messageType, 'text')
  assert.equal(landed.text, 'hola')
})

test('source_account falls back to the configured id when the envelope omits phone_number_id', () => {
  const landed = mapWhatsAppMessageToLanding({
    value: {
      metadata: { display_phone_number: RECEIVING_LINE },
      contacts: [],
      messages: [],
    },
    direction: 'incoming',
    sourceAccount: 'configured-phone-number-id',
    message: inbound('wamid.FALLBACK', '1700000000'),
  })

  assert.equal(landed.sourceAccount, 'configured-phone-number-id')
  assert.equal(landed.toAddress, RECEIVING_LINE)
})

test('two messages between the same pair share one conversation_id and the newest sent_at answers the thread', () => {
  const first = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.A', '1700000000'),
  })
  const second = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.B', '1700000600'),
  })

  assert.equal(first.conversationId, second.conversationId)
  assert.equal(first.conversationId, whatsappThreadKey(RECEIVING_LINE, CONTACT_WA_ID))

  const thread = [first, second].filter(
    (row) => row.conversationId === first.conversationId,
  )
  const newest = thread.reduce<string | null>(
    (latest, row) =>
      row.sentAt && (!latest || row.sentAt > latest) ? row.sentAt : latest,
    null,
  )
  assert.equal(newest, second.sentAt)
  assert.notEqual(newest, first.sentAt)
})

test('the quoted reply context id is recorded, and is never the conversation_id', () => {
  const landed = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.C', '1700000000', { context: { id: 'wamid.QUOTED' } }),
  })

  assert.equal(landed.contextId, 'wamid.QUOTED')
  assert.notEqual(landed.conversationId, 'wamid.QUOTED')
  assert.equal(landed.conversationId, whatsappThreadKey(RECEIVING_LINE, CONTACT_WA_ID))
})

test('the contact name is captured and raw carries the envelope facts read, not the message object', () => {
  const landed = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.D', '1700000000'),
  })

  const raw = landed.raw as {
    metadata?: { display_phone_number?: string; phone_number_id?: string }
    contact?: { wa_id?: string; profile?: { name?: string } }
    text?: unknown
  }

  assert.equal(raw.metadata?.display_phone_number, RECEIVING_LINE)
  assert.equal(raw.metadata?.phone_number_id, PHONE_NUMBER_ID)
  assert.equal(raw.contact?.wa_id, CONTACT_WA_ID)
  assert.equal(raw.contact?.profile?.name, 'Ada Lovelace')
  // The bare message object must no longer be what the row claims as raw.
  assert.equal(raw.text, undefined)
})

test('the matching contact is chosen by wa_id when several contacts are present', () => {
  const landed = mapWhatsAppMessageToLanding({
    value: envelope([
      { wa_id: '10000000000', profile: { name: 'Someone Else' } },
      { wa_id: CONTACT_WA_ID, profile: { name: 'Ada Lovelace' } },
    ]),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.E', '1700000000'),
  })

  const raw = landed.raw as { contact?: { profile?: { name?: string } } }
  assert.equal(raw.contact?.profile?.name, 'Ada Lovelace')
})

test('an outgoing echo threads to the same pair using the recipient as counterparty', () => {
  const inboundRow = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'incoming',
    sourceAccount: null,
    message: inbound('wamid.F', '1700000000'),
  })
  const echo = mapWhatsAppMessageToLanding({
    value: envelope(),
    direction: 'outgoing',
    sourceAccount: null,
    message: {
      id: 'wamid.G',
      from: PHONE_NUMBER_ID,
      to: CONTACT_WA_ID,
      type: 'text',
      timestamp: '1700000600',
      text: { body: 'reply' },
    },
  })

  assert.equal(echo.toAddress, RECEIVING_LINE)
  assert.equal(echo.conversationId, inboundRow.conversationId)
})
