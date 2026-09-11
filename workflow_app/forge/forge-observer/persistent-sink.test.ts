import test from 'node:test'
import assert from 'node:assert/strict'

import { createPersistentTraceSink, type TraceWrite } from './persistent-sink'

const base = {
  storyId: 'ENG-OBS-1',
  processInstanceId: 'pi-1',
  taskId: 'task-1',
  nodeId: 'smith',
  attempt: 1,
  worktreePath: '/tmp/wt',
  baseCommit: 'origin/main',
}

function recorder() {
  const writes: Parameters<TraceWrite>[0][] = []
  const write: TraceWrite = async (input) => {
    writes.push(input)
  }
  return { writes, write }
}

test('persistent sink keeps the synchronous contract readable', () => {
  const { write } = recorder()
  const sink = createPersistentTraceSink({ write, traceId: 'trace-1' })
  sink.append({ ...base, kind: 'run.start' })
  // list() must stay synchronous: the TraceSink contract is sync.
  assert.equal(sink.list('ENG-OBS-1').length, 1)
  assert.equal(sink.snapshot('ENG-OBS-1')[0]?.kind, 'run.start')
})

test('persistent sink writes the worker-execution layer to the existing trace', async () => {
  const { writes, write } = recorder()
  const sink = createPersistentTraceSink({ write, traceId: 'trace-1' })
  sink.append({
    ...base,
    kind: 'scope.check',
    sha: 'abcdef1234567890',
    paths: ['lib/a.ts', 'lib/b.ts'],
    verdict: 'deny',
    reason: 'outside assignment',
    detail: { violationCount: 2 },
  })
  await Promise.resolve()

  const written = writes[0]
  assert.ok(written, 'the event reached the durable seam')
  assert.equal(written.eventType, 'SCOPE_CHECK')
  assert.equal(written.sourceSystem, 'forge_observer')
  assert.equal(written.traceId, 'trace-1')
  assert.equal(written.workflowInstanceId, 'pi-1')
  assert.equal(written.workflowNodeId, 'smith')
  assert.equal(written.outcome, 'deny')
  assert.equal(written.summary, 'outside assignment')
  assert.equal(written.metadata.sha, 'abcdef1234567890')
  assert.equal(written.metadata.paths, 'lib/a.ts,lib/b.ts')
  assert.equal(written.metadata.violationCount, 2)
})

test('source identity is deterministic so the trace de-dupes replays', async () => {
  const { writes, write } = recorder()
  const first = createPersistentTraceSink({ write, traceId: 't' })
  first.append({ ...base, kind: 'run.start' })
  await Promise.resolve()

  const second = createPersistentTraceSink({ write, traceId: 't' })
  second.append({ ...base, kind: 'run.start' })
  await Promise.resolve()

  // Same story/task/node/attempt/seq => same source_event_id => replay-safe.
  assert.equal(writes[0].sourceEventId, writes[1].sourceEventId)
  assert.equal(writes[0].sourceEventId, 'ENG-OBS-1:task-1:smith:1:1')
})

test('a failing write never fails the run (observer only)', async () => {
  const sink = createPersistentTraceSink({
    write: async () => {
      throw new Error('db down')
    },
  })
  assert.doesNotThrow(() => sink.append({ ...base, kind: 'run.start' }))
  await new Promise((resolve) => setTimeout(resolve, 0))
  assert.equal(sink.list('ENG-OBS-1').length, 1, 'the local view still has the event')
})
