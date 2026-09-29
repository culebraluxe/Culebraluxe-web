import assert from 'node:assert/strict'
import test from 'node:test'

import { addedLinesFromDiff, evaluateGate } from '@/scripts/forge-silent-failure-gate'

// ---------------------------------------------------------------------------
// The gate exists because a note is not a block. These fixtures prove the exit code and the
// file:line it names, that a reporting catch is not flagged, and that no path is exempt.
// ---------------------------------------------------------------------------

const emptySuccess = 'try {\n  risky()\n} catch {\n  return NextResponse.json({ rows: [], total: 0 })\n}\n'

function lines(content: string): Set<number> {
  return new Set(Array.from({ length: content.split('\n').length }, (_, i) => i + 1))
}

test('an empty success out of a catch exits non-zero and names the file and line', () => {
  const file = { path: 'app/api/portal/issues/route.ts', content: emptySuccess }
  const result = evaluateGate([file], new Map([[file.path, lines(file.content)]]))
  assert.equal(result.exitCode, 1)
  assert.match(result.output, /app\/api\/portal\/issues\/route\.ts:4/)
})

test('null, [], and a bare empty collection all block', () => {
  const shapes = ['return null', 'return NextResponse.json([])', 'return NextResponse.json({ channels: [] })']
  for (const shape of shapes) {
    const content = `try {\n  risky()\n} catch {\n  ${shape}\n}\n`
    const file = { path: 'app/api/x/route.ts', content }
    const result = evaluateGate([file], new Map([[file.path, lines(content)]]))
    assert.equal(result.exitCode, 1, shape)
  }
})

test('a catch that reports the error and returns an error response is not flagged', () => {
  const content =
    'try {\n  risky()\n} catch (err) {\n  captureServerError("issues", err)\n  return NextResponse.json({ error: "database_unavailable", rows: [], total: 0 }, { status: 503 })\n}\n'
  const file = { path: 'app/api/portal/issues/route.ts', content }
  const result = evaluateGate([file], new Map([[file.path, lines(content)]]))
  assert.equal(result.exitCode, 0)
  assert.equal(result.output, '')
})

test('a pre-existing empty success outside the added lines is reported, not blocked', () => {
  const content = 'try {\n  risky()\n} catch {\n  return null\n}\n'
  const file = { path: 'app/api/x/route.ts', content }
  const result = evaluateGate([file], new Map([[file.path, new Set([1, 2])]]))
  assert.equal(result.exitCode, 0)
})

test('there is no exemption list: any server-surface path blocks', () => {
  for (const path of ['app/api/a/route.ts', 'app/portal/forms/actions.ts', 'legacy/services/x/y.ts']) {
    const content = 'try {\n  risky()\n} catch {\n  return []\n}\n'
    const file = { path, content }
    const result = evaluateGate([file], new Map([[file.path, lines(content)]]))
    assert.equal(result.exitCode, 1, path)
  }
})

test('a test file that contains the pattern is out of scope', () => {
  const file = { path: 'app/api/x/route.test.ts', content: emptySuccess }
  const result = evaluateGate([file], new Map([[file.path, lines(file.content)]]))
  assert.equal(result.exitCode, 0)
})

test('added lines are parsed from a diff hunk', () => {
  const diff = ['diff --git a/app/api/x/route.ts b/app/api/x/route.ts', '+++ b/app/api/x/route.ts', '@@ -1,2 +1,4 @@', '+a', '+b'].join(
    '\n',
  )
  const added = addedLinesFromDiff(diff)
  assert.deepEqual([...added.get('app/api/x/route.ts')!].sort((a, b) => a - b), [1, 2, 3, 4])
})
