import test from 'node:test'
import assert from 'node:assert/strict'

import {
  ACTIVE_TEMPLATE_VERSIONS,
  getActiveTemplate,
  getLatestTemplate,
  getTemplate,
  listTemplates,
  LISTING_AGREEMENT_TEMPLATE_ID,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
} from '../../lib/forms/template-registry'

// ---------------------------------------------------------------------------
// FORMS TEMPLATE VERSIONING — the registry guarantee that matters.
//
// The rule (lib/forms/template-registry.ts): keep one ACTIVE version per form
// family, PLUS every version a persisted record still points at. A document
// re-renders from its template file by (templateId, templateVersion), so
// removing a version that a live record references makes that record
// unresolvable.
//
// PERSISTED_VERSIONS below is the real PROD reference set:
//
//   select template_id, template_version, count(*)::int
//     from transaction_document group by 1, 2 order by 1, 2
//
// 2026-09-11: LISTING-01 v2 (6 documents) and PR-PNS v1 (3 documents) had been
// removed as "superseded" while PROD still referenced them. This proof is why
// they were restored — re-run that query before deleting any version.
// ---------------------------------------------------------------------------

/** [templateId, templateVersion, issued documents in PROD]. */
const PERSISTED_VERSIONS: readonly (readonly [string, number, number])[] = [
  [LISTING_AGREEMENT_TEMPLATE_ID, 2, 6],
  [LISTING_AGREEMENT_TEMPLATE_ID, 3, 19],
  [LISTING_AGREEMENT_TEMPLATE_ID, 4, 8],
  [PURCHASE_SALE_TEMPLATE_ID, 1, 3],
  [PURCHASE_SALE_TEMPLATE_ID, 3, 8],
  ['SHOW-RPT', 1, 1],
]

test('FORMS-VER: every template version a PROD record references still resolves', () => {
  for (const [id, version, documents] of PERSISTED_VERSIONS) {
    const template = getTemplate(id, version)
    assert.ok(template, `${id} v${version} (${documents} issued documents) must resolve`)
    assert.equal(template.id, id)
    assert.equal(template.version, version)
  }
})

test('FORMS-VER: exact lookup returns the requested version, never a substitute', () => {
  for (const [id, version] of PERSISTED_VERSIONS) {
    assert.equal(getTemplate(id, version)?.version, version, `${id} v${version}`)
  }
})

test('FORMS-VER: new forms use the active version, and it is the latest kept', () => {
  for (const id of [PURCHASE_SALE_TEMPLATE_ID, LISTING_AGREEMENT_TEMPLATE_ID, OFFER_LETTER_TEMPLATE_ID]) {
    const active = getActiveTemplate(id)
    assert.ok(active, `${id} has an active version`)
    assert.equal(active.version, ACTIVE_TEMPLATE_VERSIONS[id], `${id} active version matches the manifest`)
    assert.equal(active.version, getLatestTemplate(id)?.version, `${id} active is the newest kept version`)
  }
})

test('FORMS-VER: unknown persisted versions fail closed', () => {
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 0), null)
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 999), null)
})

test('FORMS-VER: no duplicate (id, version) pair is registered', () => {
  const seen = new Set<string>()
  for (const template of listTemplates()) {
    const key = `${template.id}@${template.version}`
    assert.equal(seen.has(key), false, `duplicate registration for ${key}`)
    seen.add(key)
  }
})

test('FORMS-VER: the Purchase and Sale Agreement keeps its full body', () => {
  const active = getActiveTemplate(PURCHASE_SALE_TEMPLATE_ID)
  assert.ok(active)
  assert.equal(active.rendering.title, 'PURCHASE AND SALE AGREEMENT')
  assert.ok(active.fields.length >= 16, 'agreement keeps its structured fields')
  assert.ok(active.sections.length >= 15, 'agreement keeps its full section set')
})

test('FORMS-VER: the Listing Agreement keeps its section body', () => {
  const active = getActiveTemplate(LISTING_AGREEMENT_TEMPLATE_ID)
  assert.ok(active)
  assert.ok(active.sections.length >= 10, 'listing agreement keeps its sections')
})
