import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  addOtherParticipant,
  assertNoActiveStructuralRole,
  endParticipant,
  updateParticipantRoleLabel,
} from '../../db/deal-participants'
import { PortalWriteError } from '../../lib/portal-write-error'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// CRM-13 — canonical deal_participant service-level invariants.
//
// deal_participant is THE canonical participant model; the service layer
// enforces the same invariants the DB layer guarantees (migration 034):
//   - at most one active structural participant (client/owner/seller) per deal
//   - at most one active role='other' participant per role_label per deal
//     (case-insensitive)
// These tests run against an in-memory fake executor (no database, no env) and
// assert the service checks fire BEFORE any write, so a duplicate can never
// reach the DB layer. The DB-level unique indexes are asserted statically in
// deal-participant-migration.test.ts and proven live in
// persistence/deal-participant-invariants.test.ts.
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeParticipantDb {
  rows: Row[] = []
  seq = 0

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce(
        (acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''),
        '',
      ),
    )
    const p = params as any[]

    if (t.includes('insert into deal_participant')) {
      // The module's insert template literalizes role ('other') and active
      // (true); the parameters are (deal_id, person_id, user_id, role_label).
      const row = {
        id: `p-${++this.seq}`,
        deal_id: p[0],
        person_id: p[1] ?? null,
        user_id: p[2] ?? null,
        role: 'other',
        role_label: p[3] ?? null,
        active: true,
      }
      this.rows.push(row)
      return Promise.resolve([{ id: row.id }])
    }

    if (t.includes('update deal_participant') && t.includes('set active = false')) {
      const row = this.rows.find((r) => r.id === p[0] && r.active === true)
      if (!row) return Promise.resolve([])
      row.active = false
      row.ended_at = 'now'
      return Promise.resolve([{ id: row.id }])
    }

    if (t.includes('update deal_participant') && t.includes('set role_label')) {
      // Params are (role_label, participant_id).
      const row = this.rows.find((r) => r.id === p[1] && r.role === 'other')
      if (!row) return Promise.resolve([])
      row.role_label = p[0]
      return Promise.resolve([{ id: row.id }])
    }

    if (t.includes('select deal_id, active from deal_participant')) {
      const row = this.rows.find((r) => r.id === p[0] && r.role === 'other')
      return Promise.resolve(
        row ? [{ deal_id: row.deal_id, active: row.active }] : [],
      )
    }

    if (
      t.includes('select id from deal_participant') &&
      t.includes('lower(role_label)')
    ) {
      const found = this.rows.some(
        (r) =>
          r.deal_id === p[0] &&
          r.role === 'other' &&
          r.active === true &&
          r.role_label !== null &&
          String(r.role_label).toLowerCase() === String(p[1]).toLowerCase() &&
          (p[2] === null || r.id !== p[2]),
      )
      return Promise.resolve(found ? [{ id: 'dup' }] : [])
    }

    if (t.includes('select id from deal_participant')) {
      const found = this.rows.some(
        (r) => r.deal_id === p[0] && r.role === p[1] && r.active === true,
      )
      return Promise.resolve(found ? [{ id: 'exists' }] : [])
    }

    throw new Error(`unhandled query: ${t}`)
  }
}

function isValidation(err: unknown) {
  return err instanceof PortalWriteError && err.code === 'validation'
}

function isConflict(err: unknown) {
  return err instanceof PortalWriteError && err.code === 'conflict'
}

test('addOtherParticipant requires exactly one of person or user', async () => {
  const db = new FakeParticipantDb()
  await assert.rejects(
    addOtherParticipant({ dealId: 'd1', roleLabel: 'Lender' }, db.tx),
    isValidation,
  )
  await assert.rejects(
    addOtherParticipant(
      { dealId: 'd1', personId: 'p1', userId: 'u1', roleLabel: 'Lender' },
      db.tx,
    ),
    isValidation,
  )
  assert.equal(db.rows.length, 0, 'no write must reach the storage layer')
})

test('addOtherParticipant validates the role label (required, <= 120 chars)', async () => {
  const db = new FakeParticipantDb()
  await assert.rejects(
    addOtherParticipant({ dealId: 'd1', personId: 'p1', roleLabel: '   ' }, db.tx),
    isValidation,
  )
  await assert.rejects(
    addOtherParticipant(
      { dealId: 'd1', personId: 'p1', roleLabel: 'x'.repeat(121) },
      db.tx,
    ),
    isValidation,
  )
  assert.equal(db.rows.length, 0)
})

