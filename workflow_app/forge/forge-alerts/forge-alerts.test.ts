import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { createMemoryTraceSink } from '../forge-observer/sink'
import { recordGitCommit, recordHold, recordRunEnd, recordScopeCheck } from '../forge-observer/record'
import type { TraceIdentity } from '../forge-observer/types'
import { evaluateAlerts, holdRecommendations } from './evaluate'
import { defaultAlertRules } from './rules'

const base = (over: Partial<TraceIdentity> = {}): TraceIdentity => ({
  storyId: 'STORY-1',
  processInstanceId: 'proc-1',
  taskId: 'task-1',
  nodeId: 'smith_split_work',
  attempt: 1,
  worktreePath: '/tmp/wt',
  baseCommit: 'a'.repeat(40),
  ...over,
})

describe('forge-alerts', () => {
  it('SCOPE_DENIED on violation', () => {
    const sink = createMemoryTraceSink()
    recordScopeCheck(sink, base(), {
      sha: 'c'.repeat(40),
      paths: ['ok.ts', 'bad.ts'],
      violations: ['bad.ts'],
    })
    const alerts = evaluateAlerts(sink.snapshot('STORY-1'), defaultAlertRules())
    assert.equal(alerts.some((a) => a.code === 'SCOPE_DENIED'), true)
    assert.equal(holdRecommendations(alerts).length, 1)
  })

  it('SIBLING_FILE_COLLISION when two nodes commit the same file', () => {
    const sink = createMemoryTraceSink()
    recordGitCommit(sink, base({ taskId: 't0', nodeId: 'smith_split_work' }), {
      sha: '1'.repeat(40),
      paths: ['shared.ts'],
      changed: true,
    })
    recordGitCommit(sink, base({ taskId: 't1', nodeId: 'smith_split_work' }), {
      sha: '2'.repeat(40),
      paths: ['shared.ts'],
      changed: true,
    })
    // same nodeId — distinguish by taskId
    const alerts = evaluateAlerts(sink.snapshot('STORY-1'))
    assert.equal(alerts.some((a) => a.code === 'SIBLING_FILE_COLLISION'), true)
  })

  it('RETRY_UNCHANGED_INPUT on repeated retryHash', () => {
    const sink = createMemoryTraceSink()
    recordHold(sink, base({ attempt: 1 }), {
      reasons: ['lead-plan:missing'],
    })
    // inject hash via a second hold with detail — recordHold does not set retryHash;
    // simulate runner attaching it
    sink.append({
      ...base({ attempt: 2 }),
      kind: 'hold',
      verdict: 'deny',
      reason: 'lead-plan:missing',
      detail: { retryHash: 'same', reasonCount: 1 },
    })
    sink.append({
      ...base({ attempt: 3 }),
      kind: 'hold',
      verdict: 'deny',
      reason: 'lead-plan:missing',
      detail: { retryHash: 'same', reasonCount: 1 },
    })
    const alerts = evaluateAlerts(sink.snapshot('STORY-1'))
    assert.equal(alerts.some((a) => a.code === 'RETRY_UNCHANGED_INPUT'), true)
  })

  it('MISSING_DEPLOY_RECEIPT is watch not hold', () => {
    const sink = createMemoryTraceSink()
    recordRunEnd(sink, base({ nodeId: 'dev_ops' }), { status: 'completed' })
    sink.append({
      ...base({ nodeId: 'dev_ops' }),
      kind: 'run.end',
      verdict: 'allow',
      detail: { status: 'completed', hasReleaseEvidence: false },
    })
    const alerts = evaluateAlerts(sink.snapshot('STORY-1'))
    const receipt = alerts.find((a) => a.code === 'MISSING_DEPLOY_RECEIPT')
    assert.ok(receipt)
    assert.equal(receipt.severity, 'watch')
  })
})
