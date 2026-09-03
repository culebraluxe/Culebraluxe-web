import assert from 'node:assert/strict'
import test from 'node:test'

import { openClawProvider } from './openclaw-provider'
import { resolveForgeExecutionProvider } from './provider'
import { warpProvider } from './warp-provider'

test('gateway defaults to DeepSeek', () => {
  assert.equal(resolveForgeExecutionProvider(undefined), 'deepseek')
})

test('gateway accepts Warp and OpenClaw explicitly', () => {
  assert.equal(resolveForgeExecutionProvider('warp'), 'warp')
  assert.equal(resolveForgeExecutionProvider('openclaw'), 'openclaw')
})

test('gateway fails closed on unknown providers', () => {
  assert.throws(() => resolveForgeExecutionProvider('mystery'), /unknown FORGE_EXECUTION_PROVIDER/)
})

test('Warp provider builds an Oz local agent command in the worker cwd', () => {
  const command = warpProvider.buildCommand({
    cwd: '/tmp/worktree',
    task: 'Implement story',
    modelProfile: 'builder-flash',
  })
  assert.equal(command.bin, process.env.WARP_AGENT_BIN ?? 'oz')
  assert.deepEqual(command.args, ['agent', 'run', '--cwd', '/tmp/worktree', 'Implement story'])
})

test('OpenClaw provider builds isolated headless agent exec command', () => {
  const command = openClawProvider.buildCommand({
    cwd: '/tmp/worktree',
    task: 'Verify story',
    modelProfile: 'verifier-mini',
  })
  assert.equal(command.bin, process.env.OPENCLAW_BIN ?? 'openclaw')
  assert.deepEqual(command.args, ['agent', 'exec', 'Verify story', '--cwd', '/tmp/worktree', '--json'])
})
