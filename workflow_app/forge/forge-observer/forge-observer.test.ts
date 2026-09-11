import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { createMemoryTraceSink } from './sink'
import { recordGitCommit, recordRunStart, recordScopeCheck, retryInputHash } from './record'
import type { TraceIdentity } from './types'

const base: TraceIdentity = {
  storyId: 'STORY-1',
  processInstanceId: 'proc-1',
  taskId: 'task-1',
  nodeId: 'smith_split_work',
  attempt: 1,
  worktreePath: '/tmp/wt',
  baseCommit: 'a'.repeat(40),
}

describe('forge-observer', () => {
  it('assigns monotonic seq and strips #symbol to file paths', () => {
    const sink = createMemoryTraceSink()
    recordRunStart(sink, base)
    const commit = recordGitCommit(sink, base, {
      sha: 'b'.repeat(40),
      paths: ['ui/foo.ts#Bar', 'ui/foo.ts#Baz', 'ui/foo.ts'],
      changed: true,
    })
    assert.equal(commit.seq, 2)
    assert.deepEqual(commit.paths, ['ui/foo.ts'])
  })

  it('records scope deny with violation paths', () => {
    const sink = createMemoryTraceSink()
    const ev = recordScopeCheck(sink, base, {
      sha: 'c'.repeat(40),
      paths: ['a.ts', 'secret.ts'],
      violations: ['secret.ts'],
    })
    assert.equal(ev.verdict, 'deny')
    assert.deepEqual(ev.paths, ['secret.ts'])
  })

  it('refuses 0-based attempt', () => {
    const sink = createMemoryTraceSink()
    assert.throws(() => recordRunStart(sink, { ...base, attempt: 0 }))
  })

  it('retryInputHash is order-insensitive on misses', () => {
    const a = retryInputHash({ missReasons: ['b', 'a'], packetHash: 'p', assignmentId: 'x' })
    const b = retryInputHash({ missReasons: ['a', 'b'], packetHash: 'p', assignmentId: 'x' })
    assert.equal(a, b)
  })
})
