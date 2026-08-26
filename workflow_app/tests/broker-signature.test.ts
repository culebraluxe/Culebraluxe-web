import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  getBrokerSignatureConfig,
  resolveBrokerSignatureForIssuance,
  type BrokerSignatureConfig,
} from '../../db/broker-signature'
import type { QueryExecutor } from '../../db/query-executor'
import { getTemplate } from '../../lib/forms/template-registry'

const TEST_CONFIG: BrokerSignatureConfig = {
  enabled: true,
  configured: true,
  appUserId: 'owner-user',
  mediaId: 'protected-signature-media',
  signerName: 'Lisa Penfield',
  licenseNumber: 'C-9931',
}

function executor(): QueryExecutor {
  const image = readFileSync(
    join(process.cwd(), 'public/brand/CLLOGO.png'),
  )
  return async (strings) => {
    const sql = strings.join('?')
    if (sql.includes('from app_user u')) {
      return [
        {
          display_name: 'Lisa Penfield',
          person_id: 'lisa-person',
          active: true,
          is_owner: true,
        },
      ]
    }
    if (sql.includes('from media')) {
      return [{ file_data: image, mime_type: 'image/png' }]
    }
    return []
  }
}

test('broker signature config requires an explicit enabled gate and all bindings', () => {
  assert.deepEqual(getBrokerSignatureConfig({}), {
    enabled: false,
    appUserId: null,
    mediaId: null,
    signerName: null,
    licenseNumber: null,
    configured: false,
  })
  assert.equal(
    getBrokerSignatureConfig({
      BROKER_SIGNATURE_ENABLED: 'true',
      BROKER_SIGNATURE_APP_USER_ID: 'owner-user',
      BROKER_SIGNATURE_MEDIA_ID: 'media-id',
      BROKER_SIGNATURE_SIGNER_NAME: 'Lisa Penfield',
    }).configured,
    false,
    'the legal credential must be present before composition can be enabled',
  )
  assert.equal(
    getBrokerSignatureConfig({
      BROKER_SIGNATURE_ENABLED: 'true',
      BROKER_SIGNATURE_APP_USER_ID: 'owner-user',
      BROKER_SIGNATURE_MEDIA_ID: 'media-id',
      BROKER_SIGNATURE_SIGNER_NAME: 'Lisa Penfield',
      BROKER_SIGNATURE_LICENSE_NUMBER: 'C-9931',
    }).configured,
    true,
  )
})

test('authenticated owner issuance resolves protected signature material and slot', async () => {
  const template = getTemplate('SHOW-RPT')!
  const result = await resolveBrokerSignatureForIssuance(
    {
      template,
      values: { agentName: 'Lisa Penfield' },
      participants: [
        {
          role: 'BUYER_BROKER',
          slotId: 'BUYER_BROKER:1',
          personId: 'lisa-person',
          name: 'Lisa Penfield',
          email: 'owner@example.test',
          required: false,
          order: 0,
        },
      ],
      actorAppUserId: 'owner-user',
      issuedAt: '2026-08-26T18:30:00.000Z',
    },
    executor(),
    TEST_CONFIG,
  )
  assert.equal(result.ok, true)
  if (!result.ok) return
  assert.equal(result.signatures.length, 1)
  assert.equal(result.signatures[0].slotId, 'BUYER_BROKER:1')
  assert.equal(result.signatures[0].signerName, 'Lisa Penfield')
  assert.equal(
    result.signatures[0].credentialLine,
    'Real Estate Broker License #: C-9931',
  )
  assert.match(result.signatures[0].assetChecksumSha256, /^[0-9a-f]{64}$/)
  assert.ok(result.signatures[0].imageBytes.length > 0)
})

test('signature policy never signs another broker and rejects a different actor', async () => {
  const template = getTemplate('SHOW-RPT')!
  const otherBroker = await resolveBrokerSignatureForIssuance(
    {
      template,
      values: { agentName: 'Another Broker' },
      participants: [],
      actorAppUserId: 'owner-user',
      issuedAt: '2026-08-26T18:30:00.000Z',
    },
    executor(),
    TEST_CONFIG,
  )
  assert.deepEqual(otherBroker, { ok: true, signatures: [] })

  const wrongActor = await resolveBrokerSignatureForIssuance(
    {
      template,
      values: { agentName: 'Lisa Penfield' },
      participants: [],
      actorAppUserId: 'different-user',
      issuedAt: '2026-08-26T18:30:00.000Z',
    },
    executor(),
    TEST_CONFIG,
  )
  assert.equal(wrongActor.ok, false)
  if (!wrongActor.ok) assert.equal(wrongActor.outcome, 'unauthorized')
})

test('PR-PNS requires Lisa to occupy one immutable seller-broker slot', async () => {
  const template = getTemplate('PR-PNS')!
  const result = await resolveBrokerSignatureForIssuance(
    {
      template,
      values: { sellerBrokerName: 'Lisa Penfield' },
      participants: [],
      actorAppUserId: 'owner-user',
      issuedAt: '2026-08-26T18:30:00.000Z',
    },
    executor(),
    TEST_CONFIG,
  )
  assert.equal(result.ok, false)
  if (!result.ok) assert.match(result.message, /SELLER_BROKER execution slot/)
})
