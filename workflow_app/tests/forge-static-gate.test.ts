import assert from 'node:assert/strict'
import { test } from 'node:test'
import { runStaticGate } from '../forge/forge-static-gate'

// The deterministic static gate, run against the real tree (architecture is the
// hard gate). dependency-cruiser must be available via pnpm in this checkout.
test('static gate: runs dependency-cruiser and passes on the bounded tree', () => {
  const result = runStaticGate({
    workspace: process.cwd(),
    roots: ['db', 'services', 'workflow_app'],
    config: '.dependency-cruiser.js',
  })
  assert.equal(result.workspace, process.cwd())
  assert.equal(result.archOk, true)
  assert.deepEqual(result.archErrors, [])
  assert.equal(result.ok, true)
})
