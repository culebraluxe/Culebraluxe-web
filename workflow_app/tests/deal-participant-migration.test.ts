import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

// ---------------------------------------------------------------------------
// CRM-13 — migration content invariants (DB-free, deterministic).
//
// The DB-layer guarantees behind deal_participant are declared in migrations
// 034 (unique partial indexes + legacy-FK reconciliation) and 035 (stale
// Story Board note reconciliation). These tests assert the migrations actually
// declare them, so a future edit cannot silently drop the invariant. The live
// behavior is proven in persistence/deal-participant-invariants.test.ts.
// ---------------------------------------------------------------------------

const MIGRATION_034 = new URL(
  '../../db/migrations/034_deal_participant_invariants.sql',
  import.meta.url,
)
const MIGRATION_035 = new URL(
  '../../db/migrations/035_storyboard_crm13_note.sql',
  import.meta.url,
)

function compact(sql: string): string {
  return sql.replace(/\s+/g, ' ').trim()
}

test('034 declares the one-active-structural-role-per-deal unique index', async () => {
  const sql = compact(await readFile(MIGRATION_034, 'utf8'))
  assert.match(
    sql,
    /create unique index uq_deal_participant_active_structural_role\s+on deal_participant \(deal_id, role\)\s+where role in \('client', 'owner', 'seller'\) and active/,
    'partial unique index over (deal_id, role) filtered to active structural roles',
  )
})

test('034 declares the one-active-role_label-per-deal unique index (long tail)', async () => {
  const sql = compact(await readFile(MIGRATION_034, 'utf8'))
  assert.match(
    sql,
    /create unique index uq_deal_participant_active_other_label\s+on deal_participant \(deal_id, lower\(role_label\)\)\s+where role = 'other' and active/,
    'partial unique index over (deal_id, lower(role_label)) filtered to active role=other rows',
  )
})

test('034 reconciles legacy FKs into active participant rows before indexing', async () => {
  const sql = compact(await readFile(MIGRATION_034, 'utf8'))
  for (const role of ['client', 'owner', 'seller']) {
    assert.ok(
      sql.includes(`'${role}'`),
      `reconciliation must reference role '${role}'`,
    )
  }
  assert.ok(
    sql.includes('insert into deal_participant'),
    'reconciliation must insert participant rows',
  )
  assert.ok(
    sql.includes('not exists'),
    'reconciliation must be idempotent (skip deals that already have the row)',
  )
})

test('035 reconciles the stale CRM-13 note without changing status/completion', async () => {
  const sql = compact(await readFile(MIGRATION_035, 'utf8'))
  assert.match(sql, /update storyboard_story\s+set notes =/i)
  assert.match(sql, /where id = 'CRM-13'/i)
  assert.match(sql, /status = 'Partial'/i)
  assert.match(sql, /completion = 50/i)
  assert.ok(
    !sql.includes("status = 'Complete'"),
    'the note update must not change the story status',
  )
})
