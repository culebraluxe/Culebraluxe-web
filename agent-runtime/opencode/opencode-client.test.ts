// ---------------------------------------------------------------------------
// ENG-FORGE-V5-01 — OpenCode client focused tests (pure + bounded).
//
// Proves the command construction and process-wrapper seams deterministically:
//   - `opencode run` argv construction with the model ALWAYS passed
//     explicitly as deepseek/deepseek-v4-flash (never OpenCode's default)
//   - the non-interactive `--auto` flag in the isolated worker worktree
//   - spawn happens in the EXACT supplied cwd
//   - success / non-zero / launch-failure result mapping to factual evidence
//
// The fake executables below are throwaway shell scripts; no real OpenCode
// model run is ever started, and no Neon/Postgres is touched.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  buildOpenCodeRunArgs,
  startOpenCodeRun,
  type OpenCodeHandle,
} from './opencode-client'
import { OPENCODE_PINNED_MODEL } from './opencode-harness-adapter'

const TASK = 'Execute SDLC story ENG-FORGE-V5-01: OpenCode Harness Adapter.'

/** Write a throwaway executable named `fake-opencode` into `dir`. */
function fixtureBin(dir: string, body: string): string {
  const bin = join(dir, 'fake-opencode')
  writeFileSync(bin, body)
  chmodSync(bin, 0o755)
  return bin
}

test('command construction invokes `opencode run` with the model always pinned', () => {
  const args = buildOpenCodeRunArgs({ model: OPENCODE_PINNED_MODEL, task: TASK })
  assert.deepEqual(args, ['run', '--model', OPENCODE_PINNED_MODEL, '--auto', TASK])
})

test('command construction never relies on OpenCode default model selection', () => {
  const args = buildOpenCodeRunArgs({ model: OPENCODE_PINNED_MODEL, task: TASK })
  assert.ok(args.includes('--model'), 'a --model flag must always be passed')
  const modelIndex = args.indexOf('--model')
  assert.equal(args[modelIndex + 1], OPENCODE_PINNED_MODEL)
  assert.ok(!args.includes('--interactive'), 'run stays non-interactive')
})

test('command construction includes --auto (safe only inside the isolated Forge worktree)', () => {
  const args = buildOpenCodeRunArgs({ model: OPENCODE_PINNED_MODEL, task: TASK })
  assert.ok(args.includes('--auto'))
})

test('client spawns in the exact supplied cwd and maps exit 0 to success', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'opencode-cwd-'))
  try {
    const bin = fixtureBin(dir, '#!/bin/sh\necho "fake opencode output"\nexit 0\n')
    const handle = startOpenCodeRun({
      cliBin: bin,
      cwd: dir,
      model: OPENCODE_PINNED_MODEL,
      task: TASK,
      env: { EXECUTION_ENV: 'DEV' },
    })
    assert.equal(handle.proc.spawnargs[0], bin)
    const result = await handle.promise
    assert.equal(result.status, 'success')
    assert.equal(result.exitCode, 0)
    assert.match(result.stdout, /fake opencode output/)
    assert.equal(handle.done, true)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('client maps a non-zero exit to failed with truthful stderr', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'opencode-nonzero-'))
  try {
    const bin = fixtureBin(dir, '#!/bin/sh\necho "boom from opencode" >&2\nexit 3\n')
    const handle = startOpenCodeRun({
      cliBin: bin,
      cwd: dir,
      model: OPENCODE_PINNED_MODEL,
      task: TASK,
    })
    const result = await handle.promise
    assert.equal(result.status, 'failed')
    assert.equal(result.exitCode, 3)
    assert.match(result.stderr, /boom from opencode/)
    assert.equal(handle.done, true)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('client maps a launch failure (missing binary) to failed, never running forever', async () => {
  const cwd = mkdtempSync(join(tmpdir(), 'opencode-launch-'))
  try {
    const handle: OpenCodeHandle = startOpenCodeRun({
      cliBin: join(cwd, 'does-not-exist-opencode-xyz'),
      cwd,
      model: OPENCODE_PINNED_MODEL,
      task: TASK,
    })
    const result = await handle.promise
    assert.equal(result.status, 'failed')
    assert.equal(result.exitCode, null)
    assert.ok(result.stderr.length > 0)
    assert.equal(handle.done, true)
  } finally {
    rmSync(cwd, { recursive: true, force: true })
  }
})
