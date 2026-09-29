// ---------------------------------------------------------------------------
// ENG-FORGE-PROPERTY-INVARIANTS-01 — property tests for the value families that
// have repeatedly produced real bugs.
//
// Every family runs through fast-check, so a failing case is shrunk to a minimal
// counterexample. The suite imports ONLY pure functions and opens NO database
// connection (the DB modules resolve their executor lazily), so it runs in CI
// without a database.
//
// The story notes record two real bugs, both SQL three-valued logic:
//   1. the sprint-close rule — a NULL outcome/closed_at passed the CHECK;
//   2. ENG-FORGE-SPLIT-SHAPE-01 — only a null split_assignment resolved to
//      FALSE, so a null parallel_size passed.
// `property-red-against-prefix-behaviour` reproduces each pre-fix predicate and
// asserts the SAME property FAILS on it, proving the property has power.
// ---------------------------------------------------------------------------
import { execFileSync } from 'node:child_process'
import { test } from 'node:test'
import assert from 'node:assert/strict'

import fc from 'fast-check'

import { parallelShapeRefusal } from '@/legacy/db/agent-work'
import { sprintCloseRefusal } from '@/legacy/db/sprint'
import {
  normalizeEmail,
  normalizePhone,
} from '@/lib/relationship-intel/normalize'
import { semanticPhoneKey } from '@/legacy/db/person-identities'
import {
  STATUS_BUCKET,
  STORY_STATUSES,
  statusBucket,
} from '@/lib/storyboard-data'

const BUCKETS = ['complete', 'partial', 'open', 'blocked'] as const

type GroupedInput = {
  parallelGroupId?: string | null
  parallelSlot?: number | null
  splitAssignment?: string | null
  parallelSize?: number | null
}

// ---------------------------------------------------------------------------
// FAMILY 1 — SQL null / three-valued semantics in the work-item writer.
//
// A SQL CHECK treats NULL as satisfied. The pre-fix grouped-row guard (and the
// CHECK it mirrors) therefore let a NULL `parallel_size` through; the pure guard
// must be TOTAL and refuse every NULL/blank/boundary tuple before any SQL.
// ---------------------------------------------------------------------------
test('family-work-item-null-shape', () => {
  const groupArb = fc.option(fc.string({ minLength: 1 }), { nil: null })
  const slotArb = fc.option(fc.integer({ min: -5, max: 5 }), { nil: null })
  const sizeArb = fc.option(fc.integer({ min: -5, max: 5 }), { nil: null })
  const assignmentArb = fc.oneof(
    fc.constant<string | null>(null),
    fc.constantFrom('', '   ', '\t', '\n', '\u00a0', '\u2003', 'unit-a'),
    fc.string(),
  )

  fc.assert(
    fc.property(
      groupArb,
      slotArb,
      sizeArb,
      assignmentArb,
      (parallelGroupId, parallelSlot, parallelSize, splitAssignment) => {
        const refusal = parallelShapeRefusal({
          parallelGroupId,
          parallelSlot,
          parallelSize,
          splitAssignment,
        })

        // TOTAL: only a string or null, never an undefined three-valued result.
        assert.ok(
          refusal === null || typeof refusal === 'string',
          `refusal must be string | null, got ${typeof refusal}`,
        )

        // A slot with no group is refused.
        if (parallelSlot != null && parallelGroupId == null) {
          assert.notEqual(refusal, null)
        }

        // A grouped row with a NULL/blank assignment or a NULL/non-positive size
        // is refused — this is the exact clause NULL used to slip past.
        if (parallelGroupId != null && parallelSlot != null) {
          const blankAssignment =
            splitAssignment == null || splitAssignment.trim() === ''
          const badSize =
            parallelSize == null ||
            !Number.isInteger(parallelSize) ||
            parallelSize < 1
          if (blankAssignment || badSize) assert.notEqual(refusal, null)
        }

        // A clean grouped row is admitted.
        if (
          refusal === null &&
          parallelGroupId != null &&
          parallelSlot != null
        ) {
          assert.ok(splitAssignment != null && splitAssignment.trim() !== '')
          assert.ok(
            Number.isInteger(parallelSize) && (parallelSize as number) >= 1,
          )
        }
      },
    ),
    { numRuns: 500 },
  )
})

