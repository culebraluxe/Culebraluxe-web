import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  SESSION_CONTINUITY_ENV,
  SESSION_MARKER_FILENAME,
  forgeSessionContinuityEnabled,
  forgeSessionMarkerPath,
  readForgeSessionId,
  writeForgeSessionId,
} from './opencode-harness-adapter'
import { buildOpenCodeRunArgs } from './opencode-client'

// ---------------------------------------------------------------------------
// ENG-FORGE-WARM-SESSION-01 — one live session per execution generation.
//
// The old marker held a bare `1`: "somebody ran here, resume the project's LAST session"
// (`--continue`). That is a guess, and it is only correct while exactly one lane is live.
// The marker now holds the actual id, which the next role pins with `--session <id>`.
// These tests fence that contract, the failure path, and the opt-out.
// ---------------------------------------------------------------------------

const withWorkspace = (run: (workspace: string) => void): void => {
  const workspace = mkdtempSync(join(tmpdir(), 'forge-session-'))
  try {
    run(workspace)
  } finally {
    rmSync(workspace, { recursive: true, force: true })
  }
}

/** The env shape the gate reads, built explicitly so the types are honest. */
const envWith = (value?: string): NodeJS.ProcessEnv => {
  const env: NodeJS.ProcessEnv = { NODE_ENV: 'test' }
  if (value !== undefined) env[SESSION_CONTINUITY_ENV] = value
  return env
}

test('continuity is ON by default, with an explicit opt-out', () => {
  assert.equal(forgeSessionContinuityEnabled(envWith()), true, 'one session per generation is the design')
  assert.equal(forgeSessionContinuityEnabled(envWith('1')), true)
  for (const off of ['0', 'false', 'FALSE', 'off']) {
    assert.equal(forgeSessionContinuityEnabled(envWith(off)), false, `${off} must opt out`)
  }
})

test('a generation records its session id and the next role reads it back', () => {
  withWorkspace((workspace) => {
    assert.equal(readForgeSessionId(workspace), null, 'a fresh worktree has no session')
    writeForgeSessionId(workspace, 'ses_abc123')
    assert.equal(readForgeSessionId(workspace), 'ses_abc123')
  })
})

test('a legacy bare `1` marker reads as "unknown id", so it degrades to --continue', () => {
  withWorkspace((workspace) => {
    writeFileSync(forgeSessionMarkerPath(workspace), '1')
    assert.equal(readForgeSessionId(workspace), null)
  })
})

test('a dead session is DROPPED, so the replacement happens once rather than per role', () => {
  withWorkspace((workspace) => {
    writeForgeSessionId(workspace, 'ses_dead')
    assert.equal(readForgeSessionId(workspace), 'ses_dead')

    // The role that discovers the corpse clears the marker...
    writeForgeSessionId(workspace, null)
    assert.equal(readForgeSessionId(workspace), null, 'the next role starts fresh')
  })
})

test('the marker filename lives in the WORKSPACE, so generations cannot share a desk', () => {
  withWorkspace((workspace) => {
    assert.equal(forgeSessionMarkerPath(workspace), join(workspace, SESSION_MARKER_FILENAME))
    writeForgeSessionId(workspace, 'ses_one')
    assert.match(readFileSync(forgeSessionMarkerPath(workspace), 'utf8'), /ses_one/)
  })
})

test('a pinned id is passed as --session and never alongside --continue', () => {
  const args = buildOpenCodeRunArgs({ model: 'deepseek/deepseek-v4-flash', task: 'do it', session: 'ses_abc' })
  assert.ok(args.includes('--session'))
  assert.equal(args[args.indexOf('--session') + 1], 'ses_abc')
  assert.ok(!args.includes('--continue'), 'an exact id and a guess are mutually exclusive')
})

test('no session and no continue leaves a fresh session, as the first role of a generation runs', () => {
  const args = buildOpenCodeRunArgs({ model: 'deepseek/deepseek-v4-flash', task: 'do it' })
  assert.ok(!args.includes('--session'))
  assert.ok(!args.includes('--continue'))
})
