// ---------------------------------------------------------------------------
// ENG-FORGE-OBS-SERIAL-01 — the serial lane's observer hook.
//
// These tests exercise the hook with a FAKE SINK: no OpenCode, no database, no
// engine. What they pin is the behaviour the story's acceptance names:
//   * the serial candidate records GIT_COMMIT and a SCOPE_CHECK;
//   * a HOLD is recorded with its reason text (and a retry hash when a miss list
//     produced it);
//   * alerts are RECORDED, never thrown — the runner still owns every HOLD;
//   * the durable seam stamps source_system=forge_observer and keeps one event
//     per source identity per attempt.
//
// The serial lane was dark: it produced no SCOPE_CHECK / GIT_COMMIT / HOLD at
// all, so scorecard split health was the only measured lane. These tests are what
// now fails if that goes dark again.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import { createMemoryTraceSink } from '../forge/forge-observer/sink'
import { createPersistentTraceSink, type TraceWrite } from '../forge/forge-observer/persistent-sink'
import type { TraceEvent, TraceIdentity } from '../forge/forge-observer/types'
import {
  drainAlerts,
  observeAttemptBegin,
  observeAttemptEnd,
  observeCandidateCommit,
  observeHold,
  observeRouteHold,
} from '../forge/forge-observer-seam'
import type { SmithExecutionContract } from '../forge/smith-contract'

const SHA = 'b'.repeat(40)
const ALLOWED = 'workflow_app/forge/a.ts'
const OUTSIDE = 'workflow_app/forge/b.ts'

const base: TraceIdentity = {
  storyId: 'FORGE-HOLES-1',
  processInstanceId: 'proc-1',
  taskId: 'task-1',
  nodeId: 'smith',
  attempt: 1,
  worktreePath: '/tmp/wt',
  baseCommit: 'a'.repeat(40),
}

function contract(overrides: Partial<SmithExecutionContract> = {}): SmithExecutionContract {
  return {
    identity: { storyId: base.storyId, nodeId: 'smith', attempt: 1, owner: 'assign-1' },
    objective: 'the accepted Lead assignment',
    requiredInputs: [],
    allowedScope: [ALLOWED],
    prohibitedScope: [],
    expectedOutputs: [],
    requiredEvidence: [],
    dependsOn: [],
    ...overrides,
  }
}

function recorder() {
  const writes: Parameters<TraceWrite>[0][] = []
  const write: TraceWrite = async (input) => {
    writes.push(input)
  }
  return { writes, write }
}

function kinds(events: TraceEvent[]): string[] {
  return events.map((e) => e.kind)
}

