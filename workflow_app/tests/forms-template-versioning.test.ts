import test from 'node:test'
import assert from 'node:assert/strict'

import {
  getTemplate,
  getLatestTemplate,
  resolveLatestTemplateVersion,
  listTemplates,
  PURCHASE_SALE_TEMPLATE_ID,
} from '../../lib/forms/template-registry'
import { compareTemplateStructure } from '../../lib/forms/template-versioning'
import type {
  TemplateDefinition,
  TemplateFieldDefinition,
  TemplateParticipantRole,
  TemplateSectionDefinition,
  TemplateSignatureGroup,
} from '../../lib/forms/template-types'

// ---------------------------------------------------------------------------
// FORMS TEMPLATE VERSIONING — registry + structural compatibility proofs.
// ---------------------------------------------------------------------------

const BASE_FIELD: TemplateFieldDefinition = {
  name: 'buyerName',
  label: 'Buyer',
  type: 'text',
  required: true,
  binding: 'deal.client.name',
}

const BASE_SECTION: TemplateSectionDefinition = {
  name: 'terms',
  label: 'Terms',
  editable: false,
  segments: [{ kind: 'text', text: 'The parties agree to the following terms.' }],
  values: [],
}

const BASE_PARTICIPANT: TemplateParticipantRole = {
  role: 'SELLER',
  label: 'Seller',
  multiple: false,
}

const BASE_SIGNATURE: TemplateSignatureGroup = {
  role: 'SELLER',
  label: 'Seller',
  field: 'sellerName',
  initials: true,
}

function makeTemplate(overrides: Partial<TemplateDefinition> = {}): TemplateDefinition {
  return {
    id: 'PR-PNS',
    version: 1,
    displayName: 'Purchase and Sale Agreement',
    documentTypeLabel: 'Purchase and Sale Agreement',
    fields: [{ ...BASE_FIELD }],
    sections: [{ ...BASE_SECTION }],
    participants: [{ ...BASE_PARTICIPANT }],
    signatureGroups: [{ ...BASE_SIGNATURE }],
    rendering: { title: 'PURCHASE AND SALE AGREEMENT', issuer: 'CulebraLuxe Real Estate', presentation: 'agreement' },
    ...overrides,
  }
}

// --- Registry: coexistence, exact lookup, latest ----------------------------

test('FORMS-VER 1: registry holds PR-PNS v1 and v2 simultaneously', () => {
  const versions = listTemplates()
    .filter((t) => t.id === PURCHASE_SALE_TEMPLATE_ID)
    .map((t) => t.version)
    .sort((a, b) => a - b)
  assert.deepEqual(versions, [1, 2])
})

test('FORMS-VER 2: exact lookup PR-PNS+1 -> v1, PR-PNS+2 -> v2', () => {
  const v1 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)
  const v2 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 2)
  assert.ok(v1)
  assert.ok(v2)
  assert.equal(v1.version, 1)
  assert.equal(v2.version, 2)
  assert.notEqual(v1, v2, 'v1 and v2 are distinct definitions')
})

test('FORMS-VER 3: latest lookup returns v2', () => {
  assert.equal(getLatestTemplate(PURCHASE_SALE_TEMPLATE_ID)?.version, 2)
})

test('FORMS-VER 4: when only v1 exists, latest resolves to v1 (new form stores v1)', () => {
  const v1 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)!
  const onlyV1 = [v1]
  assert.equal(resolveLatestTemplateVersion(onlyV1, PURCHASE_SALE_TEMPLATE_ID)?.version, 1)
})

test('FORMS-VER 5: after v2 exists, a NEW form resolves the latest (v2)', () => {
  assert.equal(getLatestTemplate(PURCHASE_SALE_TEMPLATE_ID)?.version, 2)
})

test('FORMS-VER 6: an existing v1 form still resolves v1 after v2 exists', () => {
  // Even though v2 is now latest, the exact v1 lookup must still return v1.
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)?.version, 1)
})

