import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  LONG_TAIL_ROLE_LABELS,
  RESPONSIBILITY_HINTS,
  resolveParticipantTarget,
  resolveResponsibility,
} from '../../workflow_app/responsibility'
import { PARTICIPANT_ROLE_LABEL_MAX } from '../../db/deal-participants'

// ---------------------------------------------------------------------------
// CRM-13 — responsibility hint vocabulary ↔ deal_participant role_label
// reconciliation.
//
// The workflow XML responsibility hint is a SEPARATE concept that resolves to
// a participant via role_label — the two taxonomies are not merged:
//   - buyer  -> structural role='client'  (no role_label)
//   - seller -> structural role='seller'  (no role_label)
//   - lender/inspector/appraiser/notario/title_company
//            -> long tail: role='other' + role_label
//   - brokerage / other_sme -> no participant target
// ---------------------------------------------------------------------------

test('buyer and seller resolve to structural participant roles without role_label', () => {
  const buyer = resolveParticipantTarget(resolveResponsibility('buyer'))
  assert.deepEqual(buyer, { kind: 'structural', role: 'client' })

  const seller = resolveParticipantTarget(resolveResponsibility('seller'))
  assert.deepEqual(seller, { kind: 'structural', role: 'seller' })
})

test('SME hints resolve to the long tail: role=other + role_label', () => {
  const expectations: Record<string, string> = {
    lender: 'lender',
    inspector: 'inspector',
    appraiser: 'appraiser',
    notario: 'notario',
    title_company: 'title',
  }
  for (const [hint, expectedLabel] of Object.entries(expectations)) {
    const spec = resolveResponsibility(hint)
    const target = resolveParticipantTarget(spec)
    assert.deepEqual(
      target,
      { kind: 'long_tail', role: 'other', roleLabel: expectedLabel },
      `${hint} must resolve to role=other + role_label '${expectedLabel}'`,
    )
    assert.equal(
      spec.smeRoleLabel,
      expectedLabel,
      `${hint} smeRoleLabel must equal the canonical role_label`,
    )
    assert.ok(
      expectedLabel.length <= PARTICIPANT_ROLE_LABEL_MAX,
      `role_label '${expectedLabel}' must fit the 120-char column`,
    )
  }
})

test('brokerage and other_sme have no participant target', () => {
  assert.equal(resolveParticipantTarget(resolveResponsibility('brokerage')), null)
  assert.equal(resolveParticipantTarget(resolveResponsibility('other_sme')), null)
})

test('unknown hints fall back to a neutral spec with no participant target', () => {
  const spec = resolveResponsibility('mystery_role')
  assert.equal(spec.owner, 'other')
  assert.equal(spec.label, 'mystery_role')
  assert.equal(resolveParticipantTarget(spec), null)
  const unassigned = resolveResponsibility(undefined)
  assert.equal(unassigned.label, 'Unassigned')
  assert.equal(resolveParticipantTarget(unassigned), null)
})

test('LONG_TAIL_ROLE_LABELS is the canonical, drift-free long-tail vocabulary', () => {
  assert.deepEqual(
    [...LONG_TAIL_ROLE_LABELS].sort(),
    ['appraiser', 'inspector', 'lender', 'notario', 'title'],
  )
  assert.equal(new Set(LONG_TAIL_ROLE_LABELS).size, LONG_TAIL_ROLE_LABELS.length)

  // Every long-tail hint is declared as participantRole='other' with a
  // role_label, and every structural hint is declared with its role — the
  // table itself must stay consistent.
  for (const [hint, spec] of Object.entries(RESPONSIBILITY_HINTS)) {
    if (spec.participantRole === 'other') {
      assert.ok(
        spec.smeRoleLabel && spec.smeRoleLabel.length > 0,
        `${hint} must carry a role_label for the long tail`,
      )
    }
    if (spec.smeRoleLabel) {
      assert.ok(
        spec.smeRoleLabel.length <= PARTICIPANT_ROLE_LABEL_MAX,
        `${hint} role_label exceeds the deal_participant column limit`,
      )
    }
  }
})
