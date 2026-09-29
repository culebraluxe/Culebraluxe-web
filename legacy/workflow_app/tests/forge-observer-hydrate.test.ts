// ---------------------------------------------------------------------------
// FORGE-OBS-LIST-01 — the observer must be able to see an attempt it did not make.
//
// Grok, 91 review, against the code before this: "the retry hash cannot see a
// restarted attempt." That was true, and it is worse than it sounds — the same
// blind sink is what drainAlerts de-dupes against, so a restart also made every
// alert look brand new (phantom alerts), not just RETRY_UNCHANGED_INPUT.
//
// The mechanism: persistent-sink.ts writes every event durably but keeps list() in
// process memory. These tests rehydrate that memory from the trace rows a previous
// process wrote, with a FAKE reader — no database.
//
// The acceptance case is the restart itself: attempt 1 is in the trace, a NEW
// process loads it, attempt 2 repeats its input, and the alert fires naming both
// attempts. The same test asserts the OLD behaviour (no load) does NOT fire it, so
// this cannot pass by accident.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import {
  createPersistentTraceSink,
  type TraceRead,
  type TraceWrite,
} from '@/legacy/workflow_app/forge/forge-observer/persistent-sink'
import {
  kindOf,
  traceEventsFromRows,
  type PersistedTraceRow,
} from '@/legacy/workflow_app/forge/forge-observer/rehydrate'
import { createMemoryTraceSink } from '@/legacy/workflow_app/forge/forge-observer/sink'
import { retryInputHash } from '@/legacy/workflow_app/forge/forge-observer/record'
import type { TraceEvent, TraceIdentity } from '@/legacy/workflow_app/forge/forge-observer/types'
import { drainAlerts, observeHold } from '@/legacy/workflow_app/forge/forge-observer-seam'

const STORY = 'FORGE-OBS-LIST-01'
const PROC = 'proc-restart'
const TASK = 'task-1'
const NODE = 'smith'

const base: TraceIdentity = {
  storyId: STORY,
  processInstanceId: PROC,
  taskId: TASK,
  nodeId: NODE,
  attempt: 2,
  baseCommit: 'a'.repeat(40),
}

const noWrite: TraceWrite = async () => {}

/** One persisted row as the write path leaves it: dotted kind -> UPPER_SNAKE. */
function row(over: Partial<PersistedTraceRow> & { eventType: string }): PersistedTraceRow {
  return {
    occurredAt: '2026-09-12T10:00:00.000Z',
    workflowInstanceId: PROC,
    workflowNodeId: NODE,
    taskId: TASK,
    metadata: { attempt: 1, storyId: STORY },
    ...over,
  }
}