test('FORMS-VER 7: v1 preview after v2 exists still uses the v1 definition', () => {
  const v1 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)!
  const v2 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 2)!
  assert.equal(v1.version, 1)
  assert.notEqual(v1, v2)
  assert.equal(v1.rendering.title, 'PURCHASE AND SALE AGREEMENT')
  assert.ok(v1.sections.length >= 15, 'v1 keeps its full section set')
})

test('FORMS-VER 8/9: issuance resolves the exact stored version (v1 -> 1, v2 -> 2)', () => {
  // db/issued-document.ts resolves getTemplate(form.templateId, form.templateVersion),
  // so the issued artifact's templateVersion equals the exact resolved template.
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)?.version, 1)
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 2)?.version, 2)
})

test('FORMS-VER 10: unknown persisted template version fails closed (null)', () => {
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 999), null)
  assert.equal(getTemplate(PURCHASE_SALE_TEMPLATE_ID, 0), null)
})

test('FORMS-VER 11: previously issued v1 definition remains unchanged', () => {
  // The v1 file/definition is preserved exactly (not overwritten by v2).
  const v1 = getTemplate(PURCHASE_SALE_TEMPLATE_ID, 1)!
  assert.equal(v1.version, 1)
  assert.equal(v1.fields.length, 16)
  assert.ok(v1.fields.some((f) => f.name === 'buyerName'))
})

// --- Structural compatibility validator -------------------------------------

test('FORMS-VER 12: validator accepts a prose-only revision', () => {
  const prev = makeTemplate()
  const next = makeTemplate({
    sections: [
      {
        ...BASE_SECTION,
        segments: [{ kind: 'text', text: 'The parties agree to the following revised terms.' }],
      },
    ],
  })
  const report = compareTemplateStructure(prev, next)
  assert.equal(report.contentChanged, true, 'prose differs')
  assert.equal(report.participants, 'unchanged')
  assert.equal(report.canonicalFields, 'unchanged')
  assert.equal(report.signatureGroups, 'unchanged')
  assert.equal(report.executionStructure, 'unchanged')
  assert.equal(report.compatible, true, 'prose-only revision is structurally compatible')
})

test('FORMS-VER 13: validator rejects a participant-role change', () => {
  const prev = makeTemplate()
  const next = makeTemplate({
    participants: [
      { ...BASE_PARTICIPANT },
      { role: 'BUYER', label: 'Buyer', multiple: false },
    ],
  })
  const report = compareTemplateStructure(prev, next)
  assert.ok(Array.isArray(report.participants), 'participant change reported')
  assert.equal(report.compatible, false, 'participant set change is not compatible')
})

test('FORMS-VER 13b: validator rejects a participant multiplicity change', () => {
  const prev = makeTemplate()
  const next = makeTemplate({
    participants: [{ ...BASE_PARTICIPANT, multiple: true }],
  })
  const report = compareTemplateStructure(prev, next)
  assert.ok(Array.isArray(report.participants), 'participant change reported')
  assert.equal(report.compatible, false)
})

test('FORMS-VER 14: validator rejects a signature-group change', () => {
  const prev = makeTemplate()
  const next = makeTemplate({
    signatureGroups: [{ ...BASE_SIGNATURE, initials: false }],
  })
  const report = compareTemplateStructure(prev, next)
  assert.ok(Array.isArray(report.signatureGroups), 'signature change reported')
  assert.equal(report.compatible, false)
})

test('FORMS-VER 15: validator rejects a canonical field/binding change', () => {
  const prev = makeTemplate()
  const next = makeTemplate({
    fields: [{ ...BASE_FIELD, binding: 'person.displayName' }],
  })
  const report = compareTemplateStructure(prev, next)
  assert.ok(Array.isArray(report.canonicalFields), 'field/binding change reported')
  assert.equal(report.compatible, false)

  const nextType = makeTemplate({ fields: [{ ...BASE_FIELD, type: 'money' }] })
  const reportType = compareTemplateStructure(prev, nextType)
  assert.ok(Array.isArray(reportType.canonicalFields))
  assert.equal(reportType.compatible, false)
})

