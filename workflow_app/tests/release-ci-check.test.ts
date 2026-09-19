import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import test from 'node:test'

// ---------------------------------------------------------------------------
// FORGE-LOCAL-RELEASE-CI-CHECK-01 — the release reads the gate, and never builds in CI.
//
// Astra review feature 4, and the captain endorsed it the same night: a deploy job in CI was added and then
// DELETED for cost (it was inert, but told the next reader how to switch it on, and CI deploys were doubling
// the bill). CI now checks and never ships; the deploy is local. That left a red main releasable by hand
// because nothing in the release path read the gate. This fence drives the READER with a fake provider, so
// every refusal is proven without touching the network.
// ---------------------------------------------------------------------------

const sha = 'a'.repeat(40)

function runCheck(conclusions: string, env: Record<string, string> = {}): { code: number; out: string } {
  const script = new URL('../../scripts/release-ci-check.sh', import.meta.url).pathname
  try {
    const out = execFileSync('bash', [script, sha], {
      encoding: 'utf8',
      env: { ...process.env, RELEASE_CI_CMD: `printf '${conclusions}'`, ...env },
      stdio: 'pipe',
    })
    return { code: 0, out: `${out}` }
  } catch (error) {
    const err = error as { status?: number; stdout?: string; stderr?: string }
    return { code: err.status ?? 1, out: `${err.stdout ?? ''}${err.stderr ?? ''}` }
  }
}

test('release gate: green CI lets the release proceed', () => {
  const { code, out } = runCheck('success\\n')
  assert.equal(code, 0)
  assert.match(out, /green/)
})

test('release gate: a FAILED run refuses', () => {
  const { code, out } = runCheck('failure\\n')
  assert.equal(code, 1)
  assert.match(out, /not green/)
})

test('release gate: a run still IN PROGRESS refuses rather than waits or assumes', () => {
  const { code, out } = runCheck('in_progress\\n')
  assert.equal(code, 1)
  assert.match(out, /still running/)
})

test('release gate: NO RUN for the sha refuses — an unverified commit is not releasable', () => {
  const { code, out } = runCheck('')
  assert.equal(code, 1)
  assert.match(out, /no CI run exists/)
})

test('release gate: an UNREADABLE reader refuses — "could not check" is not "fine"', () => {
  const { code, out } = runCheck('', { RELEASE_CI_CMD: 'exit 3' })
  assert.equal(code, 1)
  assert.match(out, /could not read/)
})

test('release gate: an UNKNOWN conclusion is not a pass', () => {
  const { code, out } = runCheck('some_new_state\\n')
  assert.equal(code, 1)
  assert.match(out, /unrecognised/)
})

test('release gate: the named opt-out exists, and it says the gate was NOT read', () => {
  const { code, out } = runCheck('failure\\n', { RELEASE_CI_CHECK: 'skip' })
  assert.equal(code, 0, 'the bypass must work, or somebody deletes the check during an outage')
  assert.match(out, /NOT read/)
})

test('release gate: the release script calls it BEFORE the build and re-reads HEAD afterwards', () => {
  // The structural half: a reader nobody calls protects nothing, and a check whose sha can move before the
  // build is a stale cite waiting to happen.
  const release = readFileSync(new URL('../../scripts/release-record.sh', import.meta.url), 'utf8')
  const gateAt = release.indexOf('release-ci-check.sh')
  const buildAt = release.indexOf('vercel-build-prod.sh')
  assert.notEqual(gateAt, -1, 'the release no longer reads the gate')
  assert.ok(gateAt < buildAt, 'the gate must be read BEFORE the build')
  assert.match(release, /AFTER_CHECK_SHA/, 'HEAD is not re-read after the check')
  assert.equal(
    /vercel-build-prod\.sh/.test(readFileSync(new URL('../../.github/workflows/gates.yml', import.meta.url), 'utf8')),
    false,
    'CI must never build or deploy: that is what doubled the bill',
  )
})
