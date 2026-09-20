import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

/**
 * The fence for the Rust parity ledger.
 *
 * Two things are being protected. First, that the pairing map in `scripts/rust-parity-map.json` still describes the
 * repository: every route it claims is mounted, every path it names exists, and every mounted route belongs to a
 * capability. Second, that the committed ledger is CURRENT — `--check` regenerates in memory and fails on any
 * difference, so a new route or a renamed module cannot leave `docs/rust-parity-ledger.md` quietly wrong.
 *
 * This test is why the ledger can be trusted as evidence instead of report: it cannot drift without something
 * going red.
 */
function runLedger(extra: string[] = []): string {
  return execFileSync('node', ['--import', 'tsx', 'scripts/rust-parity-ledger.ts', ...extra], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  })
}

test('parity ledger: the map matches the repository and the run is clean', () => {
  const output = runLedger()
  assert.match(output, /0 consistency problem\(s\)/, `ledger reported drift: ${output}`)
  assert.match(output, /\d+ capabilities/, 'the ledger must report capability counts')
  assert.match(output, /\d+ Rust routes/, 'the ledger must read the router')
})

test('parity ledger: the committed ledger is current', () => {
  const before = readFileSync('docs/rust-parity-ledger.md', 'utf8')
  const output = runLedger(['--check'])
  assert.match(output, /\[check\]/, '--check must not rewrite the file')
  assert.equal(readFileSync('docs/rust-parity-ledger.md', 'utf8'), before, '--check must leave the file untouched')
})

test('parity ledger: a capability is never claimed as serving Rust without a route', () => {
  const map = JSON.parse(readFileSync('scripts/rust-parity-map.json', 'utf8')) as {
    capabilities: Array<{ id: string; productionPath: string; routes: string[] }>
  }
  for (const capability of map.capabilities) {
    if (capability.productionPath === 'rust') {
      assert.ok(
        capability.routes.length > 0,
        `${capability.id} claims productionPath=rust with no route — nothing could be calling it`,
      )
    }
  }
})

test('parity ledger: every route is either claimed or explicitly listed as infrastructure', () => {
  const map = JSON.parse(readFileSync('scripts/rust-parity-map.json', 'utf8')) as {
    capabilities: Array<{ id: string; routes: string[] }>
    knownUnmappedRoutes: string[]
  }
  const claimed = new Set(map.capabilities.flatMap((capability) => capability.routes))
  const router = readFileSync('rust/server/src/api/routes.rs', 'utf8')
  const mounted = [...router.matchAll(/\.route\(\s*"([^"]+)"/g)].map((match) => match[1])
  const unaccounted = mounted.filter(
    (route) => !claimed.has(route) && !map.knownUnmappedRoutes.includes(route),
  )
  assert.deepEqual(unaccounted, [], `routes mounted but unaccounted for: ${unaccounted.join(', ')}`)
})