describe('ENG-FORGE-OBS-SERIAL-01 serial observer hook', () => {
  it('records a SCOPE_CHECK deny for a candidate that leaves its assignment, without throwing', () => {
    const sink = createMemoryTraceSink()
    let observed: { violations: string[]; scopeChecked: boolean } | null = null

    assert.doesNotThrow(() => {
      observed = observeCandidateCommit(sink, base, {
        candidateSha: SHA,
        changedFiles: [ALLOWED, OUTSIDE],
        contract: contract(),
      })
    })

    const events = sink.list(base.storyId)
    assert.deepEqual(kinds(events), ['git.commit', 'scope.check'])
    const scope = events[1]
    assert.equal(scope.verdict, 'deny')
    assert.deepEqual(scope.paths, [OUTSIDE], 'the denied paths are the violations')
    assert.match(String(scope.reason), /outside assignment/)
    assert.deepEqual(observed!.violations, [OUTSIDE])
  })

  it('records SCOPE_CHECK allow plus GIT_COMMIT when the candidate is inside scope', () => {
    const sink = createMemoryTraceSink()
    const observed = observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [ALLOWED],
      contract: contract(),
    })

    const events = sink.list(base.storyId)
    assert.deepEqual(kinds(events), ['git.commit', 'scope.check'])
    assert.equal(events[0].sha, SHA)
    assert.equal(events[0].verdict, 'allow')
    assert.deepEqual(events[0].paths, [ALLOWED])
    assert.equal(events[1].verdict, 'allow')
    assert.deepEqual(observed.violations, [])
    assert.equal(observed.scopeChecked, true)
  })

  it('records the commit but invents no scope verdict when the lane has no contract', () => {
    const sink = createMemoryTraceSink()
    const observed = observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [OUTSIDE],
      contract: null,
    })

    assert.deepEqual(kinds(sink.list(base.storyId)), ['git.commit'])
    assert.equal(observed.scopeChecked, false)
    assert.deepEqual(observed.violations, [], 'no declared scope means nothing to violate')
  })

  it('writes the serial hook through the durable seam as forge_observer', async () => {
    const { writes, write } = recorder()
    const sink = createPersistentTraceSink({ write, traceId: 'trace-1' })
    observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [ALLOWED, OUTSIDE],
      contract: contract(),
    })
    await Promise.resolve()

    const [commit, scope] = writes
    assert.equal(commit.eventType, 'GIT_COMMIT')
    assert.equal(commit.sourceSystem, 'forge_observer')
    assert.equal(commit.workflowNodeId, 'smith')
    assert.equal(commit.metadata.sha, SHA)
    assert.equal(scope.eventType, 'SCOPE_CHECK')
    assert.equal(scope.outcome, 'deny', 'the verdict reaches the trace as the outcome')
    assert.equal(scope.metadata.violationCount, 1)
    assert.equal(scope.sourceEventId, 'FORGE-HOLES-1:task-1:smith:1:2')
  })

  it('keeps one event per source identity per attempt on the wired path', async () => {
    const { writes, write } = recorder()
    const sink = createPersistentTraceSink({ write, traceId: 'trace-1' })
    observeAttemptBegin(sink, base, { role: 'smith', route: 'SMITH' })
    observeAttemptEnd(sink, base, { status: 'completed', sha: SHA, storyId: base.storyId })
    observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [ALLOWED],
      contract: contract(),
    })
    await Promise.resolve()

    const ids = writes.map((w) => w.sourceEventId)
    assert.equal(new Set(ids).size, ids.length, 'no double-seq collision within an attempt')
    assert.ok(ids.every((id) => id.startsWith('FORGE-HOLES-1:task-1:smith:1:')))
    assert.equal(writes[0].eventType, 'RUN_START')
    assert.equal(writes[0].metadata.route, 'SMITH', 'the chosen route rides run.start')
  })

  it('records the attempt facts for a plain lane that chose no route', () => {
    const sink = createMemoryTraceSink()
    observeAttemptBegin(sink, base, { role: 'qa', route: null })
    observeAttemptEnd(sink, base, { status: 'completed', sha: SHA, storyId: base.storyId })

    const events = sink.list(base.storyId)
    assert.deepEqual(kinds(events), ['run.start', 'run.end'])
    assert.equal(events[0].detail?.role, 'qa')
    assert.equal(events[0].detail?.route, undefined, 'no route is recorded when none was chosen')
    assert.equal(events[1].verdict, 'allow')
    assert.equal(events[1].sha, SHA)
  })

  it('a HOLD carries its reason text and a retry hash', () => {
    const sink = createMemoryTraceSink()
    const miss = ['lead-routing:missing merge checks', 'routing:lead_decision']
    observeHold(sink, base, { reasons: miss, sha: SHA, missReasons: miss })

    const hold = sink.list(base.storyId)[0]
    assert.equal(hold.kind, 'hold')
    assert.equal(hold.verdict, 'deny')
    assert.match(String(hold.reason), /lead-routing:missing merge checks/)
    assert.match(String(hold.reason), /routing:lead_decision/)
    assert.equal(hold.detail?.reasonCount, 2)
    assert.equal(typeof hold.detail?.retryHash, 'string')
  })

  it('omits the retry hash when there is no miss list (a constant hash would fake a replay)', () => {
    const sink = createMemoryTraceSink()
    observeHold(sink, base, { reasons: ['candidate left its assignment'], sha: SHA })
    assert.equal(sink.list(base.storyId)[0].detail?.retryHash, undefined)
  })

  it('a repeated held attempt surfaces RETRY_UNCHANGED_INPUT, recorded not thrown', () => {
    const sink = createMemoryTraceSink()
    const miss = ['routing:lead_decision']
    observeHold(sink, { ...base, attempt: 1 }, { reasons: miss, missReasons: miss })
    observeHold(sink, { ...base, attempt: 2 }, { reasons: miss, missReasons: miss })

    let alerts: ReturnType<typeof drainAlerts> = []
    assert.doesNotThrow(() => {
      alerts = drainAlerts(sink, { ...base, attempt: 2 }, base.storyId)
    })
    assert.deepEqual(
      alerts.map((a) => a.code),
      ['RETRY_UNCHANGED_INPUT'],
    )
    const recorded = sink.list(base.storyId).filter((e) => e.kind === 'alert')
    assert.equal(recorded.length, 1)
    assert.match(String(recorded[0].reason), /RETRY_UNCHANGED_INPUT/)
  })

  it('a hold-recommend alert is recorded, never thrown (alerts are not the gate)', () => {
    const sink = createMemoryTraceSink()
    observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [OUTSIDE],
      contract: contract(),
    })

    let alerts: ReturnType<typeof drainAlerts> = []
    assert.doesNotThrow(() => {
      alerts = drainAlerts(sink, base, base.storyId)
    })
    const boundary = alerts.find((a) => a.code === 'SCOPE_DENIED')
    assert.ok(boundary, 'the out-of-lane candidate is surfaced')
    assert.equal(boundary.severity, 'hold-recommend')
    assert.equal(typeof boundary.reason, 'string', 'the reason text is carried, not just the code')
  })

  it('drains each alert once instead of re-recording it on every drain', () => {
    const sink = createMemoryTraceSink()
    observeCandidateCommit(sink, base, {
      candidateSha: SHA,
      changedFiles: [OUTSIDE],
      contract: contract(),
    })
    drainAlerts(sink, base, base.storyId)
    drainAlerts(sink, base, base.storyId)
    drainAlerts(sink, base, base.storyId)

    const alerts = sink.list(base.storyId).filter((e) => e.kind === 'alert')
    assert.equal(alerts.length, 1, 'one alert, not one per drain')
  })

  it('a Lead PRE HOLD decision leaves a HOLD event with its reason text', () => {
    const sink = createMemoryTraceSink()
    observeRouteHold(sink, base, 'LARGE work cannot dispatch while SPLIT is unavailable')

    const hold = sink.list(base.storyId)[0]
    assert.equal(hold.kind, 'hold')
    assert.equal(hold.verdict, 'deny')
    assert.match(String(hold.reason), /route:HOLD/)
    assert.match(String(hold.reason), /SPLIT is unavailable/)
  })

  it('refuses a 0-based attempt rather than writing an unattributable event', () => {
    const sink = createMemoryTraceSink()
    assert.throws(() => observeAttemptBegin(sink, { ...base, attempt: 0 }, { role: 'smith' }))
  })
})
