import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { parsePostgresInstant } from '@/legacy/db/forge-engine-task-execution'
import {
  InterruptedSequenceCrash,
  assertDriverValueShape,
  interruptedSequence,
  isDriverNumericText,
  isDriverTimestamptzText,
} from '@/legacy/workflow_app/tests/helpers/interrupted-sequence'

// ---------------------------------------------------------------------------
// ENG-FORGE-CRASH-TESTS-01 — the interrupted-sequence class, exercised.
//
// Every test below names the durable write it interrupts and reads the durable
// state left behind. The proof is env-free: the store is in memory, so it opens
// no connection and writes no database row. The driver value-format fixture is
// captured from the real driver by scripts/capture-driver-value-formats.ts and
// checked in; a guard rejects any value in a shape the driver never emits.
// ---------------------------------------------------------------------------

const FIXTURE = JSON.parse(
  readFileSync(new URL('./fixtures/driver-value-formats.json', import.meta.url), 'utf8'),
) as { values: Record<string, string> }

// ---------------------------------------------------------------------------
// The completion unit: the engine transition, then the claim-first unit that
// merges the evidence and finalizes the receipt. A crash between the two leaves
// the transition durable, the evidence unwritten and no receipt — the durable
// signal the resume reconciles on. The unit is exactly-once behind its receipt.
// ---------------------------------------------------------------------------

const COMPLETION_RECEIPT = 'forge.completion:task-1'
const COMPLETION_EVIDENCE = { leadDecision: 'SMITH' }

function completionStore() {
  const receipts = new Map<string, string>()
  const store = {
    transitioned: false,
    evidence: null as Record<string, unknown> | null,
    evidenceMerges: 0,
    finalizeCount: 0,
    transition: () => {
      store.transitioned = true
    },
    mergeEvidence: (evidence: Record<string, unknown>) => {
      store.evidenceMerges += 1
      store.evidence = evidence
    },
    finalize: (receiptId: string) => {
      // Claim-first: a receipt that already committed is never re-applied.
      if (receipts.has(receiptId)) return
      store.finalizeCount += 1
      receipts.set(receiptId, 'success')
    },
    receipt: (receiptId: string) => receipts.get(receiptId) ?? null,
  }
  return store
}

function completionWrites(store: ReturnType<typeof completionStore>) {
  return [
    { name: 'engine.transition', run: () => store.transition() },
    {
      name: 'completion.evidence.merge',
      run: () => store.mergeEvidence(COMPLETION_EVIDENCE),
    },
    {
      name: 'completion.receipt.finalize',
      run: () => store.finalize(COMPLETION_RECEIPT),
    },
  ]
}

test('interrupted: completion then evidence leaves evidence unwritten', async () => {
  const store = completionStore()
  const sequence = interruptedSequence(completionWrites(store))

  await assert.rejects(
    () => sequence.crashAfter('engine.transition'),
    InterruptedSequenceCrash,
  )

  assert.equal(store.transitioned, true, 'the engine transition is durable')
  assert.equal(store.evidence, null, 'the evidence merge did not commit')
  assert.equal(store.receipt(COMPLETION_RECEIPT), null, 'no receipt was committed')
  assert.deepEqual(sequence.committed(), ['engine.transition'])
  assert.deepEqual(sequence.pending(), [
    'completion.evidence.merge',
    'completion.receipt.finalize',
  ])
})

test('interrupted: completion then evidence resumes exactly once', async () => {
  const store = completionStore()
  const sequence = interruptedSequence(completionWrites(store))
  await assert.rejects(
    () => sequence.crashAfter('engine.transition'),
    InterruptedSequenceCrash,
  )

  await sequence.resume()
  assert.deepEqual(store.evidence, COMPLETION_EVIDENCE, 'the resume completes the evidence merge')
  assert.equal(store.receipt(COMPLETION_RECEIPT), 'success', 'the resume leaves its receipt')
  assert.equal(store.evidenceMerges, 1)
  assert.equal(store.finalizeCount, 1)

  await sequence.resume()
  assert.equal(store.evidenceMerges, 1, 'a second resume must not re-merge the evidence')
  assert.equal(store.finalizeCount, 1, 'a second resume must not re-write the receipt')
  assert.deepEqual(sequence.pending(), [])
})

