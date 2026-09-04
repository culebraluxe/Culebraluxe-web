import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { getTemplate } from '../../lib/forms/template-registry'
import { prefillFieldValues, emptySectionValues, validateFormValues } from '../../lib/forms/offer-letter-data'
import { buildOfferLetterPdf, formatMoney, formatDate } from '../../lib/forms/pdf'
import { issueFormDocument } from '../../db/issued-document'
import { listIssuedDocuments } from '../../db/transaction-document'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { FormInstance } from '../../db/document-form-instance'
import type { TxRunner } from '../../db/tx'

const OFFER_LETTER_TEMPLATE = getTemplate('OFFER-01', 1)!
const ISSUED_AT = '2026-08-22T12:00:00.000Z'
const SIGNATURE_BYTES = readFileSync(join(process.cwd(), 'public/brand/CLLOGO.png'))

test('DOC-06/07 proof 1: reference template loads through TemplateDefinition', () => {
  const template = getTemplate('OFFER-01', 1)
  assert.ok(template)
  assert.equal(template.id, 'OFFER-01')
  assert.equal(template.version, 1)
  assert.equal(template.displayName, 'Offer Letter')
  assert.equal(template.documentTypeLabel, 'Offer Letter')
  const names = template.fields.map((f) => f.name)
  for (const expected of ['buyerName', 'sellerName', 'brokerName', 'property', 'offerAmount', 'deposit', 'financing', 'closingDate', 'expiration', 'contingencies']) {
    assert.ok(names.includes(expected), `field ${expected} defined`)
  }
  assert.ok(template.sections.some((s) => s.name === 'specialTerms' && s.editable))
})

test('DOC-06/07 proof 2: canonical values prepopulate where available', () => {
  const values = prefillFieldValues(OFFER_LETTER_TEMPLATE, {
    clientName: 'Jane Buyer', propertyLabel: 'Villa Rosa', offerAmount: '1250000',
    financingType: 'Cash', closingDate: '2026-10-15',
  })
  assert.equal(values.buyerName, 'Jane Buyer')
  assert.equal(values.sellerName, '')
  assert.equal(values.brokerName, '')
  assert.equal(values.property, 'Villa Rosa')
  assert.equal(values.offerAmount, '1250000')
  assert.equal(values.financing, 'Cash')
  assert.equal(values.closingDate, '2026-10-15')
  assert.equal(values.deposit, '')
  assert.match(values.expiration, /^\d{4}-\d{2}-\d{2}$/)
  assert.equal(values.contingencies, '')
  assert.deepEqual(emptySectionValues(OFFER_LETTER_TEMPLATE), { specialTerms: '' })
})

test('DOC-06/07 proof 3: edited form state previews as a real PDF', async () => {
  const values = {
    buyerName: 'Jane Buyer', sellerName: 'Carlos Vega', brokerName: 'Lisa Penfield',
    property: 'Villa Rosa', offerAmount: '1250000', deposit: '50000', financing: 'Cash',
    closingDate: '2026-10-15', expiration: '2026-09-01', contingencies: 'Financing and inspection contingencies.',
  }
  const sections = { specialTerms: 'Closing by October 15, 2026, subject to attorney approval.' }
  const pdf = await buildOfferLetterPdf(OFFER_LETTER_TEMPLATE, values, sections, 1)
  assert.ok(pdf.toString('latin1').startsWith('%PDF-'))
  assert.ok(pdf.length > 2000)
  assert.equal(formatMoney('1250000'), '$1,250,000')
  assert.equal(formatDate('2026-10-15'), 'October 15, 2026')
  assert.equal(formatMoney(''), '')
  assert.equal(formatDate('not-a-date'), 'not-a-date')
})

type FakeState = {
  form: FormInstance | null
  prior: { id: string; issued_version: number } | null
  partyPersonId: string | null
  mediaRows: { id: string; bytes: Buffer; filename: string }[]
  docs: QueryRow[]
  supersededCalls: number
  formStatusUpdates: string[]
}

function makeState(form: FormInstance | null, prior: FakeState['prior'], partyPersonId: string | null): FakeState {
  return { form, prior, partyPersonId, mediaRows: [], docs: [], supersededCalls: 0, formStatusUpdates: [] }
}

