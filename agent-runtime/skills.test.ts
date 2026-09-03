import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseSkillIds } from './skills'

test('parses comma and list skills', () => {
  assert.deepEqual(parseSkillIds('neon, forms'), ['neon', 'forms'])
  assert.deepEqual(parseSkillIds('- neon\n- workflow\n'), ['neon', 'workflow'])
})

test('drops unknown tokens', () => {
  assert.deepEqual(parseSkillIds('neon openclaw warp'), ['neon'])
})
