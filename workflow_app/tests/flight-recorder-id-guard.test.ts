// ENG-FORGE-RECORDER-ID-01 — a link that carries something other than an instance id must be a
// NAMED refusal, not a database error wearing a 503.
//
// Before this guard, `getFlightRecorderTransaction('ENG-FORGE-TURN-BUDGET-01')` sent the string to
// Postgres as a uuid, got `22P02 invalid input syntax for type uuid`, and the route answered
// "flight_recorder_unavailable" with a 503 — the operator's screen said the server was unavailable
// while the actual fault was the id in the link. Operator input is control flow, not a server
// fault, so the boundary now refuses it before the driver ever sees it.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  getFlightRecorderTransaction,
  isProcessInstanceId,
} from '../flight-recorder-read'

test('isProcessInstanceId accepts a real uuid, in either case', () => {
  assert.equal(isProcessInstanceId('25ae63c1-77e8-4b84-91a5-3f9894bf4f3d'), true)
  assert.equal(isProcessInstanceId('25AE63C1-77E8-4B84-91A5-3F9894BF4F3D'), true)
})

test('isProcessInstanceId rejects everything else a link might carry', () => {
  assert.equal(isProcessInstanceId(''), false)
  assert.equal(isProcessInstanceId('undefined'), false)
  assert.equal(isProcessInstanceId('null'), false)
  assert.equal(isProcessInstanceId('ENG-FORGE-TURN-BUDGET-01'), false)
  assert.equal(isProcessInstanceId(' 25ae63c1-77e8-4b84-91a5-3f9894bf4f3d '), false)
  assert.equal(isProcessInstanceId('25ae63c177e84b8491a53f9894bf4f3d'), false)
  assert.equal(isProcessInstanceId(null), false)
  assert.equal(isProcessInstanceId(undefined), false)
})

test('a malformed id returns null instead of throwing a driver error', async () => {
  // The regression itself: no throw, no `22P02`. null is what the route maps to 404 when the id
  // is well-formed, and the route refuses a malformed id with 400 before calling this at all.
  const result = await getFlightRecorderTransaction('ENG-FORGE-TURN-BUDGET-01')
  assert.equal(result, null)
})