function makeExecutor(state: FakeState): QueryExecutor {
  return async (strings, ...values): Promise<QueryRow[]> => {
    const sql = strings.join('?').toLowerCase()
    if (sql.includes('insert into workflow_command_receipt')) return [{ command_id: String(values[0]) }]
    if (sql.includes('from document_form_instance')) {
      if (!state.form) return []
      const f = state.form
      return [{ id: f.id, template_id: f.templateId, template_version: f.templateVersion, deal_id: f.dealId, person_id: f.personId ?? null, property_id: f.propertyId ?? null, status: f.status, field_values: f.fieldValues, sections: f.sections, created_by_user_id: f.createdByUserId, created_at: new Date('2026-08-22T00:00:00Z'), updated_at: new Date('2026-08-22T00:00:00Z') }]
    }
    if (sql.includes('from app_user')) {
      return [{ id: 'user-1', display_name: 'Lisa Penfield', email: 'lisa@culebraluxe.test', person_id: 'lisa-person', active: true }]
    }
    if (sql.includes('select id') && sql.includes('from media')) return [{ id: 'protected-signature-media' }]
    if (sql.includes('select file_data') && sql.includes('from media')) {
      return [{ file_data: SIGNATURE_BYTES, mime_type: 'image/png', alt_text: 'broker_signature:lisa_penfield', caption: null }]
    }
    if (sql.includes('issued_version') && sql.includes('from transaction_document') && sql.includes('where deal_id')) {
      return state.prior ? [{ id: state.prior.id, issued_version: state.prior.issued_version }] : []
    }
    if (sql.includes('insert into media')) {
      const id = `media-${state.mediaRows.length + 1}`
      state.mediaRows.push({ id, bytes: Buffer.isBuffer(values[0]) ? values[0] : Buffer.from(String(values[0])), filename: String(values[1]) })
      return [{ id }]
    }
    if (sql.includes("set state = 'superseded'")) { state.supersededCalls += 1; return [] }
    if (sql.includes('from deal_participant') && sql.includes("role = 'client'")) return state.partyPersonId ? [{ person_id: state.partyPersonId }] : []
    if (sql.includes('insert into transaction_document')) {
      const row: QueryRow = {
        id: `doc-${state.docs.length + 1}`, deal_id: values[0], document_type: values[1], document_type_label: values[2], title: values[3], state: values[4], source: values[5], source_system: values[6], source_external_id: values[7], prepared_by_user_id: values[8], party_person_id: values[9], media_id: values[10], signed_media_id: values[11], signed_at: values[12], supersedes_document_id: values[13], issued_checksum_sha256: values[14], template_id: values[15], template_version: values[16], source_snapshot: values[17], issued_version: values[18], form_instance_id: values[19], created_at: new Date('2026-08-22T00:00:00Z'), updated_at: new Date('2026-08-22T00:00:00Z'),
      }
      state.docs.push(row); return [row]
    }
    if (sql.includes('update document_form_instance')) { state.formStatusUpdates.push('issued'); return [] }
    if (sql.includes('update workflow_command_receipt')) return []
    if (sql.includes('from transaction_document td')) {
      return state.docs.map((d) => ({ id: d.id, deal_id: d.deal_id, document_type_label: d.document_type_label, title: d.title, state: d.state, template_id: d.template_id, template_version: d.template_version, issued_version: d.issued_version, issued_checksum_sha256: d.issued_checksum_sha256, issued_by_display_name: null, party_name: null, property_name: null, deal_name: null, created_at: new Date('2026-08-22T00:00:00Z') }))
    }
    return []
  }
}

function runFake(executor: QueryExecutor): TxRunner { return async (cb) => cb(executor) }

function formFixture(overrides: Partial<FormInstance> & { id: string }): FormInstance {
  return {
    id: overrides.id, templateId: overrides.templateId ?? 'OFFER-01', templateVersion: overrides.templateVersion ?? 1,
    dealId: overrides.dealId ?? 'deal-1', personId: overrides.personId ?? null, propertyId: overrides.propertyId ?? null,
    status: overrides.status ?? 'draft',
    fieldValues: overrides.fieldValues ?? { buyerName: 'Jane Buyer', sellerName: 'Carlos Vega', brokerName: 'Lisa Penfield', property: 'Villa Rosa', offerAmount: '1250000', deposit: '50000', financing: 'Cash', closingDate: '2026-10-15', expiration: '2026-09-01', contingencies: 'Financing and inspection contingencies.' },
    sections: overrides.sections ?? { specialTerms: 'Closing by October 15, 2026, subject to attorney approval.' },
    createdByUserId: overrides.createdByUserId ?? 'user-1', createdAt: overrides.createdAt ?? '2026-08-22T00:00:00Z', updatedAt: overrides.updatedAt ?? '2026-08-22T00:00:00Z',
  }
}

const sha256 = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex')
const issueInput = (commandId: string, formInstanceId: string) => ({ commandId, formInstanceId, actorAppUserId: 'user-1', issuedAt: ISSUED_AT })