test('addOtherParticipant rejects a duplicate active role_label within a deal (case-insensitive)', async () => {
  const db = new FakeParticipantDb()
  const first = await addOtherParticipant(
    { dealId: 'd1', personId: 'p1', roleLabel: 'Lender' },
    db.tx,
  )
  assert.ok(first.participantId)
  await assert.rejects(
    addOtherParticipant(
      { dealId: 'd1', personId: 'p2', roleLabel: 'lender' },
      db.tx,
    ),
    isConflict,
  )
  assert.equal(db.rows.length, 1, 'the duplicate must never be written')
})

test('addOtherParticipant allows the same role_label on a different deal', async () => {
  const db = new FakeParticipantDb()
  await addOtherParticipant({ dealId: 'd1', personId: 'p1', roleLabel: 'Lender' }, db.tx)
  const second = await addOtherParticipant(
    { dealId: 'd2', personId: 'p1', roleLabel: 'Lender' },
    db.tx,
  )
  assert.ok(second.participantId)
  assert.equal(db.rows.length, 2)
})

test('addOtherParticipant allows the label again after the previous participant ended', async () => {
  const db = new FakeParticipantDb()
  const first = await addOtherParticipant(
    { dealId: 'd1', personId: 'p1', roleLabel: 'Lender' },
    db.tx,
  )
  await endParticipant(first.participantId, db.tx)
  const second = await addOtherParticipant(
    { dealId: 'd1', personId: 'p2', roleLabel: 'Lender' },
    db.tx,
  )
  assert.ok(second.participantId)
  assert.equal(db.rows.length, 2, 'ended rows do not block a new active one')
})

test('endParticipant deactivates once; conflicts on already-ended or unknown', async () => {
  const db = new FakeParticipantDb()
  const created = await addOtherParticipant(
    { dealId: 'd1', personId: 'p1', roleLabel: 'Lender' },
    db.tx,
  )
  const ended = await endParticipant(created.participantId, db.tx)
  assert.equal(ended.participantId, created.participantId)
  await assert.rejects(endParticipant(created.participantId, db.tx), isConflict)
  await assert.rejects(endParticipant('missing', db.tx), isConflict)
})

test('updateParticipantRoleLabel rejects a label held by another active participant', async () => {
  const db = new FakeParticipantDb()
  const a = await addOtherParticipant(
    { dealId: 'd1', personId: 'p1', roleLabel: 'Lender' },
    db.tx,
  )
  const b = await addOtherParticipant(
    { dealId: 'd1', personId: 'p2', roleLabel: 'Inspector' },
    db.tx,
  )
  await assert.rejects(
    updateParticipantRoleLabel(b.participantId, 'LENDER', db.tx),
    isConflict,
  )
  // Renaming the participant to its own label (any casing) is allowed.
  const self = await updateParticipantRoleLabel(a.participantId, 'LENDER', db.tx)
  assert.equal(self.participantId, a.participantId)
})

test('updateParticipantRoleLabel only applies to role=other participants', async () => {
  const db = new FakeParticipantDb()
  db.rows.push({
    id: 'structural',
    deal_id: 'd1',
    person_id: 'p1',
    user_id: null,
    role: 'client',
    role_label: null,
    active: true,
  })
  await assert.rejects(
    updateParticipantRoleLabel('structural', 'Lender', db.tx),
    isConflict,
  )
})

test('assertNoActiveStructuralRole enforces one active client/owner/seller per deal', async () => {
  const db = new FakeParticipantDb()
  db.rows.push({
    id: 'client-1',
    deal_id: 'd1',
    person_id: 'p1',
    user_id: null,
    role: 'client',
    role_label: null,
    active: true,
  })
  await assert.rejects(
    assertNoActiveStructuralRole('d1', 'client', db.tx),
    isConflict,
  )
  await assert.doesNotReject(assertNoActiveStructuralRole('d1', 'seller', db.tx))
  await assert.doesNotReject(assertNoActiveStructuralRole('d2', 'client', db.tx))

  // An ended structural row does not block a new active one.
  db.rows.push({
    id: 'client-ended',
    deal_id: 'd2',
    person_id: 'p9',
    user_id: null,
    role: 'client',
    role_label: null,
    active: false,
  })
  await assert.doesNotReject(assertNoActiveStructuralRole('d2', 'client', db.tx))
})
