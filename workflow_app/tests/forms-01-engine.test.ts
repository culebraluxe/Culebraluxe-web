import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { createHash } from 'node:crypto'

import { parseTemplateXml, TemplateXmlError } from '../../lib/forms/xml-template'
import {
  getTemplate,
  listTemplates,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
  LISTING_AGREEMENT_TEMPLATE_ID,
  SHOWING_INFO_TEMPLATE_ID,
  SHOWING_REPORT_TEMPLATE_ID,
} from '../../lib/forms/template-registry'
import { renderFormPdf, buildOfferLetterPdf } from '../../lib/forms/pdf'
import { prefillFieldValues, emptySectionValues } from '../../lib/forms/offer-letter-data'

test('FORMS-01: all production templates load and validate', () => {
  const ids = [
    OFFER_LETTER_TEMPLATE_ID,
    PURCHASE_SALE_TEMPLATE_ID,
    PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
    LISTING_AGREEMENT_TEMPLATE_ID,
    SHOWING_INFO_TEMPLATE_ID,
    SHOWING_REPORT_TEMPLATE_ID,
  ]
  const loaded = listTemplates()
  for (const id of ids) {
    const template = getTemplate(id)
    assert.ok(template, `${id} loads`)
    assert.ok(template.fields.length > 0, `${id} has fields`)
    assert.equal(loaded.some((item) => item.id === id), true)
  }
})

test('FORMS-01: P&S has role collections and signature groups', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)
  assert.ok(template)
  assert.ok(template.participants.some((p) => p.role === 'BUYER' && p.multiple))
  assert.ok(template.participants.some((p) => p.role === 'SELLER' && p.multiple))
  assert.ok(template.signatureGroups.some((g) => g.role === 'BUYER'))
  assert.ok(template.signatureGroups.some((g) => g.role === 'SELLER'))
})

test('FORMS-01: XML rejects unknown bindings and duplicate fields', () => {
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="X" version="1" title="X"><field id="a" label="A" type="text" source="nope.path"/></form>`,
      ),
    TemplateXmlError,
  )
  assert.throws(
    () =>
      parseTemplateXml(
        `<form id="X" version="1" title="X"><field id="a" label="A" type="text"/><field id="a" label="B" type="text"/></form>`,
      ),
    TemplateXmlError,
  )
})

test('FORMS-01: unified renderer matches Offer Letter byte identity', () => {
  const template = getTemplate(OFFER_LETTER_TEMPLATE_ID)!
  const values = prefillFieldValues(template, {
    clientName: 'James Lee',
    propertyLabel: 'Sunset Point',
    offerAmount: '3950000',
    financingType: 'Cash',
    closingDate: '2026-11-15',
  })
  values.expiration = '2026-09-01'
  const sections = emptySectionValues(template)
  const a = renderFormPdf(template, values, sections, 1)
  const b = buildOfferLetterPdf(template, values, sections, 1)
  assert.equal(a.equals(b), true)
  assert.equal(a.subarray(0, 5).toString(), '%PDF-')
})

test('FORMS-01: P&S preview and issuance share renderer and paginate', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)!
  const values = prefillFieldValues(template, {
    clientName: 'James Lee',
    propertyLabel: 'Sunset Point, Culebra',
    offerAmount: '3950000',
    financingType: 'Financed',
    closingDate: '2026-11-15',
  })
  values.sellerName = 'Casa Luar LLC'
  values.deposit = '100000'
  values.municipality = 'Culebra'
  values.catastroNumber = '123-45'
  values.registryEntry = 'F-99'
  const sections = emptySectionValues(template)
  const preview = renderFormPdf(template, values, sections, 0)
  const issued = renderFormPdf(template, values, sections, 1)
  assert.equal(preview.subarray(0, 5).toString(), '%PDF-')
  assert.equal(issued.subarray(0, 5).toString(), '%PDF-')
  const countPages = (buf: Buffer) => {
    const text = buf.toString('latin1')
    const match = /\/Count (\d+)/.exec(text)
    return match ? Number(match[1]) : 0
  }
  assert.ok(countPages(issued) >= 2, 'P&S must be multi-page')
  assert.notEqual(
    createHash('sha256').update(preview).digest('hex'),
    createHash('sha256').update(issued).digest('hex'),
  )
})

test('FORMS-01: remaining production templates render PDFs', () => {
  for (const id of [
    PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
    LISTING_AGREEMENT_TEMPLATE_ID,
    SHOWING_INFO_TEMPLATE_ID,
    SHOWING_REPORT_TEMPLATE_ID,
  ]) {
    const template = getTemplate(id)!
    const values: Record<string, string> = {}
    for (const field of template.fields) {
      values[field.name] = field.type === 'date' ? '2026-09-01' : 'Test'
    }
    const pdf = renderFormPdf(template, values, emptySectionValues(template), 1)
    assert.equal(pdf.subarray(0, 5).toString(), '%PDF-', id)
  }
})

test('FORMS-01: on-disk XML is the authoring source', () => {
  const xml = readFileSync(
    join(process.cwd(), 'lib/forms/templates/PR-PNS.xml'),
    'utf8',
  )
  const parsed = parseTemplateXml(xml)
  assert.equal(parsed.id, 'PR-PNS')
  assert.ok(parsed.sections.length >= 10)
})
