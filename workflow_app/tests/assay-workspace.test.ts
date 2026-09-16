import assert from 'node:assert/strict'
import test from 'node:test'
import {
  ASSAY_NO_WORKTREE,
  assayWorkspaceRefusal,
  isAssayNode,
} from '../forge/assay-workspace'

test('only QA nodes are assay nodes', () => {
  assert.equal(isAssayNode('qa_verify'), true)
  assert.equal(isAssayNode('fast_qa_verify'), true)
  assert.equal(isAssayNode('smith'), false)
  assert.equal(isAssayNode('lead_pre'), false)
})

test('assay in the operator checkout is refused', () => {
  const cwd = '/Users/captain/Documents/Culebraluxe-web'
  assert.equal(
    assayWorkspaceRefusal({ nodeId: 'qa_verify', roleCwd: cwd, operatorCwd: cwd }),
    ASSAY_NO_WORKTREE,
  )
})

test('assay in a worktree is allowed (pin happens separately)', () => {
  assert.equal(
    assayWorkspaceRefusal({
      nodeId: 'qa_verify',
      roleCwd: '/Users/captain/Documents/Culebraluxe-worktrees/story-e0',
      operatorCwd: '/Users/captain/Documents/Culebraluxe-web',
    }),
    null,
  )
})

test('non-assay nodes may run in cwd — this refusal is not a second brain', () => {
  const cwd = '/repo'
  assert.equal(assayWorkspaceRefusal({ nodeId: 'smith', roleCwd: cwd, operatorCwd: cwd }), null)
})
