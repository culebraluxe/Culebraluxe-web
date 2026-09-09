import assert from 'node:assert/strict'
import { test } from 'node:test'
import { execFileSync } from 'node:child_process'
import { runStaticGate } from '../forge/forge-static-gate'

// dependency-cruiser must be available via pnpm in this checkout for the gate
// to run. When it is not installed (a minimal dev environment), skip rather
// than fail so the DB-free suite stays green; the integration still runs where
// the tool exists.
function depcruiseAvailable(): boolean {
  try {
    execFileSync('pnpm', ['exec', 'depcruise', '--version'], {
      stdio: 'ignore',
      timeout: 20_000,
    })
    return true
  } catch {
    return false
  }
}

test(
  'static gate: runs dependency-cruiser and passes on the bounded tree',
  { skip: depcruiseAvailable() ? false : 'dependency-cruiser is not installed in this checkout' },
  () => {
    const result = runStaticGate({
      workspace: process.cwd(),
      roots: ['db', 'services', 'workflow_app'],
      config: '.dependency-cruiser.js',
    })
    assert.equal(result.workspace, process.cwd())
    assert.equal(result.archOk, true)
    assert.deepEqual(result.archErrors, [])
    assert.equal(result.ok, true)
  },
)
