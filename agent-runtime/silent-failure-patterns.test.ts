import assert from 'node:assert/strict'
import test from 'node:test'

import { findSilentFailures, newSilentFailures } from './silent-failure-patterns'

// ---------------------------------------------------------------------------
// Every pattern gets a failing fixture and a passing one. The gate exists because a sentence in
// AGENTS.md ("route every failure through the capture framework") did not hold when a catch silently
// dropped every record - so the tests prove the hunter can see that exact shape.
// ---------------------------------------------------------------------------

test('an empty catch is a hit', () => {
  const hits = findSilentFailures([
    { path: 'app/api/thing/route.ts', content: 'try {\n  risky()\n} catch {}\n' },
  ])
  assert.deepEqual(hits.map((h) => h.pattern), ['empty-catch'])
  assert.equal(hits[0].line, 3)
})

test('a catch that swallows into a default is a hit', () => {
  const hits = findSilentFailures([
    { path: 'lib/thing.ts', content: 'const rows = await load().catch(() => [])\n' },
  ])
  assert.deepEqual(hits.map((h) => h.pattern), ['swallowed-catch'])
})

test('console.error on a server surface with no capture is a hit; with capture it is not', () => {
  const without = findSilentFailures([
    { path: 'app/portal/thing.ts', content: 'console.error("nope")\n' },
  ])
  assert.deepEqual(without.map((h) => h.pattern), ['console-error-without-capture'])

  const withCapture = findSilentFailures([
    {
      path: 'app/portal/thing.ts',
      content: 'captureServerError("thing", err)\nconsole.error("context")\n',
    },
  ])
  assert.deepEqual(withCapture, [])
})

test('a catch returning a bare 500 body with no capture is a hit', () => {
  const hits = findSilentFailures([
    {
      path: 'app/api/thing/route.ts',
      content: 'try {\n  risky()\n} catch (e) {\n  return Response.json({ error: "x" }, { status: 500 })\n}\n',
    },
  ])
  assert.ok(hits.some((h) => h.pattern === 'bare-500-in-catch'))
})

test('a captured 500 is acceptable (the obligation is capturable failure, not no failure)', () => {
  const hits = findSilentFailures([
    {
      path: 'app/api/thing/route.ts',
      content:
        'try {\n  risky()\n} catch (e) {\n  captureServerError("thing", e)\n  return Response.json({}, { status: 500 })\n}\n',
    },
  ])
  assert.deepEqual(hits, [])
})

test('an empty success out of a catch is a hit, with the return line', () => {
  const hits = findSilentFailures([
    {
      path: 'app/api/portal/issues/route.ts',
      content: 'try {\n  risky()\n} catch {\n  return NextResponse.json({ rows: [], total: 0 })\n}\n',
    },
  ])
  assert.deepEqual(hits.map((h) => h.pattern), ['empty-success-in-catch'])
  assert.equal(hits[0].line, 4)
})

test('null, [], and a bare empty collection out of a catch are all hits', () => {
  const shapes = ['return null', 'return NextResponse.json([])', 'return NextResponse.json({ channels: [] })']
  for (const shape of shapes) {
    const hits = findSilentFailures([
      { path: 'app/api/x/route.ts', content: `try {\n  risky()\n} catch {\n  ${shape}\n}\n` },
    ])
    assert.deepEqual(hits.map((h) => h.pattern), ['empty-success-in-catch'], shape)
  }
})

test('a catch that captures and returns an error response is not an empty-success hit', () => {
  const hits = findSilentFailures([
    {
      path: 'app/api/x/route.ts',
      content:
        'try {\n  risky()\n} catch (err) {\n  captureServerError("x", err)\n  return NextResponse.json({ error: "database_unavailable", rows: [], total: 0 }, { status: 503 })\n}\n',
    },
  ])
  assert.deepEqual(hits, [])
})

test('an empty success is not read out of a later block past the catch', () => {
  const hits = findSilentFailures([
    {
      path: 'app/api/x/route.ts',
      content:
        'try {\n  risky()\n} catch (err) {\n  return report(err)\n}\nfunction next() {\n  return NextResponse.json([])\n}\n',
    },
  ])
  assert.deepEqual(hits, [])
})

test('scripts and tests are not judged as server surfaces', () => {
  const hits = findSilentFailures([
    { path: 'scripts/probe.ts', content: 'console.error("a script may say this")\n' },
  ])
  assert.deepEqual(hits, [])
})

test('only hits on ADDED lines fail the change; pre-existing hits are reported, not blocked', () => {
  const files = [
    { path: 'app/a.ts', content: 'try {\n  x()\n} catch {}\n' },
    { path: 'app/b.ts', content: 'try {\n  y()\n} catch {}\n' },
  ]
  const hits = findSilentFailures(files)
  assert.equal(hits.length, 2)

  // The change added line 3 of app/b.ts only.
  const added = new Map<string, Set<number>>([['app/b.ts', new Set([3])]])
  const blocking = newSilentFailures(hits, added)
  assert.deepEqual(
    blocking.map((h) => h.path),
    ['app/b.ts'],
  )
})
