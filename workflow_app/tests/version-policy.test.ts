import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  definitionVersionPolicy,
  IMMUTABLE_DEFINITION_ERROR,
} from '../definitions/version-policy'

test('a missing (key, version) row is new', () => {
  assert.deepEqual(definitionVersionPolicy(false, 0), { kind: 'new' })
})

test('an existing version with no instances is replaceable (draft iteration)', () => {
  assert.deepEqual(definitionVersionPolicy(true, 0), { kind: 'replaceable' })
})

test('an existing version with instances is immutable', () => {
  assert.deepEqual(definitionVersionPolicy(true, 1), { kind: 'immutable' })
  assert.deepEqual(definitionVersionPolicy(true, 99), { kind: 'immutable' })
})

test('the immutable error message names the refusal', () => {
  assert.match(IMMUTABLE_DEFINITION_ERROR, /Refusing to replace/)
})
