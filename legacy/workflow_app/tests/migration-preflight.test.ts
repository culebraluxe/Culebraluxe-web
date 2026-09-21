import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  assessMigrationPreflight,
  preflightMigrationStart,
} from '@/legacy/workflow_app/forge/migration-applied-guard'

// ENG-FORGE-MIGRATION-START-01 — the start-seam preflight. A run must not START while a repo
// db/migrations/*.sql file is absent from the PROD schema_migration ledger. It reads the repo
// file LIST and the LEDGER TABLE only — never a story diff — and an unreadable ledger fails
// CLOSED to a named refusal rather than to a silent start.
const repo = [
  'legacy/db/migrations/184_whatsapp_context_id.sql',
  'legacy/db/migrations/185_supersede.sql',
  'legacy/db/migrations/186_acceptance_assertions.sql',
]

test('refuses when the repo list carries migrations the ledger lacks', () => {
  const result = assessMigrationPreflight({
    repoPaths: repo,
    ledgerFilenames: ['legacy/db/migrations/184_whatsapp_context_id.sql'],
  })
  assert.equal(result.ok, false)
  assert.notEqual(result.refusal, null)
})

test('names every unapplied file, three-file case (184/185/186)', () => {
  const result = assessMigrationPreflight({ repoPaths: repo, ledgerFilenames: [] })
  assert.equal(result.ok, false)
  assert.deepEqual(result.unapplied, repo)
  assert.match(String(result.refusal), /184_whatsapp_context_id\.sql/)
  assert.match(String(result.refusal), /185_supersede\.sql/)
  assert.match(String(result.refusal), /186_acceptance_assertions\.sql/)
})

test('proceeds when the ledger carries every repo migration', () => {
  const result = assessMigrationPreflight({ repoPaths: repo, ledgerFilenames: repo })
  assert.equal(result.ok, true)
  assert.deepEqual(result.unapplied, [])
  assert.equal(result.refusal, null)
})

test('takes only repo paths and ledger filenames, no story diff input', () => {
  const result = assessMigrationPreflight({
    repoPaths: ['app/page.tsx', 'legacy/db/migrations/184_whatsapp_context_id.sql', 'lib/x.ts'],
    ledgerFilenames: ['legacy/db/migrations/184_whatsapp_context_id.sql'],
  })
  assert.equal(result.ok, true)
  assert.deepEqual(result.unapplied, [])
})

test('a throwing ledger reader returns ok:false with a named refusal', async () => {
  const result = await preflightMigrationStart({
    repoPaths: repo,
    baselineAt: null,
    readLedger: async () => {
      throw new Error('DATABASE_URL_PROD is not configured')
    },
  })
  assert.equal(result.ok, false)
  assert.match(String(result.refusal), /ledger/i)
})

test('both directions over fixture repo list and fixture ledger', async () => {
  // baselineAt: null is the STRICT case: a ledger claiming coverage from the start judges every file.
  const unapplied = await preflightMigrationStart({
    repoPaths: repo,
    baselineAt: null,
    ledgerFilenames: ['legacy/db/migrations/184_whatsapp_context_id.sql'],
  })
  assert.equal(unapplied.ok, false)
  const applied = await preflightMigrationStart({ repoPaths: repo, baselineAt: null, ledgerFilenames: repo })
  assert.equal(applied.ok, true)
})

// THE BASELINE RULE (2026-09-17). The ledger is authoritative only from its `<baseline>` row forward; the
// repo says so in scripts/migration-status.mjs ("unrecorded (pre-baseline or never applied here)"). The
// first cut judged the whole repo, saw ~150 pre-baseline migrations, and refused to START ANY RUN — the
// factory stop this guard was meant to prevent. Only migrations ADDED SINCE THE BASELINE can be judged.
test('with a baseline, a pre-baseline migration is NOT reported as unapplied', async () => {
  const result = await preflightMigrationStart({
    repoPaths: ['legacy/db/migrations/001_initial_schema.sql', 'legacy/db/migrations/187_new.sql'],
    ledgerFilenames: ['<baseline>', 'legacy/db/migrations/186_acceptance_assertions.sql'],
    baselineAt: '2026-09-10T00:00:00.000Z',
    candidatePaths: ['legacy/db/migrations/187_new.sql'],
  })
  assert.equal(result.ok, false)
  assert.deepEqual(result.unapplied, ['legacy/db/migrations/187_new.sql'])
  assert.doesNotMatch(String(result.refusal), /001_initial_schema/)
})

test('with a baseline, the three-file case still refuses and names only those files', async () => {
  const result = await preflightMigrationStart({
    repoPaths: ['legacy/db/migrations/001_initial_schema.sql', ...repo],
    ledgerFilenames: ['<baseline>'],
    baselineAt: '2026-09-10T00:00:00.000Z',
    candidatePaths: repo,
  })
  assert.equal(result.ok, false)
  assert.deepEqual(result.unapplied, repo)
  assert.doesNotMatch(String(result.refusal), /001_initial_schema/)
})

test('a history that cannot be read since the baseline refuses by name', async () => {
  const result = await preflightMigrationStart({
    repoPaths: repo,
    ledgerFilenames: ['<baseline>'],
    baselineAt: '2026-09-10T00:00:00.000Z',
    readCandidatePaths: () => {
      throw new Error('git history unavailable')
    },
  })
  assert.equal(result.ok, false)
  assert.match(String(result.refusal), /baseline/i)
  assert.match(String(result.refusal), /pre-baseline/)
})

test('three-file case', () => {
  const result = assessMigrationPreflight({
    repoPaths: repo,
    ledgerFilenames: ['legacy/db/migrations/186_acceptance_assertions.sql'],
  })
  assert.equal(result.ok, false)
  assert.deepEqual(result.unapplied, [
    'legacy/db/migrations/184_whatsapp_context_id.sql',
    'legacy/db/migrations/185_supersede.sql',
  ])
})