describe('rehydration: trace rows back into observer events', () => {
  it('inverts the write path: kind, verdict, paths, detail, sha', () => {
    const rows: PersistedTraceRow[] = [
      row({ eventType: 'RUN_START', sourceEventId: `${STORY}:${TASK}:${NODE}:1:1` }),
      row({
        eventType: 'SCOPE_CHECK',
        outcome: 'deny',
        sourceEventId: `${STORY}:${TASK}:${NODE}:1:4`,
        summary: 'candidate touched files outside assignment: x.ts',
        metadata: {
          attempt: 1,
          storyId: STORY,
          sha: 'c'.repeat(40),
          paths: 'x.ts,y.ts',
          violationCount: 2,
        },
      }),
    ]
    const events = traceEventsFromRows(rows, STORY)
    assert.equal(events.length, 2)
    assert.equal(events[0].kind, 'run.start')
    assert.equal(events[1].kind, 'scope.check')
    assert.equal(events[1].verdict, 'deny')
    assert.deepEqual(events[1].paths, ['x.ts', 'y.ts'])
    assert.equal(events[1].sha, 'c'.repeat(40))
    // Reserved keys are lifted out; everything else survives into detail.
    assert.deepEqual(events[1].detail, { violationCount: 2 })
    assert.equal(events[1].reason, 'candidate touched files outside assignment: x.ts')
  })

  it('recovers seq and attempt from sourceEventId, so a rehydrated event keeps its identity', () => {
    const [event] = traceEventsFromRows(
      [row({ eventType: 'HOLD', sourceEventId: `${STORY}:${TASK}:${NODE}:3:17` })],
      STORY,
    )
    assert.equal(event.seq, 17)
    assert.equal(event.attempt, 3)
  })

  it('skips unknown kinds and other stories instead of inventing a kind for them', () => {
    const rows: PersistedTraceRow[] = [
      row({ eventType: 'COMMAND_RECEIVED', sourceEventId: 'x:i:n:1:1' }),
      row({ eventType: 'RUN_END', metadata: { attempt: 1, storyId: 'SOMEONE-ELSE' } }),
      row({ eventType: 'RUN_END', sourceEventId: `${STORY}:${TASK}:${NODE}:1:2` }),
    ]
    const events = traceEventsFromRows(rows, STORY)
    assert.equal(events.length, 1)
    assert.equal(events[0].kind, 'run.end')
  })

  it('kindOf maps the persisted spelling back to the dotted kind', () => {
    assert.equal(kindOf('SCOPE_CHECK'), 'scope.check')
    assert.equal(kindOf('GIT_COMMIT'), 'git.commit')
    assert.equal(kindOf('NOT_A_KIND'), null)
    assert.equal(kindOf(null), null)
  })
describe('the restart the observer used to be blind to', () => {
  /** The miss list attempt 2 repeats verbatim — hash computed, never hand-written. */
  const MISSES = ['smith-scope:x.ts is outside assign-7']
  const MISS_HASH = retryInputHash({ missReasons: MISSES })

  /** Attempt 1 held on the same miss list attempt 2 is about to repeat. */
  const priorHold = row({
    eventType: 'HOLD',
    outcome: 'deny',
    sourceEventId: `${STORY}:${TASK}:${NODE}:1:5`,
    summary: 'smith-scope:x.ts is outside assign-7',
    metadata: {
      attempt: 1,
      storyId: STORY,
      retryHash: MISS_HASH,
      reasonCount: 1,
    },
  })

  const reader: TraceRead = async () => [priorHold]

  it('WITHOUT load, attempt 2 repeating attempt 1 is invisible (the old behaviour)', () => {
    const sink = createPersistentTraceSink({ write: noWrite, read: reader })
    observeHold(sink, base, { reasons: MISSES, missReasons: MISSES })
    const alerts = drainAlerts(sink, base, STORY)
    assert.equal(
      alerts.some((a) => a.code === 'RETRY_UNCHANGED_INPUT'),
      false,
      'the old blind sink cannot see a prior attempt — this is the bug being fixed',
    )
  })

  it('WITH load, attempt 2 repeating attempt 1 fires RETRY_UNCHANGED_INPUT naming both', async () => {
    const sink = createPersistentTraceSink({ write: noWrite, read: reader })
    const loaded = await sink.load({ storyId: STORY, processInstanceId: PROC })
    assert.equal(loaded, 1, 'the prior attempt is taken into this process')

    observeHold(sink, base, { reasons: MISSES, missReasons: MISSES })
    const alerts = drainAlerts(sink, base, STORY)
    const retry = alerts.find((a) => a.code === 'RETRY_UNCHANGED_INPUT')
    assert.ok(retry, 'the repeated no-op retry must now be visible')
    assert.match(retry.reason, /attempt 2 repeated retry hash .* from attempt 1/)
    assert.equal(retry.severity, 'hold-recommend')
  })

  it('a restarted process does not re-use sequence numbers already written', async () => {
    const sink = createPersistentTraceSink({ write: noWrite, read: reader })
    await sink.load({ storyId: STORY, processInstanceId: PROC })
    observeHold(sink, base, { reasons: MISSES, missReasons: MISSES })
    const events = sink.list(STORY)
    assert.equal(events.length, 2, 'the hydrated attempt plus the new one')
    const newest = events[events.length - 1]
    assert.ok(
      newest.seq > 5,
      `new events must continue past the hydrated history (got seq ${newest.seq}), otherwise the ` +
        'trace table de-dupe key collides with a write this process did not make',
    )
  })

  it('an alert already in the hydrated history is not re-recorded as new', async () => {
    const priorAlert = row({
      eventType: 'ALERT',
      outcome: 'watch',
      sourceEventId: `${STORY}:${TASK}:${NODE}:1:9`,
      summary: 'RETRY_UNCHANGED_INPUT: attempt 1 repeated retry hash h from attempt 1',
      metadata: { attempt: 1, storyId: STORY, code: 'RETRY_UNCHANGED_INPUT', severity: 'watch' },
    })
    const sink = createPersistentTraceSink({
      write: noWrite,
      read: async () => [priorHold, priorAlert],
    })
    await sink.load({ storyId: STORY, processInstanceId: PROC })
    const before = sink.list(STORY).filter((e) => e.kind === 'alert').length

    observeHold(sink, base, { reasons: MISSES, missReasons: MISSES })
    drainAlerts(sink, base, STORY)
    const after = sink.list(STORY).filter((e) => e.kind === 'alert').length
    // The duplicate-code alert is de-duped against the hydrated history.
    assert.ok(after - before <= 1, 'hydrated alerts must count as already recorded')
  })
})

describe('the sink stays an observer', () => {
  it('load without a reader is a no-op, not a crash', async () => {
    const sink = createPersistentTraceSink({ write: noWrite })
    assert.equal(await sink.load({ storyId: STORY, processInstanceId: PROC }), 0)
    assert.deepEqual(sink.list(STORY), [])
  })

  it('a failing reader leaves the sink exactly as it was and never throws', async () => {
    const sink = createPersistentTraceSink({
      write: noWrite,
      read: async () => {
        throw new Error('trace table unreachable')
      },
    })
    assert.equal(await sink.load({ storyId: STORY, processInstanceId: PROC }), 0)
    // Still usable, still recording: a reader outage must not cost the live trace.
    const event = sink.append({ ...base, kind: 'run.start' })
    assert.equal(event.kind, 'run.start')
  })

  it('loads a story once per process', async () => {
    let reads = 0
    const sink = createPersistentTraceSink({
      write: noWrite,
      read: async () => {
        reads += 1
        return [row({ eventType: 'RUN_START', sourceEventId: `${STORY}:${TASK}:${NODE}:1:1` })]
      },
    })
    await sink.load({ storyId: STORY, processInstanceId: PROC })
    assert.equal(await sink.load({ storyId: STORY, processInstanceId: PROC }), 0)
    assert.equal(reads, 1, 'a second load would duplicate the history')
    assert.equal(sink.list(STORY).length, 1)
  })

  it('the memory sink seed never moves its counter backwards', () => {
    const memory = createMemoryTraceSink()
    const seeded = (seq: number): TraceEvent => ({
      ...base,
      attempt: 1,
      seq,
      atIso: '2026-09-12T09:00:00.000Z',
      kind: 'run.start',
    })
    memory.seed([seeded(3), seeded(9)])
    const event = memory.append({ ...base, kind: 'run.start' })
    assert.equal(event.seq, 10)
  })
})

})
