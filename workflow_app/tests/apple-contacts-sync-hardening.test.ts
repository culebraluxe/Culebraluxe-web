import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  batchTotals,
  classifyReplayFastPath,
  decideBatchLoadStatus,
  decideLoaderExit,
  formatProgress,
  stageWriteDecision,
} from '../../scripts/load-apple-contacts'

// ---------------------------------------------------------------------------
// Apple Contacts PROD sync hardening — targeted proofs (no PROD required).
// ---------------------------------------------------------------------------

const REPO = process.cwd()
const LOCK_LIB = 'scripts/contacts-sync-lock.sh'

function runBash(script: string): string {
  return execSync(script, { cwd: REPO, shell: '/bin/bash', encoding: 'utf8' })
}

// --- Replay / changed / new classification ---------------------------------

test('hardening 1: exact replay classification remains replay', () => {
  const prior = { id: 'r1', revision: 1, payload_fingerprint: 'fp1' }
  assert.equal(classifyReplayFastPath(prior, 'fp1', true), 'replay')
})

test('hardening 2: changed fingerprint routes to the write path (changed)', () => {
  const prior = { id: 'r1', revision: 1, payload_fingerprint: 'fp1' }
  assert.equal(classifyReplayFastPath(prior, 'fp2', true), 'write')
})

test('hardening 3: new identity routes to the write path (new)', () => {
  assert.equal(classifyReplayFastPath(undefined, 'fp1', false), 'write')
})

test('hardening 8: replay fast path does NOT create a new staged revision', () => {
  // A pure replay (prior fingerprint matches + durable inbox already present)
  // is classified 'replay' — the loader skips the staged-profile write entirely.
  const prior = { id: 'r1', revision: 1, payload_fingerprint: 'fp1' }
  assert.equal(classifyReplayFastPath(prior, 'fp1', true), 'replay')
  // Without the durable inbox receipt, the same contact must NOT be skipped:
  // the inbox gap is closed via the write path (no semantic gap).
  assert.equal(classifyReplayFastPath(prior, 'fp1', false), 'write')
})

test('hardening 9: changed contact creates exactly one next revision with correct supersedes', () => {
  const prior = { id: 'r1', revision: 1, payload_fingerprint: 'fp1' }
  assert.deepEqual(stageWriteDecision(prior), { nextRevision: 2, supersedesId: 'r1' })
})

test('hardening 10: new contact gets revision 1 + null supersedes (inbox + staging durability)', () => {
  assert.deepEqual(stageWriteDecision(undefined), { nextRevision: 1, supersedesId: null })
})

// --- Truthful failure semantics ---------------------------------------------

test('hardening 4: mixed batch counters remain balanced', () => {
  const counts = { new: 5, replay: 2573, changed: 0, error: 0 }
  assert.deepEqual(batchTotals(counts, 2578), { valid: 2578, balanced: true })
  // Mixed with errors stays balanced too.
  const mixed = { new: 4, replay: 2500, changed: 1, error: 3 }
  assert.deepEqual(batchTotals(mixed, 2508), { valid: 2505, balanced: true })
})

test('hardening 5: contact-level failure -> non-zero overall loader outcome', () => {
  assert.equal(decideBatchLoadStatus(1), 'failed')
  assert.equal(decideLoaderExit(1), 1)
  assert.equal(decideLoaderExit(5), 1)
})

test('hardening 6: contact-level failure does not erase successful contact work', () => {
  // The batch still records every successful new/replay/changed contact and the
  // load_status truthfully becomes 'failed' (not a full 'loaded' success).
  const counts = { new: 4, replay: 2500, changed: 1, error: 3 }
  const totals = batchTotals(counts, 2508)
  assert.equal(totals.valid, 2505, 'successful work is preserved in the counters')
  assert.equal(totals.balanced, true)
  assert.equal(decideBatchLoadStatus(counts.error), 'failed')
  assert.equal(decideLoaderExit(counts.error), 1)
})

test('hardening 7: successful zero-error load exits successfully', () => {
  const counts = { new: 5, replay: 2573, changed: 0, error: 0 }
  assert.equal(decideBatchLoadStatus(counts.error), 'loaded')
  assert.equal(decideLoaderExit(counts.error), 0)
  assert.equal(batchTotals(counts, 2578).balanced, true)
})

// --- Progress / heartbeat (no PII) ------------------------------------------

test('hardening 11: progress reporting contains no PII', () => {
  const line = formatProgress(500, 2578, 123000)
  assert.match(line, /^\[contacts-sync\] ODS 500\/2578 processed \(123s\)$/)
  // Structurally PII-free: no emails, phones, names, or source ids can appear.
  assert.ok(!line.includes('@'))
  assert.ok(!/\+?\d{6,}/.test(line))
})

// --- Single-run concurrency lock (shell) ------------------------------------

function tmpLock(): string {
  return join(mkdtempSync(join(tmpdir(), 'contacts-lock-test-')), 'lock')
}

test('hardening 12: lock prevents a second active run', () => {
  const lock = tmpLock()
  const out = runBash(`
    export CONTACTS_SYNC_LOCK='${lock}'
    source ${LOCK_LIB}
    contacts_acquire_lock; echo first=$?
    contacts_acquire_lock; echo second=$?
    contacts_release_lock
  `)
  assert.match(out, /first=0/)
  assert.match(out, /second=1/, 'a second acquire while owned must fail')
  rmSync(join(lock, '..'), { recursive: true, force: true })
})

test('hardening 13: stale lock (dead PID) is safe and recoverable', () => {
  const lock = tmpLock()
  const out = runBash(`
    mkdir -p '${lock}'
    echo 999999 > '${lock}/pid'
    export CONTACTS_SYNC_LOCK='${lock}'
    source ${LOCK_LIB}
    contacts_acquire_lock; echo acquired=$?
    echo owned_pid=$(contacts_lock_pid)
    contacts_release_lock
  `)
  assert.match(out, /acquired=0/, 'a stale lock must be reclaimed')
  assert.match(out, /owned_pid=[0-9]+/)
  rmSync(join(lock, '..'), { recursive: true, force: true })
})