// ---------------------------------------------------------------------------
// FAMILY 2 — SQL null / three-valued semantics in the sprint writer.
//
// The TS mirror of `storyboard_sprint_closed_says_what_happened` must never
// return null-as-pass on a NULL outcome; whitespace (including Unicode) counts
// as empty.
// ---------------------------------------------------------------------------
test('family-sprint-close-three-valued', () => {
  const statusArb = fc.constantFrom(
    'Closed',
    'Open',
    'Planned',
    'Hold',
    'Failed',
  )
  const closedAtArb = fc.oneof(
    fc.constant<string | null>(null),
    fc.constantFrom('', '   ', '\u00a0', '2026-09-18T00:00:00.000Z'),
    fc.string(),
  )
  const outcomeArb = fc.oneof(
    fc.constant<string | null>(null),
    fc.constantFrom('', ' ', '\t', '\n', '\u00a0', '\u2003', 'shipped'),
    fc.string(),
  )

  fc.assert(
    fc.property(statusArb, closedAtArb, outcomeArb, (status, closedAt, outcome) => {
      const refusal = sprintCloseRefusal({ status, closedAt, outcome })
      assert.ok(refusal === null || typeof refusal === 'string')

      if (status !== 'Closed') {
        assert.equal(refusal, null)
        return
      }

      const blankOutcome = outcome == null || outcome.trim() === ''
      if (closedAt == null || blankOutcome) assert.notEqual(refusal, null)
      if (refusal === null) {
        assert.ok(closedAt != null && !blankOutcome)
      }
    }),
    { numRuns: 500 },
  )
})

// ---------------------------------------------------------------------------
// FAMILY 3 — identifier and phone normalization.
//
// No adversarial input may yield an ok value outside the canonical NANP form;
// normalization is idempotent; the semantic phone key equates the three NANP
// spellings. The throwing crm-intake normalizer is deliberately NOT asserted
// here (a never-throws property would fail on it and mislead).
// ---------------------------------------------------------------------------
test('family-identifier-phone-normalization', () => {
  const phoneArb = fc.oneof(
    fc.constantFrom(
      '+1 (787) 555-0134',
      '7875550134',
      '+17875550134',
      '+44 20 7946 0958',
      '',
      '   ',
      'abc',
      '\u00a0',
    ),
    fc.string(),
  )

  fc.assert(
    fc.property(phoneArb, (raw) => {
      const result = normalizePhone(raw)
      if (!result.ok) {
        assert.ok(typeof result.reason === 'string' && result.reason.length > 0)
        return
      }
      assert.match(result.value, /^\+1\d{10}$/)
      const again = normalizePhone(result.value)
      assert.ok(again.ok && again.value === result.value, 'idempotent')
      const ten = result.value.slice(2)
      assert.equal(semanticPhoneKey(result.value), ten)
      assert.equal(semanticPhoneKey(`1${ten}`), ten)
      assert.equal(semanticPhoneKey(ten), ten)
    }),
    { numRuns: 500 },
  )

  fc.assert(
    fc.property(fc.string(), (raw) => {
      const result = normalizeEmail(raw)
      if (!result.ok) {
        assert.ok(typeof result.reason === 'string' && result.reason.length > 0)
        return
      }
      assert.equal(result.value, result.value.toLowerCase())
      assert.ok(result.value.length <= 320)
      assert.ok(!/\s/.test(result.value))
    }),
    { numRuns: 300 },
  )
})

// ---------------------------------------------------------------------------
// FAMILY 4 — story-state transitions.
//
// Every declared StoryStatus owns a STATUS_BUCKET entry and statusBucket is
// total over the declared vocabulary. The intentional default-to-open for an
// UNKNOWN string is not asserted here; only declared statuses are.
// ---------------------------------------------------------------------------
test('family-story-state-transition', () => {
  fc.assert(
    fc.property(fc.constantFrom(...STORY_STATUSES), (status) => {
      assert.ok(
        Object.prototype.hasOwnProperty.call(STATUS_BUCKET, status),
        `STATUS_BUCKET is missing declared status ${status}`,
      )
      const bucket = statusBucket(status)
      assert.ok((BUCKETS as readonly string[]).includes(bucket))
    }),
    { numRuns: 200 },
  )

  for (const status of STORY_STATUSES) {
    assert.ok(
      Object.prototype.hasOwnProperty.call(STATUS_BUCKET, status),
      `STATUS_BUCKET is missing declared status ${status}`,
    )
  }
})

