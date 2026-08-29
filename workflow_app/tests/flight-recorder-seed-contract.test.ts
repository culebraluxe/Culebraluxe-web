import test from 'node:test'
import assert from 'node:assert/strict'

import { goldenParticipantRows } from '../../lib/qa-golden'

/**
 * Bounded regression guarding the Golden fixture's deal_participant rows against
 * the canonical invariants in db/migrations/034_deal_participant_invariants.sql:
 *   - uq_deal_participant_active_structural_role: one ACTIVE structural role
 *     (client/owner/seller) per deal.
 *   - uq_deal_participant_active_other_label: one ACTIVE role='other' per
 *     (deal_id, lower(role_label)).
 * Proves the fixture never attempts two active `client` rows (the historical
 * 23505 seed failure) and never invents a role outside the checked vocabulary.
 */

const STRUCTURAL_ROLES = new Set(['client', 'owner', 'seller'])
const ALLOWED_ROLES = new Set(['client', 'owner', 'seller', 'other'])

test('golden participant rows: exactly one active structural client per deal', () => {
  const rows = goldenParticipantRows('maria-uuid', 'juan-uuid')
  const activeStructural = rows.filter((r) => r.active && STRUCTURAL_ROLES.has(r.role))
  // (deal_id, role) is unique among active structural rows — Maria is the client,
  // Juan must NOT also be a client.
  const clientRoles = activeStructural.filter((r) => r.role === 'client')
  assert.equal(clientRoles.length, 1)
  assert.equal(clientRoles[0].personId, 'maria-uuid')
  // No duplicate active structural role could violate the partial unique index.
  const seenRoles = new Set(activeStructural.map((r) => r.role))
  assert.equal(seenRoles.size, activeStructural.length)
})

test('golden participant rows: Juan uses the documented long-tail seam (role=other + label)', () => {
  const rows = goldenParticipantRows('maria-uuid', 'juan-uuid')
  const juan = rows.find((r) => r.personId === 'juan-uuid')
  assert.ok(juan, 'Juan must be present')
  assert.equal(juan.role, 'other')
  assert.equal(juan.roleLabel, 'co-client')
  assert.equal(juan.active, true)
  // Juan is NOT a second structural client — no 23505 on uq_deal_participant_active_structural_role.
  assert.notEqual(juan.role, 'client')
})

test('golden participant rows: every role is within the checked vocabulary', () => {
  const rows = goldenParticipantRows('maria-uuid', 'juan-uuid')
  for (const r of rows) assert.ok(ALLOWED_ROLES.has(r.role), `unexpected role ${r.role}`)
})

test('golden participant rows: both participants are distinct and active', () => {
  const rows = goldenParticipantRows('maria-uuid', 'juan-uuid')
  assert.equal(rows.length, 2)
  assert.notEqual(rows[0].personId, rows[1].personId)
  for (const r of rows) assert.equal(r.active, true)
})

test('golden participant rows: Maria is the canonical client (role=client, no label)', () => {
  const rows = goldenParticipantRows('maria-uuid', 'juan-uuid')
  const maria = rows.find((r) => r.personId === 'maria-uuid')
  assert.ok(maria)
  assert.equal(maria.role, 'client')
  assert.equal(maria.roleLabel, null)
})