test('DOC-06/07 proofs 4-7: first issuance creates a PDF, persists evidence, matches checksum, retrievable', async () => {
  const state = makeState(formFixture({ id: 'form-1' }), null, 'person-1')
  const executor = makeExecutor(state)
  const result = await issueFormDocument(issueInput('cmd-1', 'form-1'), runFake(executor))
  assert.equal(result.outcome, 'success')
  const value = result.value as { documentId: string; mediaId: string; issuedVersion: number; checksum: string }
  assert.equal(value.issuedVersion, 1)
  assert.equal(state.mediaRows.length, 1)
  const pdf = state.mediaRows[0].bytes
  assert.ok(pdf.toString('latin1').startsWith('%PDF-'))
  assert.equal(state.mediaRows[0].filename, 'offer-01-v1.pdf')
  const doc = state.docs[0]
  assert.equal(doc.document_type_label, 'Offer Letter')
  assert.equal(doc.source, 'generated')
  assert.equal(doc.state, 'ready')
  assert.equal(doc.prepared_by_user_id, 'user-1')
  assert.equal(doc.party_person_id, 'person-1')
  assert.equal(doc.template_id, 'OFFER-01')
  assert.equal(doc.template_version, 1)
  assert.equal(doc.issued_version, 1)
  assert.equal(doc.form_instance_id, 'form-1')
  const snapshot = JSON.parse(String(doc.source_snapshot))
  assert.equal(snapshot.templateId, 'OFFER-01')
  assert.equal(snapshot.fieldValues.buyerName, 'Jane Buyer')
  assert.equal(snapshot.fieldValues.offerAmount, '1250000')
  assert.equal(snapshot.sections.specialTerms, 'Closing by October 15, 2026, subject to attorney approval.')
  assert.equal(snapshot.pdfLayout.coordinateSpace, 'pdf-points-bottom-left')
  assert.ok(snapshot.pdfLayout.pageCount >= 1)
  assert.ok(snapshot.signatureAnchors.some((anchor: { role: string; kind: string }) => anchor.role === 'BUYER' && anchor.kind === 'signature'))
  assert.equal(doc.issued_checksum_sha256, sha256(pdf))
  assert.equal(doc.issued_checksum_sha256, value.checksum)
  const listing = await listIssuedDocuments(executor)
  assert.equal(listing.length, 1)
  assert.equal(listing[0].id, 'doc-1')
  assert.equal(listing[0].issuedVersion, 1)
  assert.equal(listing[0].issuedChecksumSha256, sha256(pdf))
  assert.equal(listing[0].state, 'ready')
  assert.deepEqual(state.formStatusUpdates, ['issued'])
})

test('DOC-06/07 proofs 8-9: editing + issuing again creates v2; v1 stays byte-for-byte unchanged', async () => {
  const state = makeState(formFixture({ id: 'form-1' }), null, 'person-1')
  const executor = makeExecutor(state)
  const v1 = await issueFormDocument(issueInput('cmd-1', 'form-1'), runFake(executor))
  assert.equal(v1.outcome, 'success')
  assert.equal((v1.value as { issuedVersion: number }).issuedVersion, 1)
  const v1Bytes = state.mediaRows[0].bytes; const v1Checksum = sha256(v1Bytes)
  state.form = formFixture({ id: 'form-2', fieldValues: { ...formFixture({ id: 'x' }).fieldValues, offerAmount: '1275000' } })
  state.prior = { id: state.docs[0].id as string, issued_version: 1 }
  const v2 = await issueFormDocument(issueInput('cmd-2', 'form-2'), runFake(executor))
  assert.equal(v2.outcome, 'success')
  assert.equal((v2.value as { issuedVersion: number }).issuedVersion, 2)
  assert.equal(state.mediaRows.length, 2)
  assert.equal(state.mediaRows[1].filename, 'offer-01-v2.pdf')
  const v2Checksum = sha256(state.mediaRows[1].bytes)
  const v2Doc = state.docs[state.docs.length - 1]
  assert.equal(v2Doc.issued_version, 2)
  assert.equal(v2Doc.supersedes_document_id, 'doc-1')
  assert.equal(state.supersededCalls, 1)
  assert.deepEqual(state.mediaRows[0].bytes, v1Bytes)
  const v1Doc = state.docs[0]
  assert.equal(v1Doc.issued_version, 1)
  assert.equal(v1Doc.media_id, 'media-1')
  assert.equal(v1Doc.issued_checksum_sha256, v1Checksum)
  assert.notEqual(v2Checksum, v1Checksum)
})

test('DOC-06/07 proof 10: missing required fields never issue a malformed artifact', async () => {
  const incomplete = formFixture({ id: 'form-3' })
  incomplete.fieldValues = { ...incomplete.fieldValues, buyerName: '', offerAmount: '' }
  const state = makeState(incomplete, null, 'person-1')
  const executor = makeExecutor(state)
  const result = await issueFormDocument(issueInput('cmd-3', 'form-3'), runFake(executor))
  assert.equal(result.outcome, 'validation_failure')
  assert.match(String(result.message), /required/)
  assert.equal(state.mediaRows.length, 0)
  assert.equal(state.docs.length, 0)
  assert.deepEqual(state.formStatusUpdates, [])
  const issues = validateFormValues(OFFER_LETTER_TEMPLATE, incomplete.fieldValues)
  assert.ok(issues.some((i) => i.field === 'buyerName'))
  assert.ok(issues.some((i) => i.field === 'offerAmount'))
})

test('DOC-06/07 proof 10b: saving the form again writes a new vault version', async () => {
  const state = makeState(formFixture({ id: 'form-4', status: 'issued' }), null, 'person-1')
  const executor = makeExecutor(state)
  const result = await issueFormDocument(issueInput('cmd-4', 'form-4'), runFake(executor))
  assert.equal(result.outcome, 'success')
  assert.equal(state.mediaRows.length, 1)
  assert.equal(state.docs.length, 1)
})