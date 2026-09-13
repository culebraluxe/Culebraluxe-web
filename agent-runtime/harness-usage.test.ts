import assert from 'node:assert/strict'
import { test } from 'node:test'
import { DatabaseSync } from 'node:sqlite'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  harnessStartedAtMs,
  pickSessionForRun,
  readHarnessUsage,
  readSessionUsage,
  usageDelta,
  type SessionUsageRow,
} from './harness-usage'

// ---------------------------------------------------------------------------
// THE SPEND CAPTURE — the link that was missing for two days.
//
// The run row carried `tokens_input`/`tokens_output`/`cost_usd`/`cost_source` since
// migrations 107/133, with a scorecard, a widget and an estimator reading them, and
// NOTHING wrote them: every run reported "unmeasured". The harness's own database has
// the numbers, so this is the reader that feeds them.
//
// Tests run against a REAL temp SQLite file, not a stub, because the thing most likely
// to break here is the query and the driver's value shape (node:sqlite can hand back
// BigInt). The production database is never touched.
// ---------------------------------------------------------------------------

const sessionRow = (over: Partial<SessionUsageRow> = {}): SessionUsageRow => ({
  id: 'ses_a',
  timeCreated: 1_000_000,
  tokensInput: 100,
  tokensOutput: 20,
  cost: 0.5,
  ...over,
})

test('the session nearest the launch wins', () => {
  const sessions = [
    sessionRow({ id: 'ses_far', timeCreated: 1_000_000 - 120_000 }),
    sessionRow({ id: 'ses_near', timeCreated: 1_000_000 + 400 }),
    sessionRow({ id: 'ses_far2', timeCreated: 1_000_000 + 90_000 }),
  ]
  assert.equal(pickSessionForRun(sessions, 1_000_000)?.id, 'ses_near')
})

test('a session outside the window is never claimed', () => {
  const sessions = [sessionRow({ id: 'ses_old', timeCreated: 1_000_000 - 10 * 60_000 })]
  assert.equal(pickSessionForRun(sessions, 1_000_000), null)
})

test('a session that started BEFORE the launch is never claimed', () => {
  // The deterministic Assay spends nothing, and the previous lane's session sits inside
  // any window you care to draw around the Assay's launch — which is exactly how it got
  // claimed and double-counted, live on 2026-09-13.
  const previousLane = sessionRow({ id: 'ses_previous', timeCreated: 1_000_000 - 30_000 })
  assert.equal(pickSessionForRun([previousLane], 1_000_000), null)
})

test('when a launch opens more than one session, the FIRST one is this run', () => {
  const sessions = [
    sessionRow({ id: 'ses_second', timeCreated: 1_000_900 }),
    sessionRow({ id: 'ses_first', timeCreated: 1_000_300 }),
  ]
  assert.equal(pickSessionForRun(sessions, 1_000_000)?.id, 'ses_first')
})

test('no sessions is null, which means unmeasured — never a fabricated zero', () => {
  assert.equal(pickSessionForRun([], 1_000_000), null)
})

test('the launch instant comes from the harness id, then the start time, else null', () => {
  assert.equal(harnessStartedAtMs({ externalRunId: 'opencode-1789318022598' }), 1789318022598)
  assert.equal(
    harnessStartedAtMs({ externalRunId: null, startedAt: '2026-09-13T18:21:31.000Z' }),
    Date.parse('2026-09-13T18:21:31.000Z'),
  )
  assert.equal(harnessStartedAtMs({ externalRunId: 'something-else', startedAt: null }), null)
  assert.equal(harnessStartedAtMs({ externalRunId: 'opencode-notanumber', startedAt: null }), null)
})

test('readHarnessUsage reads the real harness table through the real driver', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-usage-'))
  const dbPath = join(dir, 'opencode.db')
  try {
    const db = new DatabaseSync(dbPath)
    // Only the columns the reader selects, matching the harness's own schema.
    db.exec(
      'create table session (id text primary key, time_created integer not null, ' +
        'tokens_input integer default 0 not null, tokens_output integer default 0 not null, ' +
        'cost real default 0 not null)',
    )
    const insert = db.prepare(
      'insert into session (id, time_created, tokens_input, tokens_output, cost) values (?,?,?,?,?)',
    )
    insert.run('ses_target', 1_700_000_000_000, 26_714, 701, 0.005352)
    insert.run('ses_next', 1_700_000_000_000 + 60_000, 9_999, 999, 9.99)
    db.close()

    const usage = readHarnessUsage({
      harnessStartedAtMs: 1_700_000_000_000,
      dbPath,
    })
    assert.ok(usage, 'the run that started at this instant must be measured')
    assert.equal(usage!.sessionId, 'ses_target')
    assert.equal(usage!.tokensInput, 26_714)
    assert.equal(usage!.tokensOutput, 701)
    assert.equal(usage!.costUsd, 0.005352)
    assert.equal(typeof usage!.tokensInput, 'number', 'never a BigInt')

    // Nothing near that instant: unmeasured, explicitly.
    assert.equal(readHarnessUsage({ harnessStartedAtMs: 1, dbPath }), null)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('a missing or unreadable harness database fails soft to null', () => {
  assert.equal(
    readHarnessUsage({ harnessStartedAtMs: 1_700_000_000_000, dbPath: '/tmp/does-not-exist.db' }),
    null,
  )
})

// --- one session, several roles (WARM-SESSION-01) ----------------------------
//
// The time-window read finds the session a run CREATED, so it returns null for every role
// that RESUMED the generation's session — which is all of them after the first. Resumed
// roles are measured by id instead, and the role's spend is the DIFFERENCE across its turn
// because the session's totals are cumulative for the whole generation.

test('a resumed role is attributed its own delta, not the session lifetime total', () => {
  const before = { sessionId: 'ses_gen', tokensInput: 100, tokensOutput: 20, costUsd: 0.01 }
  const after = { sessionId: 'ses_gen', tokensInput: 260, tokensOutput: 55, costUsd: 0.031 }

  assert.deepEqual(usageDelta(after, before), {
    sessionId: 'ses_gen',
    tokensInput: 160,
    tokensOutput: 35,
    costUsd: 0.021,
  })
})

test('a session rewritten underneath us cannot report negative spend', () => {
  const before = { sessionId: 'ses_gen', tokensInput: 500, tokensOutput: 90, costUsd: 0.05 }
  const after = { sessionId: 'ses_gen', tokensInput: 10, tokensOutput: 5, costUsd: 0.001 }

  assert.deepEqual(usageDelta(after, before), {
    sessionId: 'ses_gen',
    tokensInput: 0,
    tokensOutput: 0,
    costUsd: 0,
  })
})

test('no baseline means the measured totals stand, never a fabrication', () => {
  const after = { sessionId: 'ses_gen', tokensInput: 10, tokensOutput: 5, costUsd: 0.001 }
  assert.deepEqual(usageDelta(after, null), after)
})

test('reading a session by id fails soft when the store is missing', () => {
  assert.equal(readSessionUsage({ sessionId: 'ses_absent', dbPath: '/tmp/does-not-exist.db' }), null)
})

