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

type Run = {
  databaseId: number
  name: string
  workflowName: string
  headSha: string
  status: 'completed' | 'in_progress' | 'queued'
  conclusion: string
  attempt: number
}

/** A realistic `gh run list --json …` entry: pending is a STATUS with an empty conclusion, never a conclusion. */
const run = (over: Partial<Run> = {}): Run => ({
  databaseId: 100,
  name: 'Gates',
  workflowName: 'Gates',
  headSha: sha,
  status: 'completed',
  conclusion: 'success',
  attempt: 1,
  ...over,
})

function runCheck(runs: Run[] | string, env: Record<string, string> = {}): { code: number; out: string } {
  const script = new URL('../../../scripts/release-ci-check.sh', import.meta.url).pathname
  const payload = typeof runs === 'string' ? runs : JSON.stringify(runs)
  try {
    const out = execFileSync('bash', [script, sha], {
      encoding: 'utf8',
      env: {
        ...process.env,
        // The provider is a shell command, so the fixture is delivered as the reader's stdout — the same seam
        // `gh` uses. No network, and the JSON is the payload rather than a paraphrase of it.
        RELEASE_CI_CMD: `printf '%s' '${payload.replace(/'/g, "'\\''")}'`,
        ...env,
      },
      stdio: 'pipe',
    })
    return { code: 0, out: `${out}` }
  } catch (error) {
    const err = error as { status?: number; stdout?: string; stderr?: string }
    return { code: err.status ?? 1, out: `${err.stdout ?? ''}${err.stderr ?? ''}` }
  }
}

test('release gate: green CI lets the release proceed', () => {
  const { code, out } = runCheck([run()])
  assert.equal(code, 0)
  assert.match(out, /green/)
})

test('release gate: a PENDING required run is refused, even with an older success beside it', () => {
  // THE REVIEW'S REPRODUCTION, in its true shape: pending is `status: in_progress` with an EMPTY conclusion.
  // The old reader asked only for conclusions, so the pending run arrived as a blank line and the older success
  // read as green.
  const { code, out } = runCheck([
    run({ databaseId: 200, status: 'in_progress', conclusion: '' }),
    run({ databaseId: 100, status: 'completed', conclusion: 'success' }),
  ])
  assert.equal(code, 1)
  assert.match(out, /not finished/)
})

test('release gate: a QUEUED required run is refused for the same reason', () => {
  const { code, out } = runCheck([run({ databaseId: 201, status: 'queued', conclusion: '' })])
  assert.equal(code, 1)
  assert.match(out, /not finished/)
})

test('release gate: an UNRELATED workflow success cannot authorise the release', () => {
  const { code, out } = runCheck([
    run({ databaseId: 300, name: 'Deploy', workflowName: 'Deploy', status: 'completed', conclusion: 'success' }),
  ])
  assert.equal(code, 1)
  assert.match(out, /has no run for/)
})

test('release gate: a success for a DIFFERENT sha is refused', () => {
  const { code, out } = runCheck([run({ headSha: 'b'.repeat(40) })])
  assert.equal(code, 1)
  assert.match(out, /other SHAs/)
})

test('release gate: the LATEST successful rerun is not blocked by a superseded failed run', () => {
  const { code, out } = runCheck([
    run({ databaseId: 400, attempt: 1, status: 'completed', conclusion: 'failure' }),
    run({ databaseId: 401, attempt: 2, status: 'completed', conclusion: 'success' }),
  ])
  assert.equal(code, 0)
  assert.match(out, /attempt 2/)
})

test('release gate: a FAILED required run is refused, and so is a cancelled one', () => {
  const failed = runCheck([run({ conclusion: 'failure' })])
  assert.equal(failed.code, 1)
  assert.match(failed.out, /did not succeed/)
  const cancelled = runCheck([run({ conclusion: 'cancelled' })])
  assert.equal(cancelled.code, 1)
  assert.match(cancelled.out, /did not succeed/)
})

test('release gate: an unrecognised conclusion is not a pass', () => {
  const { code, out } = runCheck([run({ conclusion: 'some_new_state' })])
  assert.equal(code, 1)
  assert.match(out, /did not succeed/)
})

test('release gate: no run at all, and malformed provider output, both refuse', () => {
  const none = runCheck([])
  assert.equal(none.code, 1)
  assert.match(none.out, /has no run for/)

  const malformed = runCheck('not json at all')
  assert.equal(malformed.code, 1)
  assert.match(malformed.out, /malformed/)

  const wrongShape = runCheck('{"runs":[]}')
  assert.equal(wrongShape.code, 1)
  assert.match(wrongShape.out, /malformed/)
})

test('release gate: an UNREADABLE reader refuses — "could not check" is not "fine"', () => {
  const { code, out } = runCheck([], { RELEASE_CI_CMD: 'exit 3' })
  assert.equal(code, 1)
  assert.match(out, /could not read/)
})

test('release gate: the named opt-out exists, and it says the gate was NOT read', () => {
  const { code, out } = runCheck([run({ conclusion: 'failure' })], { RELEASE_CI_CHECK: 'skip' })
  assert.equal(code, 0, 'the bypass must work, or somebody deletes the check during an outage')
  assert.match(out, /NOT read/)
})

test('release gate: the release script calls it BEFORE the build and re-reads HEAD afterwards', () => {
  // The structural half: a reader nobody calls protects nothing, and a check whose sha can move before the
  // build is a stale cite waiting to happen.
  const release = readFileSync(new URL('../../../scripts/release-record.sh', import.meta.url), 'utf8')
  const gateAt = release.indexOf('release-ci-check.sh')
  const buildAt = release.indexOf('vercel-build-prod.sh')
  assert.notEqual(gateAt, -1, 'the release no longer reads the gate')
  assert.ok(gateAt < buildAt, 'the gate must be read BEFORE the build')
  assert.match(release, /AFTER_CHECK_SHA/, 'HEAD is not re-read after the check')
  assert.equal(
    /vercel-build-prod\.sh/.test(readFileSync(new URL('../../../.github/workflows/gates.yml', import.meta.url), 'utf8')),
    false,
    'CI must never build or deploy: that is what doubled the bill',
  )
})
