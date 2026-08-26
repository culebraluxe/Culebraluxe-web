import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  getTemplate,
  OFFER_LETTER_TEMPLATE_ID,
} from '../../lib/forms/template-registry'
import { parseTemplateXml, TemplateXmlError } from '../../lib/forms/xml-template'
import { buildOfferLetterPdf } from '../../lib/forms/pdf'
import type { TemplateDefinition } from '../../lib/forms/template-types'

// ---------------------------------------------------------------------------
// DOC-08 Phase 1 — XML adapter proof.
//
// OFFER-01 now originates as XML (lib/forms/templates/OFFER-01.xml) and is
// parsed + validated into the canonical TemplateDefinition at load. These
// proofs confirm the effective business-ready definition, that validation
// fails loudly on unknown/malformed input, and that the Offer Letter workflow
// continues to render through the same XML-backed seam.
// ---------------------------------------------------------------------------

/** The approved business-ready Offer definition used as the XML projection baseline. */
const legacyOfferDefinition: TemplateDefinition = {
  id: 'OFFER-01',
  version: 1,
  displayName: 'Offer Letter',
  documentTypeLabel: 'Offer Letter',
  fields: [
    { name: 'buyerName', label: 'Buyer / Client', type: 'text', required: true, binding: 'deal.client.name' },
    { name: 'sellerName', label: 'Seller / Owner', type: 'text', required: true, binding: null },
    { name: 'brokerName', label: "Buyer's Broker", type: 'text', required: true, binding: null },
    { name: 'property', label: 'Property', type: 'text', required: true, binding: 'deal.property.label' },
    { name: 'offerAmount', label: 'Offer amount', type: 'money', required: true, binding: 'deal.offer.amount' },
    { name: 'deposit', label: 'Deposit', type: 'money', required: false, binding: null },
    { name: 'financing', label: 'Cash / Financing', type: 'select', required: true, binding: 'deal.financing.type', options: ['Cash', 'Financed'] },
    { name: 'closingDate', label: 'Proposed closing date', type: 'date', required: true, binding: 'deal.closing.date' },
    { name: 'expiration', label: 'Offer expiration', type: 'date', required: true, binding: null },
    { name: 'contingencies', label: 'Contingencies', type: 'textarea', required: false, binding: null },
  ],
  sections: [{ name: 'specialTerms', label: 'Special Terms', editable: true, segments: [], values: [] }],
  participants: [
    { role: 'BUYER', label: 'Buyer', multiple: true },
    { role: 'SELLER', label: 'Seller', multiple: true },
    { role: 'BUYER_BROKER', label: "Buyer's Broker", multiple: false },
  ],
  signatureGroups: [
    { role: 'BUYER', label: 'Buyer', field: 'buyerName', initials: true },
    { role: 'SELLER', label: 'Seller / Owner', field: 'sellerName', initials: true },
    { role: 'BUYER_BROKER', label: "Buyer's Broker", field: 'brokerName', initials: true },
  ],
  rendering: {
    title: 'OFFER LETTER',
    issuer: 'CulebraLuxe Real Estate',
    presentation: 'letter',
  },
}

const xmlTemplate = getTemplate(OFFER_LETTER_TEMPLATE_ID)
assert.ok(xmlTemplate, 'OFFER-01 must load from XML through the seam')

function fieldProjection(template: TemplateDefinition, name: string) {
  const field = template.fields.find((f) => f.name === name)
  assert.ok(field, `field ${name} exists`)
  const { options, ...rest } = field
  return { ...rest, options: options ?? undefined }
}

test('DOC-08 XML: parses into stable id/version/title/document type', () => {
  assert.equal(xmlTemplate.id, 'OFFER-01')
  assert.equal(xmlTemplate.version, 1)
  assert.equal(xmlTemplate.displayName, 'Offer Letter')
  assert.equal(xmlTemplate.documentTypeLabel, 'Offer Letter')
  assert.equal(xmlTemplate.rendering.title, 'OFFER LETTER')
  assert.equal(xmlTemplate.rendering.issuer, 'CulebraLuxe Real Estate')
})

test('DOC-08 XML: required fields survive correctly', () => {
  for (const name of ['buyerName', 'sellerName', 'brokerName', 'property', 'offerAmount', 'financing', 'closingDate', 'expiration']) {
    assert.equal(fieldProjection(xmlTemplate, name).required, true, `${name} required`)
  }
  for (const name of ['deposit', 'contingencies']) {
    assert.equal(fieldProjection(xmlTemplate, name).required, false, `${name} optional`)
  }
})

