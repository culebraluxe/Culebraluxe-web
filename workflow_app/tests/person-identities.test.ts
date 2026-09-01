import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  findIdentityMatch,
  findIdentityMatches,
  semanticPhoneKey,
} from '../../db/person-identities'
import type { QueryExecutor } from '../../db/query-executor'

test('NANP semantic key equates ten digits with E.164 +1', () => {
  assert.equal(semanticPhoneKey('617-251-6169'), '6172516169')
  assert.equal(semanticPhoneKey('+1 617-251-6169'), '6172516169')
  assert.equal(semanticPhoneKey('16172516169'), '6172516169')
})

test('semantic key leaves non-NANP international digits exact', () => {
  assert.equal(semanticPhoneKey('+34 689 351 739'), '34689351739')
})

test('identity SQL compares stored NANP values through the semantic key', async () => {
  let query = ''
  let params: unknown[] = []
  const execute: QueryExecutor = async (strings, ...values) => {
    query = strings.join('?').replace(/\s+/g, ' ').trim()
    params = values
    return [{
      identity_id: 'identity-1',
      person_id: 'person-1',
      identity_value: '+16172516169',
    }]
  }

  const matches = await findIdentityMatches({
    kind: 'phone',
    value: '+16172516169',
    normalizedValue: '+16172516169',
    evidence: 'user_supplied',
  }, execute)

  assert.equal(matches.length, 1)
  assert.equal(matches[0].personId, 'person-1')
  assert.ok(query.includes("then substring(regexp_replace(pi.identity_value"))
  assert.ok(params.includes('6172516169'), 'lookup parameter uses the ten-digit NANP key')
})

test('multiple semantic owners remain an explicit ambiguity', async () => {
  const execute: QueryExecutor = async () => [
    {
      identity_id: 'identity-1',
      person_id: 'person-1',
      identity_value: '6172516169',
    },
    {
      identity_id: 'identity-2',
      person_id: 'person-2',
      identity_value: '+16172516169',
    },
  ]

  await assert.rejects(
    () => findIdentityMatch({
      kind: 'phone',
      value: '+16172516169',
      normalizedValue: '+16172516169',
      evidence: 'user_supplied',
    }, execute),
    /ambiguous canonical identity ownership/,
  )
})
