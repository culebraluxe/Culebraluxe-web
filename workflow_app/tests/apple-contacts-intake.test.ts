import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  appleContactDisplayName,
  lowerAppleContactExport,
  parseAppleContactExportBatch,
} from '../../lib/intake/apple-contacts'
import { intakeSourceIdentity } from '../../lib/intake/contracts'

function rawBatch() {
  return {
    schemaVersion: 1,
    sourceSystem: 'apple_contacts',
    exportId: 'export-1',
    exportedAt: '2026-08-24T14:00:00.000Z',
    contacts: [
      {
        sourceId: 'apple-contact-1',
        namePrefix: '',
        givenName: 'John',
        middleName: '',
        familyName: 'Yang2020',
        nameSuffix: '',
        nickname: '',
        organization: '',
        department: '',
        jobTitle: '',
        emails: [],
        phones: [
          { sourceLabel: '_$!<Mobile>!$_', value: '13086437292' },
          { sourceLabel: '_$!<Mobile>!$_', value: '13086437292' },
        ],
        postalAddresses: [
          {
            sourceLabel: '_$!<Home>!$_',
            street: '1 Main St',
            city: 'Culebra',
            state: 'PR',
            postalCode: '00775',
            country: 'Puerto Rico',
            isoCountryCode: 'PR',
          },
        ],
      },
    ],
  }
}

test('parses the versioned Swift contact-export batch', () => {
  const batch = parseAppleContactExportBatch(rawBatch())
  assert.equal(batch.contacts.length, 1)
  assert.equal(batch.contacts[0].sourceId, 'apple-contact-1')
  assert.equal(batch.contacts[0].postalAddresses[0].postalCode, '00775')
})

test('rejects duplicate Apple source identities in one source artifact', () => {
  const raw = rawBatch()
  raw.contacts.push({ ...raw.contacts[0] })
  assert.throws(
    () => parseAppleContactExportBatch(raw),
    /Duplicate Apple sourceId/,
  )
})

test('lowers Apple contacts into the existing canonical batch contract', () => {
  const batch = parseAppleContactExportBatch(rawBatch())
  const messages = lowerAppleContactExport(batch, 'apple-contacts/export-1.json')
  assert.equal(messages.length, 1)
  const message = messages[0]
  assert.equal(message.acquisitionLane, 'batch')
  assert.equal(message.eventType, 'contact.imported')
  assert.deepEqual(intakeSourceIdentity(message), {
    source: 'apple_contacts',
    sourceAccount: '',
    externalEventId: 'apple-contact-1',
  })
  assert.equal(message.correlationId, 'export-1')
  assert.equal(message.provenance.adapter, 'apple-contacts.swift-json')
})

test('keeps full source facts while deduplicating identity candidates only', () => {
  const batch = parseAppleContactExportBatch(rawBatch())
  const [message] = lowerAppleContactExport(batch, 'apple-contacts/export-1.json')
  assert.equal(message.contactCandidates?.length, 1)
  assert.equal(message.participants.length, 1)
  const profile = message.sourcePayload as Record<string, unknown>
  assert.equal((profile.phones as unknown[]).length, 2)
  assert.equal((profile.postalAddresses as unknown[]).length, 1)
})

test('display name falls back from personal name to organization to source id', () => {
  const batch = parseAppleContactExportBatch(rawBatch())
  const base = batch.contacts[0]
  assert.equal(appleContactDisplayName(base), 'John Yang2020')
  assert.equal(
    appleContactDisplayName({
      ...base,
      givenName: '',
      familyName: '',
      organization: 'CulebraLuxe',
    }),
    'CulebraLuxe',
  )
  assert.equal(
    appleContactDisplayName({
      ...base,
      givenName: '',
      familyName: '',
      organization: '',
    }),
    'apple-contact-1',
  )
})

