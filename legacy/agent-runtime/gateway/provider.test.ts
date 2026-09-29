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

test('gateway accepts Warp, OpenClaw and OpenCode explicitly', () => {
  assert.equal(resolveForgeExecutionProvider('warp'), 'warp')
  assert.equal(resolveForgeExecutionProvider('openclaw'), 'openclaw')
  assert.equal(resolveForgeExecutionProvider('opencode'), 'opencode')
})

test('gateway fails closed on unknown providers', () => {
  assert.throws(() => resolveForgeExecutionProvider('mystery'), /unknown FORGE_EXECUTION_PROVIDER/)
})

test('profile helper has no hidden role-specific default routing', () => {
  assert.equal(resolveForgeExecutionProviderForProfile('builder-flash', {}), 'deepseek')
  assert.equal(resolveForgeExecutionProviderForProfile('lead-pro', {}), 'deepseek')
  assert.equal(resolveForgeExecutionProviderForProfile('verifier-mini', {}), 'deepseek')
})

test('explicit profile override remains available for gateway experiments', () => {
  assert.equal(
    resolveForgeExecutionProviderForProfile('builder-flash', {
      FORGE_PROVIDER_BUILDER_FLASH: 'opencode',
    }),
    'opencode',
  )
})

test('explicit global override remains available', () => {
  assert.equal(
    resolveForgeExecutionProviderForProfile('builder-flash', {
      FORGE_EXECUTION_PROVIDER: 'openclaw',
    }),
    'openclaw',
  )
})

test('profile gateway override outranks global gateway override', () => {
  assert.equal(
    resolveForgeExecutionProviderForProfile('builder-flash', {
      FORGE_EXECUTION_PROVIDER: 'deepseek',
      FORGE_PROVIDER_BUILDER_FLASH: 'warp',
    }),
    'warp',
  )
})

test('Warp provider fails closed without a headless wrapper', () => {
  const previous = process.env.WARP_HEADLESS_BIN
  delete process.env.WARP_HEADLESS_BIN
  try {
    assert.throws(
      () =>
        warpProvider.buildCommand({
          cwd: '/tmp/worktree',
          task: 'Implement story',
          modelProfile: 'builder-flash',
        }),
      /WARP_HEADLESS_BIN/,
    )
  } finally {
    if (previous === undefined) delete process.env.WARP_HEADLESS_BIN
    else process.env.WARP_HEADLESS_BIN = previous
  }
})

test('Warp provider uses the explicit Forge headless wrapper contract', () => {
  const previous = process.env.WARP_HEADLESS_BIN
  process.env.WARP_HEADLESS_BIN = '/usr/local/bin/forge-warp-headless'
  try {
    const command = warpProvider.buildCommand({
      cwd: '/tmp/worktree',
      task: 'Implement story',
      modelProfile: 'builder-flash',
    })
    assert.equal(command.bin, '/usr/local/bin/forge-warp-headless')
    assert.deepEqual(command.args, ['--cwd', '/tmp/worktree', '--task', 'Implement story'])
  } finally {
    if (previous === undefined) delete process.env.WARP_HEADLESS_BIN
    else process.env.WARP_HEADLESS_BIN = previous
  }
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