// ---------------------------------------------------------------------------
// Shrinking — fast-check reports a minimal counterexample, which is what a
// repair lane needs. `n < 10` fails for n >= 10 and shrinks to exactly 10.
// ---------------------------------------------------------------------------
test('property-shrinks-to-minimal-counterexample', () => {
  const result = fc.check(
    fc.property(fc.integer({ min: 0, max: 1000 }), (n) => n < 10),
    { numRuns: 500, seed: 20260918 },
  )
  assert.equal(result.failed, true, 'the property must fail on some generated input')
  assert.ok(
    Array.isArray(result.counterexample),
    'fast-check must report a shrunk counterexample',
  )
  assert.equal(
    result.counterexample?.[0],
    10,
    'the counterexample must be shrunk to the minimal failing value',
  )
})

// ---------------------------------------------------------------------------
// Red against pre-fix behaviour. The two recorded bugs are reproduced as
// predicates and the SAME property is shown to fail on them, so the property
// has power and is not a green test documenting a bug.
// ---------------------------------------------------------------------------
test('property-red-against-prefix-behaviour', () => {
  // Bug 1: the sprint-close predicate before the fix let a NULL outcome pass.
  const preFixSprintClose = (sprint: {
    status: string
    closedAt: string | null
    outcome: string | null
  }): string | null =>
    sprint.status === 'Closed' && !sprint.closedAt ? 'needs closed_at' : null

  const closedSprintArb = fc.record({
    status: fc.constant('Closed'),
    closedAt: fc.constant('2026-09-18T00:00:00.000Z'),
    outcome: fc.constantFrom<string | null>(null, '', ' ', '\u00a0', 'shipped'),
  })
  const closeProperty = (
    guard: (sprint: {
      status: string
      closedAt: string | null
      outcome: string | null
    }) => string | null,
  ) =>
    fc.property(closedSprintArb, (sprint) => {
      const blank = sprint.outcome == null || sprint.outcome.trim() === ''
      if (!blank) return true
      return guard(sprint) !== null
    })

  fc.assert(closeProperty(sprintCloseRefusal), { numRuns: 100 })
  assert.throws(() => fc.assert(closeProperty(preFixSprintClose), { numRuns: 100 }))

  // Bug 2 (ENG-FORGE-SPLIT-SHAPE-01): the grouped-row guard before the fix only
  // refused a blank split_assignment, so a NULL parallel_size passed.
  const preFixParallelShape = (input: GroupedInput): string | null =>
    input.parallelGroupId != null && !input.splitAssignment?.trim()
      ? 'splitAssignment required'
      : null

  const groupedArb = fc.record({
    parallelGroupId: fc.constant('group-1'),
    parallelSlot: fc.integer({ min: 1, max: 3 }),
    splitAssignment: fc.constant('unit-a'),
    parallelSize: fc.constantFrom<number | null>(
      null,
      -1,
      0,
      2,
      3,
    ),
  })
  const groupedProperty = (guard: (input: GroupedInput) => string | null) =>
    fc.property(groupedArb, (input) => {
      const badSize =
        input.parallelSize == null ||
        !Number.isInteger(input.parallelSize) ||
        input.parallelSize < 1
      if (!badSize) return true
      return guard(input) !== null
    })

  fc.assert(groupedProperty(parallelShapeRefusal), { numRuns: 100 })
  assert.throws(() =>
    fc.assert(groupedProperty(preFixParallelShape), { numRuns: 100 }),
  )
})

// ---------------------------------------------------------------------------
// No database. The suite imports only pure functions; this proves the module
// graph loads and computes with NO DATABASE_URL set.
// ---------------------------------------------------------------------------
test('suite-runs-without-database', () => {
  const script = [
    "Promise.all([import('@/legacy/workflow_app/tests/db/agent-work.ts'),import('@/legacy/workflow_app/tests/db/sprint.ts'),",
    "import('@/legacy/workflow_app/tests/lib/relationship-intel/normalize.ts'),import('@/legacy/workflow_app/tests/db/person-identities.ts'),",
    "import('@/legacy/workflow_app/tests/lib/storyboard-data.ts')]).then(([a,s,n,p,d])=>{",
    "if(typeof a.parallelShapeRefusal!=='function'||typeof s.sprintCloseRefusal!=='function'||",
    "typeof n.normalizePhone!=='function'||typeof p.semanticPhoneKey!=='function'||",
    "typeof d.statusBucket!=='function')process.exit(3);console.log('NO_DB_OK')})",
  ].join('')
  const env: NodeJS.ProcessEnv = { ...process.env }
  delete env.DATABASE_URL
  const out = execFileSync(process.execPath, ['--import', 'tsx', '-e', script], {
    cwd: process.cwd(),
    env,
    encoding: 'utf8',
    timeout: 30_000,
  })
  assert.match(out, /NO_DB_OK/)
})
