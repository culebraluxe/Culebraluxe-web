import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  assessMigrationPreflight,
  preflightMigrationStart,
} from '../forge/migration-applied-guard'

// ENG-FORGE-MIGRATION-START-01 — the start-seam preflight. A run must not START while a repo
// db/migrations/*.sql file is absent from the PROD schema_migration ledger. It reads the repo
// file LIST and the LEDGER TABLE only — never a story diff — and an unreadable ledger fails
// CLOSED to a named refusal rather than to a silent start.
const repo = [
  'db/migrations/184_whatsapp_context_id.sql',
  'db/migrations/185_supersede.sql',
  'db/migrations/186_acceptance_assertions.sql',
]

test('refuses when the repo list carries migrations the ledger lacks', () => {
  const result = assessMigrationPreflight({
    repoPaths: repo,
    ledgerFilenames: ['db/migrations/184_whatsapp_context_id.sql'],
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
    repoPaths: ['app/page.tsx', 'db/migrations/184_whatsapp_context_id.sql', 'lib/x.ts'],
    ledgerFilenames: ['db/migrations/184_whatsapp_context_id.sql'],
  })
  assert.equal(result.ok, true)
  assert.deepEqual(result.unapplied, [])
})

test('a throwing ledger reader returns ok:false with a named refusal', async () => {
  const result = await preflightMigrationStart({
    repoPaths: repo,
    readLedger: async () => {
      throw new Error('DATABASE_URL_PROD is not configured')
    },
  })
  assert.equal(result.ok, false)
  assert.match(String(result.refusal), /ledger/i)
})

test('both directions over fixture repo list and fixture ledger', async () => {
  const unapplied = await preflightMigrationStart({
    repoPaths: repo,
    ledgerFilenames: ['db/migrations/184_whatsapp_context_id.sql'],
  })
  assert.equal(unapplied.ok, false)
  const applied = await preflightMigrationStart({ repoPaths: repo, ledgerFilenames: repo })
  assert.equal(applied.ok, true)
})

test('three-file case', () => {
  const result = assessMigrationPreflight({
    repoPaths: repo,
    ledgerFilenames: ['db/migrations/186_acceptance_assertions.sql'],
  })
  assert.equal(result.ok, false)
  assert.deepEqual(result.unapplied, [
    'db/migrations/184_whatsapp_context_id.sql',
    'db/migrations/185_supersede.sql',
  ])
})
