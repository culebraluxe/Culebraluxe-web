import assert from 'node:assert/strict'
import test from 'node:test'
import { buildArchitectDirective } from '@/legacy/workflow_app/forge/forge-architect-directive'

test('directive names the handoff marker and baseRef', () => {
  const text = buildArchitectDirective('deadbeefcafebabe', ['pnpm test'])
  assert.match(text, /FORGE_ARCHITECT_HANDOFF:/)
  assert.match(text, /deadbeefcafebabe/)
  assert.match(text, /pnpm test/)
  assert.doesNotMatch(text, /LEAD_ROUTING/)
})
