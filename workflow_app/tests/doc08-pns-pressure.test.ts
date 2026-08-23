import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  getTemplate,
  PURCHASE_SALE_TEMPLATE_ID,
} from '../../lib/forms/template-registry'
import { buildPurchaseSalePdf, interpolateSectionText } from '../../lib/forms/pdf'

// ---------------------------------------------------------------------------
// DOC-08 Phase 2 — P&S pressure test.
//
// The question: can the tiny XML vocabulary express the hard document (a
// representative Puerto Rico Purchase and Sale Agreement)? This suite proves
// the PR-PNS.xml template loads through the seam with structured parties,
// property identifiers, price/deposit, financing/appraisal/survey/inspection,
// stable boilerplate + broad negotiated sections, and role-driven signature
// groups with initials — and renders deterministically across multiple pages.
// ---------------------------------------------------------------------------

const pns = getTemplate(PURCHASE_SALE_TEMPLATE_ID)
assert.ok(pns, 'PR-PNS must load from XML through the seam')

const section = (name: string) => {
  const s = pns.sections.find((x) => x.name === name)
  assert.ok(s, `section ${name} exists`)
  return s
}

test('DOC-08 P&S: template identity and shape survive', () => {
  assert.equal(pns.id, 'PR-PNS')
  assert.equal(pns.version, 1)
  assert.equal(pns.displayName, 'Purchase and Sale Agreement')
  assert.equal(pns.rendering.title, 'PURCHASE AND SALE AGREEMENT')
  assert.ok(pns.fields.length >= 16, 'structured fields declared')
  assert.ok(pns.sections.length >= 15, 'sections declared')
})

test('DOC-08 P&S: structured parties are role-driven, not buyer1/buyer2', () => {
  const roles = pns.participants.map((p) => p.role)
  for (const expected of ['BUYER', 'SELLER', 'BUYER_BROKER', 'SELLER_BROKER']) {
    assert.ok(roles.includes(expected), `role ${expected} declared`)
  }
  assert.equal(pns.participants.find((p) => p.role === 'BUYER')?.multiple, true)
  assert.equal(pns.participants.find((p) => p.role === 'SELLER')?.multiple, true)
  assert.equal(pns.participants.find((p) => p.role === 'SELLER_BROKER')?.multiple, false)
})

test('DOC-08 P&S: signature groups carry roles, name fields and initials', () => {
  const groups = pns.signatureGroups
  assert.equal(groups.length, 3)
  const buyer = groups.find((g) => g.role === 'BUYER')
  assert.ok(buyer)
  assert.equal(buyer.initials, true)
  assert.equal(buyer.field, 'buyerName')
  const seller = groups.find((g) => g.role === 'SELLER')
  assert.ok(seller)
  assert.equal(seller.initials, true)
  const broker = groups.find((g) => g.role === 'SELLER_BROKER')
  assert.ok(broker)
  assert.equal(broker.initials, false)
  for (const g of groups) {
    if (g.field) assert.ok(pns.fields.some((f) => f.name === g.field))
  }
})

test('DOC-08 P&S: stable boilerplate vs broad negotiated sections', () => {
  for (const name of ['titleInsurance', 'taxes', 'closingCosts', 'risk', 'default', 'notices', 'governingLaw']) {
    assert.equal(section(name).editable, false, `${name} boilerplate`)
  }
  for (const name of ['financingTerms', 'appraisalSurveyInspection', 'additionalTerms', 'specialConditions']) {
    assert.equal(section(name).editable, true, `${name} negotiated`)
  }
})

test('DOC-08 P&S: structured facts bind into boilerplate prose via <value>', () => {
  const parties = section('parties')
  assert.deepEqual(parties.values, ['buyerName', 'sellerName'])
  assert.deepEqual(section('propertyDescription').values, ['property', 'municipality', 'catastroNumber', 'registryEntry'])
  assert.deepEqual(section('purchasePrice').values, ['purchasePrice', 'deposit'])
  assert.deepEqual(section('closing').values, ['closingDate'])

  const text = interpolateSectionText(
    parties,
    { buyerName: 'Ana Rivera', sellerName: 'Marco Silva' },
    (field, raw) => raw,
    pns.fields,
  )
  assert.ok(text.includes('Ana Rivera'))
  assert.ok(text.includes('Marco Silva'))
  assert.ok(text.includes('the "Buyer"'))

  // <value> is a declarative substitution — no expression constructs exist.
  assert.ok(!text.includes('if('))
  assert.ok(!text.includes('${'))
})

// __PART2__
test('DOC-08 P&S: multi-page deterministic rendering', async () => {
  const values = {
    buyerName: 'Ana Rivera',
    sellerName: 'Marco Silva',
    buyerBrokerName: 'Carla Ortiz',
    sellerBrokerName: 'Luis Vega',
    property: 'Casa Luar, Carr 446, Rincon',
    municipality: 'Rincon',
    catastroNumber: '123-456-789-00-001',
    registryEntry: 'Rincon 4,231',
    purchasePrice: '1250000',
    deposit: '50000',
    closingDate: '2026-11-15',
    financing: 'Cash',
    financingDeadline: '2026-10-01',
    appraisalWaived: 'No',
    surveyDeadline: '2026-10-10',
    inspectionDeadline: '2026-10-20',
  }
  const sections: Record<string, string> = {
    financingTerms:
      'The Buyer will pay the purchase price in cash at closing. No financing ' +
      'contingency applies to this transaction.',
    appraisalSurveyInspection:
      'The Buyer shall have the right to obtain an appraisal of the Property ' +
      'and a survey at Buyer expense, and to complete an inspection within the ' +
      'inspection period. Any defects found shall be resolved as mutually agreed.',
    additionalTerms:
      'The parties agree that all personal property located on the Property and ' +
      'listed in an attached inventory shall be conveyed at closing without ' +
      'additional consideration, and that the Seller shall deliver all keys, ' +
      'codes and access credentials on the closing date.',
    specialConditions:
      'Closing is expressly conditioned on the availability of the Property ' +
      'registry certification and on the notary public selected by the parties ' +
      'completing the deed in accordance with applicable law.',
  }

  const pdf = await buildPurchaseSalePdf(pns, values, sections, 1)
  assert.ok(pdf.toString('latin1').startsWith('%PDF-'), 'valid PDF header')
  assert.ok(pdf.length > 2000, 'P&S PDF is a real binary document')

  const { PDFDocument } = await import('pdf-lib')
  const loaded = await PDFDocument.load(pdf)
  assert.ok(loaded.getPageCount() >= 2, `P&S renders on ${loaded.getPageCount()} pages`)
})

test('DOC-08 P&S: no conditional primitive is required by this template', () => {
  // Every contingency/financing/survey/inspection concern is either a
  // structured field or negotiated prose — no conditional section exists.
  for (const s of pns.sections) {
    assert.equal(typeof (s as unknown as { if?: unknown }).if, 'undefined')
  }
})

test('DOC-08 P&S: XML model bends nowhere — a malformed P&S variant is rejected', () => {
  const { parseTemplateXml } =
    require('../../lib/forms/xml-template') as typeof import('../../lib/forms/xml-template')
  // Signature group referencing an undeclared field fails at load time.
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="PNS-BAD" version="1" title="T"><field id="a" label="A" type="text"/>
         <signatures><signature-group role="BUYER" field="nope"/></signatures></form>`,
      ),
    /unknown field/,
  )
  // An unknown element anywhere inside the form is rejected.
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="PNS-BAD" version="1" title="T"><field id="a" label="A" type="text"/>
         <script>alert(1)</script></form>`,
      ),
    /Unknown element/,
  )
})

