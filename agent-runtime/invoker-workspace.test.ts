import assert from 'node:assert/strict'
import test from 'node:test'

import {
  buildAgentInvokerWorkspaces,
  resolveWorkspaceRunId,
} from './invoker'

test('serial Forge roles share one execution-scoped workspace id', () => {
  const workspaces = buildAgentInvokerWorkspaces('forge-engine-357', {
    AGENT_WORKSPACE_BASE_REF: 'origin/main',
  })

  assert.ok(workspaces)
  assert.equal(workspaces.executionId, 'forge-engine-357')
  assert.equal(resolveWorkspaceRunId(workspaces.executionId), 'forge-engine-357')
  assert.equal(resolveWorkspaceRunId(workspaces.executionId), resolveWorkspaceRunId(workspaces.executionId))
})

test('only an explicit split child id fans out a second workspace lineage', () => {
  const executionId = 'forge-engine-357'
  const serial = resolveWorkspaceRunId(executionId)
  const smithA = resolveWorkspaceRunId(executionId, 'smith-a')
  const smithB = resolveWorkspaceRunId(executionId, 'smith-b')

  assert.equal(serial, executionId)
  assert.notEqual(smithA, serial)
  assert.notEqual(smithB, serial)
  assert.notEqual(smithA, smithB)
  assert.equal(smithA, 'forge-engine-357-split-smith-a')
})