// ---------------------------------------------------------------------------
// The publish/receipt pair: the accepted-candidate publish is durable before the
// command receipt is written. A crash between them leaves the publish durable and
// the receipt absent; the resume writes the receipt exactly once.
// ---------------------------------------------------------------------------

const PUBLISH_RECEIPT = 'publish:ENG-FORGE-CRASH-TESTS-01'
const PUBLISHED_HASH = 'c0ffee'.repeat(6)

function publishStore() {
  const receipts = new Map<string, string>()
  const store = {
    publishedHash: null as string | null,
    finalizeCount: 0,
    publish: (hash: string) => {
      store.publishedHash = hash
    },
    finalize: (receiptId: string) => {
      if (receipts.has(receiptId)) return
      store.finalizeCount += 1
      receipts.set(receiptId, 'success')
    },
    receipt: (receiptId: string) => receipts.get(receiptId) ?? null,
  }
  return store
}

function publishWrites(store: ReturnType<typeof publishStore>) {
  return [
    { name: 'publish.accepted-candidate', run: () => store.publish(PUBLISHED_HASH) },
    { name: 'command.receipt.finalize', run: () => store.finalize(PUBLISH_RECEIPT) },
  ]
}

test('interrupted: publish then receipt leaves the receipt absent', async () => {
  const store = publishStore()
  const sequence = interruptedSequence(publishWrites(store))

  await assert.rejects(
    () => sequence.crashAfter('publish.accepted-candidate'),
    InterruptedSequenceCrash,
  )

  assert.equal(store.publishedHash, PUBLISHED_HASH, 'the publish is durable')
  assert.equal(store.receipt(PUBLISH_RECEIPT), null, 'the receipt was not written')
  assert.deepEqual(sequence.committed(), ['publish.accepted-candidate'])
  assert.deepEqual(sequence.pending(), ['command.receipt.finalize'])
})

test('interrupted: publish then receipt resumes exactly once', async () => {
  const store = publishStore()
  const sequence = interruptedSequence(publishWrites(store))
  await assert.rejects(
    () => sequence.crashAfter('publish.accepted-candidate'),
    InterruptedSequenceCrash,
  )

  await sequence.resume()
  assert.equal(store.receipt(PUBLISH_RECEIPT), 'success', 'the resume writes the receipt')
  assert.equal(store.finalizeCount, 1)

  await sequence.resume()
  assert.equal(store.finalizeCount, 1, 'a second resume must not re-write the receipt')
  assert.deepEqual(sequence.pending(), [])
})

// ---------------------------------------------------------------------------
// Driver value formats: the fixture is the driver's own text, and the guard
// rejects a shape the driver never emits.
// ---------------------------------------------------------------------------

test('driver fixture: timestamptz keeps its offset', () => {
  const value = FIXTURE.values.timestamptzText
  assertDriverValueShape(value, 'timestamptz')
  assert.ok(isDriverTimestamptzText(value))
  assert.ok(value.endsWith('+00'), 'the driver attaches the offset to the text')
  assert.ok(!/[Zz]$/.test(value), 'the driver never emits a trailing Z')
  assert.ok(
    Number.isFinite(parsePostgresInstant(value)),
    'the claim reader parses the driver form to an instant',
  )
})

test('driver fixture: numeric arrives as a string', () => {
  for (const key of ['countText', 'bigintText', 'epochMillisText']) {
    const value = FIXTURE.values[key]
    assert.equal(typeof value, 'string', `${key} is a string`)
    assertDriverValueShape(value, 'numeric')
    assert.ok(isDriverNumericText(value))
  }
})

test('driver fixture: a hand-normalised value fails the guard', () => {
  const zAppended = FIXTURE.values.timestamptzText.replace(/\+00$/, 'Z')
  assert.throws(
    () => assertDriverValueShape(zAppended, 'timestamptz'),
    /Z suffix is not a form the driver emits/,
  )
  assert.throws(
    () => assertDriverValueShape(Number(FIXTURE.values.countText), 'numeric'),
    /never number/,
  )
  assert.throws(
    () => assertDriverValueShape(FIXTURE.values.timestamptzText, 'numeric'),
    /not a driver numeric::text form/,
  )
})
