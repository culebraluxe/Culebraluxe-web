import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createDiagnosticThrottle,
  refusalRecord,
  refusalResponse,
} from '@/app/api/portal/diagnostic-throttle'

// ---------------------------------------------------------------------------
// ENG-FORGE-TWO-UNIT-DOGFOOD-02 — UNIT B.
//
// The two anonymous diagnostic routes could be flooded: every POST wrote a row
// and nothing bounded the writer. These lock the bound and the anonymity.
// ---------------------------------------------------------------------------

test('bounds-writes-per-source-per-window', () => {
  const throttle = createDiagnosticThrottle({
    windowMs: 1_000,
    maxWrites: 2,
    maxBodyBytes: 100,
  })

  assert.equal(throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: 0 }).allowed, true)
  assert.equal(throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: 1 }).allowed, true)

  const over = throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: 2 })
  assert.equal(over.allowed, false, 'the write over the bound is refused')
  if (!over.allowed) {
    assert.equal(over.reason, 'rate')
    assert.equal(over.status, 429)
  }

  assert.equal(
    throttle.checkDiagnosticWrite({ source: 'b', bodyBytes: 1, now: 2 }).allowed,
    true,
    'a different source keeps its own budget',
  )
  assert.equal(
    throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: 1_002 }).allowed,
    true,
    'the window rolls and the source may write again',
  )
})

test('bounds-body-size', () => {
  const throttle = createDiagnosticThrottle({
    windowMs: 1_000,
    maxWrites: 5,
    maxBodyBytes: 10,
  })

  const over = throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 11, now: 0 })
  assert.equal(over.allowed, false, 'an oversized body is refused')
  if (!over.allowed) {
    assert.equal(over.reason, 'size')
    assert.equal(over.status, 413)
  }
  assert.equal(
    throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 10, now: 0 }).allowed,
    true,
    'a body exactly at the bound is accepted',
  )
})

test('over-bound-is-refused-with-a-plain-response', async () => {
  const tooMany = refusalResponse(429)
  assert.equal(tooMany.status, 429)
  assert.equal(await tooMany.text(), 'Too Many Requests')
  assert.equal(tooMany.headers.get('content-type'), 'text/plain; charset=utf-8')

  const tooLarge = refusalResponse(413)
  assert.equal(tooLarge.status, 413)
  assert.equal(await tooLarge.text(), 'Payload Too Large')
})

test('refusals-are-counted-not-written-per-attempt', () => {
  const throttle = createDiagnosticThrottle({
    windowMs: 1_000,
    maxWrites: 1,
    maxBodyBytes: 100,
  })

  assert.equal(throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: 0 }).allowed, true)

  for (let i = 1; i <= 5; i++) {
    const refused = throttle.checkDiagnosticWrite({ source: 'a', bodyBytes: 1, now: i })
    assert.equal(refused.allowed, false)
    assert.equal(
      throttle.takeRefusalReport(i),
      null,
      'the burst is not reported one row per attempt',
    )
  }

  assert.deepEqual(throttle.takeRefusalReport(1_001), { count: 5, reason: 'rate' })
  assert.equal(throttle.takeRefusalReport(1_001), null, 'the report resets after it is taken')
})

test('refusals-carry-no-source-identity', () => {
  const throttle = createDiagnosticThrottle({
    windowMs: 1_000,
    maxWrites: 1,
    maxBodyBytes: 100,
  })
  const source = '203.0.113.9'

  throttle.checkDiagnosticWrite({ source, bodyBytes: 1, now: 0 })
  const refused = throttle.checkDiagnosticWrite({ source, bodyBytes: 1, now: 1 })
  assert.equal(refused.allowed, false)
  assert.ok(!JSON.stringify(refused).includes(source), 'the verdict carries no source')

  const record = refusalRecord({ endpoint: 'api/portal/move-trace', reason: 'rate', count: 3 })
  const serialized = JSON.stringify(record)
  assert.ok(!serialized.includes(source), 'the recorded row carries no source')
  assert.ok(!/"(source|ip|address|forwarded)"/i.test(serialized), 'no source-shaped field exists')
})
