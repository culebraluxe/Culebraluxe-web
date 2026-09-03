import assert from 'node:assert/strict'
import test from 'node:test'

import { openClawProvider } from './openclaw-provider'
import {
  resolveForgeExecutionProvider,
  resolveForgeExecutionProviderForProfile,
} from './provider'
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

test('profile provider overrides the global provider', () => {
  assert.equal(
    resolveForgeExecutionProviderForProfile('builder-flash', {
      FORGE_EXECUTION_PROVIDER: 'deepseek',
      FORGE_PROVIDER_BUILDER_FLASH: 'warp',
    }),
    'warp',
  )
})

test('different Forge profiles can route to different providers', () => {
  const env = {
    FORGE_PROVIDER_SCOUT_VOLUME: 'deepseek',
    FORGE_PROVIDER_BUILDER_FLASH: 'warp',
    FORGE_PROVIDER_VERIFIER_MINI: 'openclaw',
  }
  assert.equal(resolveForgeExecutionProviderForProfile('scout-volume', env), 'deepseek')
  assert.equal(resolveForgeExecutionProviderForProfile('builder-flash', env), 'warp')
  assert.equal(resolveForgeExecutionProviderForProfile('verifier-mini', env), 'openclaw')
})

test('profile routing falls back to the global provider', () => {
  assert.equal(
    resolveForgeExecutionProviderForProfile('scout-volume', {
      FORGE_EXECUTION_PROVIDER: 'openclaw',
    }),
    'openclaw',
  )
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
