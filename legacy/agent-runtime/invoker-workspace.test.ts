import assert from 'node:assert/strict'
import test from 'node:test'

import {
  buildAgentInvokerWorkspaces,
  forgeExecutionGenerationKey,
  resolveForgeExecutionRunId,
  resolveWorkspaceRunId,
} from './invoker'

test('NO TREES: no lane is ever handed a workspace (2026-09-16)', () => {
  // These tests used to assert the workspace `buildAgentInvokerWorkspaces` handed out — whose root
  // defaulted to `../Culebraluxe-worktrees`, so a tree reappeared the moment a story started (83
  // worktrees, and another at c1f37086 even after the estate was deleted). The builder now returns
  // nothing: no lane provisions a tree, every lane runs in the working directory it was given, and its
  // output is its ROW. The lineage helpers in this file still define the canonical execution key; what
  // changed is that nothing on disk is keyed by it any more.
  assert.equal(
    buildAgentInvokerWorkspaces('forge-engine-357', {
      ...process.env,
      AGENT_WORKSPACE_BASE_REF: 'origin/main',
    }),
    undefined,
  )
  // An explicit execution id changes nothing: there is no workspace to name, and no env opt-in brings
  // one back (the old AGENT_WORKSPACE_DISABLED escape hatch is gone with the machinery it disabled).
  assert.equal(buildAgentInvokerWorkspaces('forge-engine-357', { ...process.env }, `${PID}-e1`), undefined)
  assert.equal(
    buildAgentInvokerWorkspaces('forge-engine-357', { ...process.env, AGENT_WORKSPACE_DISABLED: '1' }),
    undefined,
  )
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

// ENG-FORGE-WORKSPACE-01 — canonical lineage = processInstanceId + generation.
const PID = '7a2c9d'

test('WORKSPACE Case E: normal serial FEATURE -> ONE lineage for the whole serial lifecycle', () => {
  // All serial roles (architect / lead / smith / repair smith / QA) reuse the
  // same processInstanceId + generation, regardless of worker/agent-work-item id.
  const architect = resolveForgeExecutionRunId(PID, 0)
  const smith = resolveForgeExecutionRunId(PID, 0)
  assert.equal(forgeExecutionGenerationKey(PID, 0), `${PID}-e0`)
  assert.equal(smith, architect)
  // workerId does NOT shape lineage: two different workers still collide to the
  // same serial workspace.
  assert.equal(resolveForgeExecutionRunId(PID, 0), resolveForgeExecutionRunId(PID, 0))
  assert.equal(smith, `${PID}-e0`)
})

test('WORKSPACE Case F: explicit SPLIT(3) -> three bounded child workspaces only where requested', () => {
  const serial = resolveForgeExecutionRunId(PID, 0)
  const c0 = resolveForgeExecutionRunId(PID, 0, '0')
  const c1 = resolveForgeExecutionRunId(PID, 0, '1')
  const c2 = resolveForgeExecutionRunId(PID, 0, '2')
  assert.equal(serial, `${PID}-e0`)
  assert.equal(c0, `${PID}-e0-split-0`)
  assert.equal(c1, `${PID}-e0-split-1`)
  assert.equal(c2, `${PID}-e0-split-2`)
  // Children are pairwise distinct and each fans out from the generation.
  assert.equal(new Set([c0, c1, c2]).size, 3)
})

test('WORKSPACE Case G: repair Smith reuses the execution-generation workspace', () => {
  // Repair is an implementation correction against the SAME generation: no new
  // workspace. The generation key (not the role/attempt) decides lineage.
  const smith = resolveForgeExecutionRunId(PID, 0)
  const repair = resolveForgeExecutionRunId(PID, 0)
  assert.equal(repair, smith)
  assert.equal(repair, `${PID}-e0`)
})

test('WORKSPACE Case H: REPLAN advances the generation and gets a new workspace', () => {
  const e0 = resolveForgeExecutionRunId(PID, 0)
  const e1 = resolveForgeExecutionRunId(PID, 1)
  assert.equal(e0, `${PID}-e0`)
  assert.equal(e1, `${PID}-e1`)
  assert.notEqual(e1, e0)
  assert.equal(forgeExecutionGenerationKey(PID, 1), `${PID}-e1`)
})

test('WORKSPACE: the execution key is still resolved, and still ignores workerId', () => {
  // The execution id is still computed at the runner (`resolveForgeExecutionRunId`, above) and passed
  // into `buildAgentInvokerWorkspaces`, which now ignores it — there is no workspace to key. What this
  // pins is the value itself, so the lineage stays answerable from the ledger even though no directory
  // is named after it, and that the worker id remains the executor/claim identity, never the lineage.
  const override = resolveForgeExecutionRunId(PID, 1)
  assert.equal(override, `${PID}-e1`)
  assert.equal(buildAgentInvokerWorkspaces('forge-engine-357', { ...process.env }, override), undefined)
  assert.equal(
    resolveForgeExecutionRunId(PID, 1),
    resolveForgeExecutionRunId(PID, 1),
    'two different workers still collide to the same serial key',
  )
})