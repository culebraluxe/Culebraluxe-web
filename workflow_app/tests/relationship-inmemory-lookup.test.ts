import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { QueryExecutor } from '../../db/query-executor'
import { semanticPhoneKey } from '../../db/person-identities'
import type { RelationshipEvidence } from '../../lib/relationship-intel/contracts'
import {
  createInMemoryPersonLookup,
  emailKey,
  phoneDigitsKey,
} from '../../lib/relationship-intel/inmemory-lookup'
import { reconcileEvidence } from '../../lib/relationship-intel/reconcile'

test('relationship lookup: NANP phone semantics match DB-backed identity lookup', () => {
  const variants = [
    '+1 (787) 555-0101',
    '1-787-555-0101',
    '7875550101',
  ]

  for (const value of variants) {
    assert.equal(phoneDigitsKey(value), '7875550101')
    assert.equal(phoneDigitsKey(value), semanticPhoneKey(value))
  }

  // Do not guess/strip non-NANP country codes.
  assert.equal(phoneDigitsKey('+44 20 7946 0958'), '442079460958')
  assert.equal(phoneDigitsKey('+44 20 7946 0958'), semanticPhoneKey('+44 20 7946 0958'))
})

test('relationship lookup: one Person owns many phone/email/source communication paths', async () => {
  const aliciaPersonId = 'person-alicia'

  const identityRows = [
    { identity_type: 'phone', identity_value: '+1 (787) 555-0101', person_id: aliciaPersonId },
    { identity_type: 'phone', identity_value: '7875550102', person_id: aliciaPersonId },
    { identity_type: 'email', identity_value: 'Alicia@Example.com', person_id: aliciaPersonId },
  ]
  const sourceLinks = [
    {
      source: 'apple_messages',
      source_account: 'local_mac',
      source_identity_key: '+17875550101',
      canonical_person_id: aliciaPersonId,
    },
    {
      source: 'apple_messages',
      source_account: 'local_mac',
      source_identity_key: '+17875550102',
      canonical_person_id: aliciaPersonId,
    },
    {
      source: 'gmail',
      source_account: 'broker@example.com',
      source_identity_key: 'alicia@example.com',
      canonical_person_id: aliciaPersonId,
    },
  ]

  const execute = (async (strings: TemplateStringsArray) => {
    const query = strings.join(' ').replace(/\s+/g, ' ').trim()
    if (query.includes('from person_identity pi')) return identityRows
    if (query.includes("to_regclass('public.integration_source_person_link')")) {
      return [{ name: 'integration_source_person_link' }]
    }
    if (query.includes('from integration_source_person_link')) return sourceLinks
    throw new Error(`unexpected test query: ${query}`)
  }) as QueryExecutor

  const { lookup, phoneToPerson, emailToPerson } = await createInMemoryPersonLookup(execute)

  // Both phone representations, including Apple-style E.164, resolve to Alicia.
  assert.deepEqual(await lookup.findPeopleByPhone('7875550101'), [{ personId: aliciaPersonId }])
  assert.deepEqual(await lookup.findPeopleByPhone('+1 (787) 555-0102'), [{ personId: aliciaPersonId }])

  // Email is another communication path on the same Person.
  assert.deepEqual(await lookup.findPeopleByEmail('ALICIA@example.com'), [{ personId: aliciaPersonId }])

  // Distinct source identities remain distinct while ownership converges on Person.
  assert.deepEqual(
    await lookup.findExplicitSourceLink('apple_messages', 'local_mac', '+17875550101'),
    { personId: aliciaPersonId },
  )
  assert.deepEqual(
    await lookup.findExplicitSourceLink('apple_messages', 'local_mac', '+17875550102'),
    { personId: aliciaPersonId },
  )
  assert.deepEqual(
    await lookup.findExplicitSourceLink('gmail', 'broker@example.com', 'alicia@example.com'),
    { personId: aliciaPersonId },
  )

  assert.deepEqual(phoneToPerson.get('7875550101'), [aliciaPersonId])
  assert.deepEqual(phoneToPerson.get('7875550102'), [aliciaPersonId])
  assert.equal(emailToPerson.get(emailKey('Alicia@Example.com')), aliciaPersonId)
})

test('relationship reconciliation: Apple ten-digit handle links to canonical +1 phone owner', async () => {
  const aliciaPersonId = 'person-alicia'
  const execute = (async (strings: TemplateStringsArray) => {
    const query = strings.join(' ').replace(/\s+/g, ' ').trim()
    if (query.includes('from person_identity pi')) {
      return [
        {
          identity_type: 'phone',
          identity_value: '+1 (787) 555-0101',
          person_id: aliciaPersonId,
        },
      ]
    }
    if (query.includes("to_regclass('public.integration_source_person_link')")) {
      return [{ name: 'integration_source_person_link' }]
    }
    if (query.includes('from integration_source_person_link')) return []
    throw new Error(`unexpected test query: ${query}`)
  }) as QueryExecutor

  const { lookup } = await createInMemoryPersonLookup(execute)
  const evidence: RelationshipEvidence = {
    source: 'apple_messages',
    sourceAccount: 'local_mac',
    sourceIdentityKey: '+17875550101',
    emails: [],
    phones: [{ value: '+17875550101', normalized: '7875550101', label: null }],
    inboundCount: 8,
    outboundCount: 7,
    isTwoWay: true,
    hasEmail: false,
    hasPhone: true,
  }

  const decision = await reconcileEvidence(evidence, lookup)
  assert.equal(decision.reviewState, 'exact_linked')
  assert.equal(decision.matchMethod, 'exact_phone')
  assert.equal(decision.canonicalPersonId, aliciaPersonId)
})
