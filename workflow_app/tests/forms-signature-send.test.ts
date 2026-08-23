import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  formSupportsSigning,
  isActiveSigningStatus,
  isUsableSignerEmail,
  pickFormSigners,
  signingStatusLabel,
} from '../../lib/forms/signer-resolution'
import { getTemplate, PURCHASE_SALE_TEMPLATE_ID } from '../../lib/forms/template-registry'
import { applyDateDefaults } from '../../lib/forms/offer-letter-data'
import { applyGrokFields, parseGrokFillJson } from '../../lib/forms/grok-fill'

test('signing is declared on XML signature groups, not a hardcoded form list', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)
  assert.ok(template)
  assert.equal(formSupportsSigning(template), true)
  assert.ok(template.signatureGroups.some((group) => group.role === 'BUYER'))
})

test('signer email validation rejects blanks and requires a real address shape', () => {
  assert.equal(isUsableSignerEmail(''), false)
  assert.equal(isUsableSignerEmail('not-an-email'), false)
  assert.equal(isUsableSignerEmail('ana@example.com'), true)
})

test('pickFormSigners prefers CRM people for signature roles, then form fields', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)!
  const picked = pickFormSigners({
    template,
    fieldValues: { buyerName: 'Ana Rivera', sellerName: 'Marco Silva' },
    people: [
      {
        personId: 'p-ana',
        name: 'Ana Rivera',
        email: 'ana@clients.test',
        role: 'BUYER',
      },
      {
        personId: 'p-marco',
        name: 'Marco Silva',
        email: null,
        role: 'SELLER',
      },
    ],
  })
  assert.equal(picked[0]?.name, 'Ana Rivera')
  assert.equal(picked[0]?.email, 'ana@clients.test')
  assert.equal(picked.some((item) => item.name === 'Marco Silva'), true)
})

test('Grok fill JSON only writes known template fields and select options', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)!
  const parsed = parseGrokFillJson(
    '{"fieldValues":{"buyerName":"Ana Rivera","financing":"Cash","nope":"x"},"note":"Filled buyer and financing."}',
  )
  const next = applyGrokFields(
    template,
    { buyerName: '', financing: '' },
    parsed.fieldValues,
  )
  assert.equal(next.buyerName, 'Ana Rivera')
  assert.equal(next.financing, 'Cash')
  assert.equal(next.nope, undefined)
})

test('empty date fields default to a real ISO date, not a grey placeholder', () => {
  const template = getTemplate(PURCHASE_SALE_TEMPLATE_ID)!
  const filled = applyDateDefaults(template, { closingDate: '' })
  assert.match(filled.closingDate, /^\d{4}-\d{2}-\d{2}$/)
})

test('neutral signing status labels never expose provider strings', () => {
  assert.equal(signingStatusLabel('sent'), 'Sent')
  assert.equal(signingStatusLabel('completed'), 'Completed')
  assert.equal(isActiveSigningStatus('sent'), true)
  assert.equal(isActiveSigningStatus('completed'), false)
})
