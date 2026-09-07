// ---------------------------------------------------------------------------
// TESTV2 — P&S (the monster): field ownership map + template audit.
//
// PR-PNS is the largest legal form. Its hydrate correctness rests on one
// invariant: EVERY template field is bound to a non-deal owner
// (Contract role / Property / Contract terms). This spec locks that invariant
// and the "no silent Deal ownership" rule against the ACTIVE template.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  getActiveTemplate,
  PURCHASE_SALE_TEMPLATE_ID,
} from '../lib/forms/template-registry'
import {
  auditPnsFieldBindings,
  getPnsFieldBinding,
  PNS_FIELD_BINDINGS,
} from '../lib/forms/pns-field-binding'

const template = getActiveTemplate(PURCHASE_SALE_TEMPLATE_ID)
assert.ok(template, 'an active PR-PNS template is required')
const audit = auditPnsFieldBindings(template)

test('PR-PNS active template is exhaustively bound (no orphan, stale, or adapter gap)', () => {
  assert.equal(audit.templateFieldCount, template.fields.length)
  assert.equal(audit.mappedFieldCount, template.fields.length, 'every field must have a binding')
  assert.deepEqual(audit.orphanFields, [], 'no template field may be unhydratable')
  assert.deepEqual(audit.staleBindings, [], 'no binding may point at a removed field')
  assert.deepEqual(audit.adapterGapFields, [], 'no hydratable field may be adapter-blocked')
})

test('the P&S binding catalog is unique and round-trips through getPnsFieldBinding', () => {
  assert.equal(
    new Set(PNS_FIELD_BINDINGS.map((b) => b.field)).size,
    PNS_FIELD_BINDINGS.length,
    'field names must be unique',
  )
  for (const binding of PNS_FIELD_BINDINGS) {
    assert.equal(getPnsFieldBinding(binding.field), binding)
  }
  assert.equal(getPnsFieldBinding('__not_a_field__'), null)
})

test('every binding is internally consistent (owner, readiness, relations)', () => {
  for (const binding of PNS_FIELD_BINDINGS) {
    assert.ok(
      ['relation', 'property', 'contract'].includes(binding.owner),
      `${binding.field}: bad owner ${binding.owner}`,
    )
    assert.ok(
      ['clean', 'projection_pressure', 'adapter_gap'].includes(binding.readiness),
      `${binding.field}: bad readiness`,
    )
    if (binding.owner === 'relation') {
      assert.ok(binding.relations && binding.relations.length > 0, `${binding.field}: relation needs options`)
      for (const option of binding.relations) {
        assert.ok(option.scope.startsWith('contract_'), `${binding.field}: scope must be Contract-scoped`)
        assert.ok(['person', 'firm'].includes(option.target), `${binding.field}: bad target`)
        assert.ok(option.roleCode, `${binding.field}: roleCode required`)
      }
    }
  }
})

test('deal.* template sources are surfaced as legacy to retire, never silently deal-owned', () => {
  assert.ok(audit.legacyDealFields.length > 0, 'the retire list should not be empty today')
  for (const fieldName of audit.legacyDealFields) {
    assert.ok(template.fields.some((f) => f.name === fieldName), `${fieldName} must be a real template field`)
    const binding = getPnsFieldBinding(fieldName)
    assert.ok(binding, `${fieldName} must have a non-deal mapping it is retiring to`)
  }
})

test('projection-pressure fields are the real broker/escrow/notary/lender conflations', () => {
  assert.ok(audit.projectionPressureFields.length > 0)
  for (const fieldName of audit.projectionPressureFields) {
    assert.ok(template.fields.some((f) => f.name === fieldName), `${fieldName} must be a real template field`)
  }
  for (const known of ['buyerBrokerName', 'sellerBrokerName', 'lenderName']) {
    assert.ok(
      audit.projectionPressureFields.includes(known),
      `${known} should be flagged as projection pressure`,
    )
  }
})