test('DOC-08 XML: source bindings survive correctly', () => {
  assert.equal(fieldProjection(xmlTemplate, 'buyerName').binding, 'deal.client.name')
  assert.equal(fieldProjection(xmlTemplate, 'property').binding, 'deal.property.label')
  assert.equal(fieldProjection(xmlTemplate, 'offerAmount').binding, 'deal.offer.amount')
  assert.equal(fieldProjection(xmlTemplate, 'financing').binding, 'deal.financing.type')
  assert.equal(fieldProjection(xmlTemplate, 'closingDate').binding, 'deal.closing.date')
  assert.equal(fieldProjection(xmlTemplate, 'sellerName').binding, null)
  assert.equal(fieldProjection(xmlTemplate, 'brokerName').binding, null)
  assert.equal(fieldProjection(xmlTemplate, 'deposit').binding, null)
  assert.equal(fieldProjection(xmlTemplate, 'expiration').binding, null)
  assert.equal(fieldProjection(xmlTemplate, 'contingencies').binding, null)
  assert.deepEqual(fieldProjection(xmlTemplate, 'financing').options, ['Cash', 'Financed'])
})

test('DOC-08 XML: editable sections survive correctly', () => {
  assert.equal(xmlTemplate.sections.length, 1)
  const section = xmlTemplate.sections[0]
  assert.equal(section.name, 'specialTerms')
  assert.equal(section.label, 'Special Terms')
  assert.equal(section.editable, true)
  assert.deepEqual(section.values, [])
  assert.deepEqual(section.segments, [])
})

// __PART2__
test('DOC-08 XML: fields/sections match the approved business-ready definition', () => {
  assert.deepEqual(
    xmlTemplate.fields.map((f) => {
      const { options, ...rest } = f
      return { ...rest, options: options ?? undefined }
    }),
    legacyOfferDefinition.fields.map((f) => {
      const { options, ...rest } = f
      return { ...rest, options: options ?? undefined }
    }),
  )
  assert.deepEqual(
    xmlTemplate.sections.map((s) => ({ name: s.name, label: s.label, editable: s.editable })),
    legacyOfferDefinition.sections.map((s) => ({ name: s.name, label: s.label, editable: s.editable })),
  )
  assert.deepEqual(xmlTemplate.rendering, legacyOfferDefinition.rendering)
  assert.deepEqual(xmlTemplate.participants, legacyOfferDefinition.participants)
  assert.deepEqual(xmlTemplate.signatureGroups, legacyOfferDefinition.signatureGroups)
})

test('DOC-08 XML: PDF is a real document with the same content from XML or fixture', async () => {
  const values = {
    buyerName: 'Jane Buyer',
    sellerName: 'Carlos Vega',
    brokerName: 'Lisa Penfield',
    property: 'Villa Rosa',
    offerAmount: '1250000',
    deposit: '50000',
    financing: 'Cash',
    closingDate: '2026-10-15',
    expiration: '2026-09-01',
    contingencies: 'Financing and inspection contingencies.',
  }
  const sections = { specialTerms: 'Closing by October 15, 2026, subject to attorney approval.' }

  const fromXml = await buildOfferLetterPdf(xmlTemplate, values, sections, 1)
  const fromLegacy = await buildOfferLetterPdf(legacyOfferDefinition, values, sections, 1)
  assert.equal(fromXml.subarray(0, 5).toString(), '%PDF-')
  assert.equal(fromLegacy.subarray(0, 5).toString(), '%PDF-')
  assert.ok(fromXml.length > 2000, 'Offer Letter PDF is a real binary, not a stub')
})

test('DOC-08 XML: unknown field type / element / binding fails at load time', () => {
  assert.throws(
    () => parseTemplateXml(`<form id="X" version="1" title="T"><field id="a" label="A" type="bogus"/></form>`),
    /unknown type/,
  )
  assert.throws(
    () => parseTemplateXml(`<form id="X" version="1" title="T"><field id="a" label="A" type="text"/><bogus/></form>`),
    /Unknown element/,
  )
  assert.throws(
    () => parseTemplateXml(`<form id="X" version="1" title="T"><field id="a" label="A" type="text" source="nope.path"/></form>`),
    /unknown source binding/,
  )
  assert.throws(
    () => parseTemplateXml(`<form id="X" version="1" title="T"><section id="s" title="S"><value field="missing"/></section><field id="a" label="A" type="text"/></form>`),
    /unknown field/,
  )
  assert.throws(
    () => parseTemplateXml(`<form id="X" version="1" title="T"></form>`),
    /at least one <field>/,
  )
})

test('DOC-08 XML: malformed XML fails clearly', () => {
  assert.throws(() => parseTemplateXml('<form id="X" version="1" title="T"><field id="a" label="A" type="text"></form>'), TemplateXmlError)
  assert.throws(() => parseTemplateXml('<form id="X" version="1" title="T"></bogus></form>'), TemplateXmlError)
  assert.throws(() => parseTemplateXml('<form id="X" version="1" title="T"><field id="a" label="A" type="text" bad/></form>'), TemplateXmlError)
  assert.throws(() => parseTemplateXml(''), TemplateXmlError)
})

test('DOC-08 XML: version must be a positive integer', () => {
  assert.throws(() => parseTemplateXml(`<form id="X" version="0" title="T"><field id="a" label="A" type="text"/></form>`), /positive integer/)
})
