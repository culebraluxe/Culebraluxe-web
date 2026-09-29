// ---------------------------------------------------------------------------
// TESTV2 — Forms MVI first pass: the four canonical form types.
//
// The Forms screen is one home with a form picker over exactly these types
// (Showing Report, Offer Letter, Purchase & Sale, Listing Contract) — P&S is a
// selection, not a separate lens. This is the pure, DB-free foundation every
// deeper form/hydration test builds on.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  getActiveTemplate,
  listPortalFormTypes,
  LISTING_AGREEMENT_TEMPLATE_ID,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  SHOWING_REPORT_TEMPLATE_ID,
} from '../lib/forms/template-registry'

const PORTAL_FORM_IDS = [
  SHOWING_REPORT_TEMPLATE_ID,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  LISTING_AGREEMENT_TEMPLATE_ID,
]

test('the portal form-type catalog lists exactly the four legal forms', () => {
  const types = listPortalFormTypes()
  assert.deepEqual(
    types.map((t) => t.id),
    PORTAL_FORM_IDS,
  )
  assert.deepEqual(
    types.map((t) => t.displayName),
    ['Showing Report', 'Offer Letter', 'Purchase & Sale', 'Listing Contract'],
  )
})

test('each portal form type has an active (openable) template', () => {
  for (const id of PORTAL_FORM_IDS) {
    const template = getActiveTemplate(id)
    assert.ok(template, `expected an active template for ${id}`)
    assert.equal(template.id, id)
    assert.ok(template.version, `expected a versioned template for ${id}`)
  }
})

test('each portal template parses XML into typed form fields (the surface the hydrate button fills)', () => {
  for (const id of PORTAL_FORM_IDS) {
    const template = getActiveTemplate(id)
    assert.ok(Array.isArray(template.fields), `${id} must expose parsed fields`)
    assert.ok(template.fields.length > 0, `${id} must parse a non-empty field set`)
    for (const field of template.fields) {
      assert.ok(field.name, `${id}: every field needs a name`)
      assert.equal(typeof field.type, 'string', `${id}:${field.name} needs a type`)
    }
  }
})

test('P&S is one of the four portal legal forms', () => {
  const types = listPortalFormTypes()
  assert.ok(types.some((t) => t.id === PURCHASE_SALE_TEMPLATE_ID), 'P&S must be a portal form type')
  assert.equal(listPortalFormTypes().length, 4)
})
