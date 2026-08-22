import { test } from 'node:test'
import assert from 'node:assert/strict'

import { interactiveSql } from '../../../lib/neon-interactive'

// ---------------------------------------------------------------------------
// CRM-13 — live deal_participant invariant proofs (DEV database).
//
// Scoped persistence proofs for the changed seam only. Every write runs
// inside an interactiveSql.begin() transaction that is rolled back (the
// success-path proof deliberately throws a rollback marker after its
// assertions), so no test data is ever committed to DEV.
//
// Requires migration 034 (deal_participant invariants) applied to the DEV
// branch; the tests skip cleanly when the indexes are not present.
// ---------------------------------------------------------------------------

async function indexesPresent(): Promise<boolean> {
  const rows = await interactiveSql`
    select indexname
    from pg_indexes
    where tablename = 'deal_participant'
      and indexname in (
        'uq_deal_participant_active_structural_role',
        'uq_deal_participant_active_other_label'
      )
  `
  return rows.length === 2
}

async function fixtureDealWithClient(): Promise<{
  dealId: string
  personId: string
} | null> {
  const rows = await interactiveSql`
    select d.id as deal_id, dp.person_id
    from deal d
    join deal_participant dp
      on dp.deal_id = d.id
      and dp.role = 'client'
      and dp.active = true
    order by d.created_at asc
    limit 1
  `
  const row = rows[0] as { deal_id?: string; person_id?: string } | undefined
  return row?.deal_id && row.person_id
    ? { dealId: row.deal_id, personId: row.person_id }
    : null
}

function isUniqueViolation(err: unknown): boolean {
  return (
    err instanceof Error &&
    (err as { code?: string }).code === '23505'
  )
}

const ROLLBACK_MARKER = new Error('CRM-13 rollback marker')

test('a deal cannot gain a second active client participant (structural invariant)', async (t) => {
  if (!(await indexesPresent())) {
    t.skip('migration 034 indexes not applied to DEV')
    return
  }
  const fixture = await fixtureDealWithClient()
  if (!fixture) {
    t.skip('no DEV deal with an active client participant to probe')
    return
  }

  await assert.rejects(
    interactiveSql.begin(async (tx) => {
      await tx`
        insert into deal_participant (deal_id, person_id, role, active)
        values (${fixture.dealId}, ${fixture.personId}, 'client', true)
      `
    }),
    isUniqueViolation,
    'second active client row must be rejected by uq_deal_participant_active_structural_role',
  )
})

test('a deal cannot hold two active role=other participants with the same role_label (case-insensitive)', async (t) => {
  if (!(await indexesPresent())) {
    t.skip('migration 034 indexes not applied to DEV')
    return
  }
  const fixture = await fixtureDealWithClient()
  if (!fixture) {
    t.skip('no DEV deal with an active client participant to probe')
    return
  }

  await assert.rejects(
    interactiveSql.begin(async (tx) => {
      await tx`
        insert into deal_participant (deal_id, person_id, role, role_label, active)
        values (${fixture.dealId}, ${fixture.personId}, 'other', 'CRM13-Proof-Lender', true)
      `
      await tx`
        insert into deal_participant (deal_id, person_id, role, role_label, active)
        values (${fixture.dealId}, ${fixture.personId}, 'other', 'crm13-proof-lender', true)
      `
    }),
    isUniqueViolation,
    'duplicate active role_label (different casing) must be rejected by uq_deal_participant_active_other_label',
  )
})

test('distinct long-tail role_labels coexist for the same deal (variety is allowed)', async (t) => {
  if (!(await indexesPresent())) {
    t.skip('migration 034 indexes not applied to DEV')
    return
  }
  const fixture = await fixtureDealWithClient()
  if (!fixture) {
    t.skip('no DEV deal with an active client participant to probe')
    return
  }

  await assert.rejects(
    interactiveSql.begin(async (tx) => {
      await tx`
        insert into deal_participant (deal_id, person_id, role, role_label, active)
        values (${fixture.dealId}, ${fixture.personId}, 'other', 'CRM13-Proof-Surveyor', true)
      `
      await tx`
        insert into deal_participant (deal_id, person_id, role, role_label, active)
        values (${fixture.dealId}, ${fixture.personId}, 'other', 'CRM13-Proof-Appraiser', true)
      `
      // Both inserts succeeded (no unique violation) — now force the rollback.
      throw ROLLBACK_MARKER
    }),
    (err) => err === ROLLBACK_MARKER,
    'distinct role_labels must both insert; the transaction must then roll back',
  )
})

test('read projections agree with legacy FKs — no dual-truth drift (migration 034 reconciliation)', async () => {
  const rows = await interactiveSql`
    select
      count(*)::int as deals_total,
      count(*)::int as client_fk_without_active_participant,
      count(*)::int as owner_fk_without_active_participant,
      count(*)::int as seller_fk_without_active_participant,
      count(*)::int as active_client_participant_mismatch,
      count(*)::int as active_owner_participant_mismatch,
      count(*)::int as active_seller_participant_mismatch
    from deal d
    left join property p
      on p.id = d.property_id
    where
      (d.client_person_id is not null and not exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'client' and dp.active
          and dp.person_id = d.client_person_id
      ))
      or (d.owner_user_id is not null and not exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'owner' and dp.active
          and dp.user_id = d.owner_user_id
      ))
      or (p.seller_person_id is not null and not exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'seller' and dp.active
          and dp.person_id = p.seller_person_id
      ))
      or exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'client' and dp.active
          and (d.client_person_id is null or dp.person_id is distinct from d.client_person_id)
      )
      or exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'owner' and dp.active
          and (d.owner_user_id is null or dp.user_id is distinct from d.owner_user_id)
      )
      or exists (
        select 1 from deal_participant dp
        where dp.deal_id = d.id and dp.role = 'seller' and dp.active
          and (p.seller_person_id is null or dp.person_id is distinct from p.seller_person_id)
      )
  `
  const row = rows[0] as Record<string, number | string>
  const mismatchCount = Number(row.deals_total)
  assert.equal(
    mismatchCount,
    0,
    `legacy FKs and active participant rows must agree (${mismatchCount} mismatching deal(s))`,
  )
})

test('CRM-13 storyboard note is reconciled; status and completion unchanged', async () => {
  const rows = await interactiveSql`
    select status, completion, notes
    from storyboard_story
    where id = 'CRM-13'
    limit 1
  `
  const row = rows[0] as
    | { status?: string; completion?: number; notes?: string }
    | undefined
  assert.ok(row, 'CRM-13 storyboard row must exist')
  assert.equal(row.status, 'Partial')
  assert.equal(Number(row.completion), 50)
  assert.ok(
    row.notes && row.notes.includes('canonical participant model'),
    'the stale note must be reconciled to the canonical-model state',
  )
})
