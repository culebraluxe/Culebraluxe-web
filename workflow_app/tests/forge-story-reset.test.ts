// ---------------------------------------------------------------------------
// ENG-FORGE-V14 — reset safety: pure argv/env resolution + the PROD --force gate.
// resolveStoryResetConfig is exercised with NO DB and NO child process: the
// resolver is side-effect free by contract.
//
// 2026-09-12 — THE TARGET IS NO LONGER A CHOICE. Captain: "always PROD but this
// should be domain responsibility of the Pool Manager — it should not even have the
// ability to make that choice." So these cases pin the new contract:
//   - the environment comes from the ONE declaration, never from a positional;
//   - a positional target is refused BY NAME, saying who owns the decision;
//   - a DEV declaration is refused outright;
//   - an undeclared environment is refused with the declaration's own reason;
//   - PROD still requires --force (that gate is about a destructive act);
//   - --force stays position-independent.
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

const PROD_ENV = { DATABASE_URL_PROD: 'postgres://prod', APP_ENV: 'production' }
const DEV_ENV = { DATABASE_URL_DEV: 'postgres://dev', APP_ENV: 'development' }

test('reset safety: PROD requires --force (destructive act stays deliberate)', () => {
  const cfg = resolve(['story-1', 'reset'], PROD_ENV)
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /--force/)
})

test('reset safety: PROD + --force resolves, and the target came from the declaration', () => {
  const cfg = resolve(['story-1', 'reset', '--force'], PROD_ENV)
  assert.equal(cfg.ok, true)
  if (!cfg.ok) return
  assert.equal(cfg.story, 'story-1')
  assert.equal(cfg.mode, 'reset')
  assert.equal(cfg.target, 'prod')
  assert.equal(cfg.force, true)
})

test('reset safety: recover on PROD is gated identically', () => {
  assert.equal(resolve(['story-1', 'recover'], PROD_ENV).ok, false)
  const ok = resolve(['story-1', 'recover', '--force'], PROD_ENV)
  assert.equal(ok.ok, true)
  if (!ok.ok) return
  assert.equal(ok.mode, 'recover')
  assert.equal(ok.target, 'prod')
})

test('reset safety: a positional target is REFUSED and names the pool manager', () => {
  for (const attempt of ['prod', 'dev']) {
    const cfg = resolve(['story-1', 'reset', attempt, '--force'], PROD_ENV)
    assert.equal(cfg.ok, false, `positional ${attempt} must not be accepted`)
    if (cfg.ok) continue
    assert.match(cfg.error, /not a choice/)
    assert.match(cfg.error, /pool manager/)
  }
})

test('reset safety: a DEV declaration is refused outright', () => {
  const cfg = resolve(['story-1', 'reset', '--force'], DEV_ENV)
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /refusing to run against DEV/)
  assert.match(cfg.error, /pool manager/)
})

test('reset safety: an undeclared environment is refused with the declaration reason', () => {
  const cfg = resolve(['story-1', 'reset', '--force'], {})
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /not declared/)
  // The reason must name the variables that would fix it.
  assert.match(cfg.error, /APP_ENV/)
})

test('reset safety: an unknown mode is refused before anything else', () => {
  const cfg = resolve(['story-1', 'obliterate', '--force'], PROD_ENV)
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /unknown mode/)
})

test('reset safety: no story id prints usage', () => {
  const cfg = resolve([], PROD_ENV)
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /usage:/)
})

test('reset safety: a declared PROD with no url configured is refused', () => {
  const cfg = resolve(['story-1', 'reset', '--force'], { APP_ENV: 'production' })
  assert.equal(cfg.ok, false)
  if (cfg.ok) return
  assert.match(cfg.error, /DATABASE_URL_PROD is not configured/)
})

test('reset safety: --force is position-independent', () => {
  const cfg = resolve(['--force', 'story-1'], PROD_ENV)
  assert.equal(cfg.ok, true)
  if (!cfg.ok) return
  assert.equal(cfg.story, 'story-1')
  assert.equal(cfg.mode, 'reset', 'mode defaults to reset')
})
