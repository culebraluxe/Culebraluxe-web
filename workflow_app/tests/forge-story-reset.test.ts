// ---------------------------------------------------------------------------
// ENG-FORGE-V14 — reset safety: pure argv/env resolution + the PROD --force
// gate. resolveStoryResetConfig is exercised with NO DB and NO child process:
// the resolver is side-effect free by contract. Cases:
//   - prod without --force refuses (reset AND recover)
//   - prod with --force ok
//   - dev default and dev explicit stay force-free
//   - unknown mode/target and missing url error
//   - --force is position-independent
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  resolveStoryResetConfig,
  type ForgeStoryResetConfig,
} from '../../scripts/forge-story-reset-config'

function resolve(
  tail: string[],
  env: NodeJS.ProcessEnv = {},
): ForgeStoryResetConfig {
  // argv shape mirrors `pnpm forge:story:reset` (node + script consumed first).
  return resolveStoryResetConfig(['node', 'scripts/forge-story-reset.ts', ...tail], env)
}

const ENV = { DATABASE_URL_PROD: 'postgres://prod', DATABASE_URL_DEV: 'postgres://dev' }

test('reset safety: prod without --force refuses', () => {
  const cfg = resolve(['story-1', 'reset', 'prod'], ENV)
  assert.equal(cfg.ok, false)
  assert.ok('error' in cfg && /--force/.test(cfg.error), `error should name --force: ${cfg.error}`)
})

test('reset safety: recover prod is gated identically', () => {
  const cfg = resolve(['story-1', 'recover', 'prod'], ENV)
  assert.equal(cfg.ok, false)
  assert.ok('error' in cfg && /--force/.test(cfg.error))
})

test('reset safety: prod with --force resolves ok', () => {
  const cfg = resolve(['story-1', 'reset', 'prod', '--force'], ENV)
  assert.equal(cfg.ok, true)
  if (!cfg.ok) return
  assert.equal(cfg.story, 'story-1')
  assert.equal(cfg.mode, 'reset')
  assert.equal(cfg.target, 'prod')
  assert.equal(cfg.force, true)
  assert.equal(cfg.url, ENV.DATABASE_URL_PROD)
})

test('reset safety: recover prod with --force resolves ok', () => {
  const cfg = resolve(['story-1', 'recover', 'prod', '--force'], ENV)
  assert.equal(cfg.ok, true)
  if (!cfg.ok) return
  assert.equal(cfg.mode, 'recover')
  assert.equal(cfg.target, 'prod')
  assert.equal(cfg.url, ENV.DATABASE_URL_PROD)
})

test('reset safety: dev default and dev explicit stay force-free', () => {
  const devDefault = resolve(['story-1'], ENV)
  assert.equal(devDefault.ok, true)
  if (!devDefault.ok) return
  assert.equal(devDefault.target, 'dev')
  assert.equal(devDefault.force, false)
  assert.equal(devDefault.url, ENV.DATABASE_URL_DEV)

  const devExplicit = resolve(['story-1', 'reset', 'dev'], ENV)
  assert.equal(devExplicit.ok, true)
  if (!devExplicit.ok) return
  assert.equal(devExplicit.target, 'dev')
  assert.equal(devExplicit.force, false)
  assert.equal(devExplicit.mode, 'reset')
})

test('reset safety: production APP_ENV defaults to prod and still requires --force', () => {
  const prodDefault = resolve(['story-1'], { ...ENV, APP_ENV: 'production' })
  assert.equal(prodDefault.ok, false)
  assert.ok('error' in prodDefault && /--force/.test(prodDefault.error))

  const forced = resolve(['story-1', '--force'], { ...ENV, APP_ENV: 'production' })
  assert.equal(forced.ok, true)
  if (!forced.ok) return
  assert.equal(forced.target, 'prod')
  assert.equal(forced.mode, 'reset')
})

test('reset safety: unknown mode and unknown target error', () => {
  const badMode = resolve(['story-1', 'frobnicate', 'dev'], ENV)
  assert.equal(badMode.ok, false)
  assert.ok('error' in badMode && /unknown mode/.test(badMode.error))

  const badTarget = resolve(['story-1', 'reset', 'staging'], ENV)
  assert.equal(badTarget.ok, false)
  assert.ok('error' in badTarget && /unknown target/.test(badTarget.error))
})

test('reset safety: missing story and missing url error', () => {
  const noStory = resolve([], ENV)
  assert.equal(noStory.ok, false)
  assert.ok('error' in noStory && /usage: forge-story-reset/.test(noStory.error))

  const noProdUrl = resolve(['story-1', 'reset', 'prod', '--force'], {
    DATABASE_URL_DEV: 'postgres://dev',
  })
  assert.equal(noProdUrl.ok, false)
  assert.ok('error' in noProdUrl && /no PROD DATABASE_URL/.test(noProdUrl.error))

  const noDevUrl = resolve(['story-1', 'reset', 'dev'], {
    DATABASE_URL_PROD: 'postgres://prod',
  })
  assert.equal(noDevUrl.ok, false)
  assert.ok('error' in noDevUrl && /no DEV DATABASE_URL/.test(noDevUrl.error))
})

test('reset safety: --force is position-independent', () => {
  const leading = resolve(['--force', 'story-1', 'reset', 'prod'], ENV)
  assert.equal(leading.ok, true)
  if (!leading.ok) return
  assert.equal(leading.target, 'prod')
  assert.equal(leading.mode, 'reset')
  assert.equal(leading.story, 'story-1')

  const midReset = resolve(['story-1', '--force', 'reset', 'prod'], ENV)
  assert.equal(midReset.ok, true)
  if (!midReset.ok) return
  assert.equal(midReset.mode, 'reset')
  assert.equal(midReset.target, 'prod')

  const midRecover = resolve(['story-1', 'recover', '--force', 'prod'], ENV)
  assert.equal(midRecover.ok, true)
  if (!midRecover.ok) return
  assert.equal(midRecover.mode, 'recover')
  assert.equal(midRecover.target, 'prod')
})
