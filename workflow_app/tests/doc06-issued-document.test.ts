import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'

import { getTemplate } from '../../lib/forms/template-registry'
import {
  prefillFieldValues,
  emptySectionValues,
  validateFormValues,
} from '../../lib/forms/offer-letter-data'
import { buildOfferLetterPdf, formatMoney, formatDate } from '../../lib/forms/pdf'
import { issueFormDocument } from '../../db/issued-document'
import { listIssuedDocuments } from '../../db/transaction-document'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { FormInstance } from '../../db/document-form-instance'
import type { TxRunner } from '../../db/tx'

// DOC-08: the Offer Letter now originates from XML (lib/forms/templates/
// OFFER-01.xml) through the TemplateDefinition seam; these proofs exercise the
// same effective definition the UI/renderer/issuance consume.
const OFFER_LETTER_TEMPLATE = getTemplate('OFFER-01')!

// ---------------------------------------------------------------------------
// DOC-06 / DOC-07 — POC proof suite.
//
//   1. reference template loads through the TemplateDefinition abstraction
//   2. canonical CRM/deal/property values prepopulate where available
//   3. edited form state can preview (deterministic PDF render)
//   4. issuing creates a PDF
//   5. issued metadata + exact source snapshot persist
//   6. checksum matches the stored canonical artifact
//   7. repository lookup returns the issued document
//   8. editing source + issuing again creates v2 (supersedes v1)
//   9. v1 remains unchanged and retrievable
//  10. invalid/missing required fields never issue a malformed artifact
// ---------------------------------------------------------------------------

// --------------------------- pure proofs ----------------------------------

test('DOC-06/07 proof 1: reference template loads through TemplateDefinition', () => {
  const template = getTemplate('OFFER-01')
  assert.ok(template, 'OFFER-01 must be resolvable through the seam')
  assert.equal(template.id, 'OFFER-01')
  assert.equal(template.version, 1)
  assert.equal(template.displayName, 'Offer Letter')
  assert.equal(template.documentTypeLabel, 'Offer Letter')

  const names = template.fields.map((f) => f.name)
  for (const expected of [
    'buyerName', 'property', 'offerAmount', 'deposit', 'financing',
    'closingDate', 'expiration', 'contingencies',
  ]) {
    assert.ok(names.includes(expected), `field ${expected} defined`)
  }
  assert.ok(template.sections.some((s) => s.name === 'specialTerms' && s.editable))
})

test('DOC-06/07 proof 2: canonical values prepopulate where available', () => {
  const values = prefillFieldValues(OFFER_LETTER_TEMPLATE, {
    clientName: 'Jane Buyer',
    propertyLabel: 'Villa Rosa',
    offerAmount: '1250000',
    financingType: 'Cash',
    closingDate: '2026-10-15',
  })
  assert.equal(values.buyerName, 'Jane Buyer')
  assert.equal(values.property, 'Villa Rosa')
  assert.equal(values.offerAmount, '1250000')
  assert.equal(values.financing, 'Cash')
  assert.equal(values.closingDate, '2026-10-15')
  // Unbound fields stay blank (user-entered) — the domain is not broadened.
  assert.equal(values.deposit, '')
  assert.equal(values.expiration, '')
  assert.equal(values.contingencies, '')
  const sections = emptySectionValues(OFFER_LETTER_TEMPLATE)
  assert.deepEqual(sections, { specialTerms: '' })
})

test('DOC-06/07 proof 3: edited form state previews as a deterministic PDF', () => {
  const values = {
    buyerName: 'Jane Buyer',
    property: 'Villa Rosa',
    offerAmount: '1250000',
    deposit: '50000',
    financing: 'Cash',
    closingDate: '2026-10-15',
    expiration: '2026-09-01',
    contingencies: 'Financing and inspection contingencies.',
  }
  const sections = { specialTerms: 'Closing by October 15, 2026, subject to attorney approval.' }
  const pdf = buildOfferLetterPdf(OFFER_LETTER_TEMPLATE, values, sections, 1)
  const again = buildOfferLetterPdf(OFFER_LETTER_TEMPLATE, values, sections, 1)

  assert.ok(pdf.toString('latin1').startsWith('%PDF-1.4'), 'valid PDF header')
  assert.ok(pdf.toString('latin1').includes('OFFER LETTER'))
  assert.ok(pdf.toString('latin1').includes('Jane Buyer'))
  assert.ok(pdf.toString('latin1').includes('$1,250,000'), 'money formatted')
  assert.ok(pdf.toString('latin1').includes('October 15, 2026'), 'date formatted')
  assert.ok(pdf.toString('latin1').includes('v1'), 'version drawn on artifact')
  // Determinism: identical input → identical bytes (the checksum invariant).
  assert.deepEqual(pdf, again)
  // Editing the source changes the bytes (a v2 would differ).
  const edited = buildOfferLetterPdf(
    OFFER_LETTER_TEMPLATE,
    { ...values, offerAmount: '1275000' },
    sections,
    2,
  )
  assert.notDeepEqual(edited, pdf)

  assert.equal(formatMoney('1250000'), '$1,250,000')
  assert.equal(formatDate('2026-10-15'), 'October 15, 2026')
  assert.equal(formatMoney(''), '')
  assert.equal(formatDate('not-a-date'), 'not-a-date')
})

// __PART2__
type FakeState = {
  form: FormInstance | null
  prior: { id: string; issued_version: number } | null
  partyPersonId: string | null
  mediaRows: { id: string; bytes: Buffer; filename: string }[]
  docs: QueryRow[]
  supersededCalls: number
  formStatusUpdates: string[]
}

function makeState(
  form: FormInstance | null,
  prior: FakeState['prior'],
  partyPersonId: string | null,
): FakeState {
  return {
    form,
    prior,
    partyPersonId,
    mediaRows: [],
    docs: [],
    supersededCalls: 0,
    formStatusUpdates: [],
  }
}

function makeExecutor(state: FakeState): QueryExecutor {
  return async (strings, ...values): Promise<QueryRow[]> => {
    const sql = strings.join('?').toLowerCase()

    if (sql.includes('insert into workflow_command_receipt')) {
      return [{ command_id: String(values[0]) }]
    }
    if (sql.includes('from document_form_instance')) {
      if (!state.form) return []
      const f = state.form
      return [{
        id: f.id, template_id: f.templateId, template_version: f.templateVersion,
        deal_id: f.dealId, person_id: f.personId ?? null, property_id: f.propertyId ?? null,
        status: f.status, field_values: f.fieldValues,
        sections: f.sections, created_by_user_id: f.createdByUserId,
        created_at: new Date('2026-08-22T00:00:00Z'),
        updated_at: new Date('2026-08-22T00:00:00Z'),
      }]
    }
    if (sql.includes('issued_version') && sql.includes('from transaction_document') && sql.includes('where deal_id')) {
      return state.prior ? [{ id: state.prior.id, issued_version: state.prior.issued_version }] : []
    }
    if (sql.includes('insert into media')) {
      const id = `media-${state.mediaRows.length + 1}`
      state.mediaRows.push({
        id,
        bytes: Buffer.isBuffer(values[0]) ? values[0] : Buffer.from(String(values[0])),
        filename: String(values[1]),
      })
      return [{ id }]
    }
    if (sql.includes("set state = 'superseded'")) {
      state.supersededCalls += 1
      return []
    }
    if (sql.includes('from deal_participant') && sql.includes("role = 'client'")) {
      return state.partyPersonId ? [{ person_id: state.partyPersonId }] : []
    }
    if (sql.includes('insert into transaction_document')) {
      const row: QueryRow = {
        id: `doc-${state.docs.length + 1}`,
        deal_id: values[0], document_type: values[1],
        document_type_label: values[2], title: values[3], state: values[4],
        source: values[5], source_system: values[6], source_external_id: values[7],
        prepared_by_user_id: values[8], party_person_id: values[9],
        media_id: values[10], signed_media_id: values[11], signed_at: values[12],
        supersedes_document_id: values[13], issued_checksum_sha256: values[14],
        template_id: values[15], template_version: values[16],
        source_snapshot: values[17], issued_version: values[18],
        form_instance_id: values[19],
        created_at: new Date('2026-08-22T00:00:00Z'),
        updated_at: new Date('2026-08-22T00:00:00Z'),
      }
      state.docs.push(row)
      return [row]
    }
    if (sql.includes('update document_form_instance')) {
      state.formStatusUpdates.push('issued')
      return []
    }
    if (sql.includes('update workflow_command_receipt')) {
      return []
    }
    if (sql.includes('from transaction_document td')) {
      return state.docs.map((d) => ({
        id: d.id, deal_id: d.deal_id, document_type_label: d.document_type_label,
        title: d.title, state: d.state, template_id: d.template_id,
        template_version: d.template_version, issued_version: d.issued_version,
        issued_checksum_sha256: d.issued_checksum_sha256,
        issued_by_display_name: null, party_name: null, property_name: null,
        deal_name: null, created_at: new Date('2026-08-22T00:00:00Z'),
      }))
    }
    return []
  }
}

function runFake(executor: QueryExecutor): TxRunner {
  return async (cb) => cb(executor)
}

function formFixture(
  overrides: Partial<FormInstance> & { id: string },
): FormInstance {
  return {
    id: overrides.id,
    templateId: overrides.templateId ?? 'OFFER-01',
    templateVersion: overrides.templateVersion ?? 1,
    dealId: overrides.dealId ?? 'deal-1',
    personId: overrides.personId ?? null,
    propertyId: overrides.propertyId ?? null,
    status: overrides.status ?? 'draft',
    fieldValues: overrides.fieldValues ?? {
      buyerName: 'Jane Buyer',
      property: 'Villa Rosa',
      offerAmount: '1250000',
      deposit: '50000',
      financing: 'Cash',
      closingDate: '2026-10-15',
      expiration: '2026-09-01',
      contingencies: 'Financing and inspection contingencies.',
    },
    sections: overrides.sections ?? {
      specialTerms: 'Closing by October 15, 2026, subject to attorney approval.',
    },
    createdByUserId: overrides.createdByUserId ?? 'user-1',
    createdAt: overrides.createdAt ?? '2026-08-22T00:00:00Z',
    updatedAt: overrides.updatedAt ?? '2026-08-22T00:00:00Z',
  }
}

const sha256 = (bytes: Buffer) =>
  createHash('sha256').update(bytes).digest('hex')

// __PART3__
test('DOC-06/07 proofs 4-7: first issuance creates a PDF, persists evidence, matches checksum, retrievable', async () => {
  const state = makeState(formFixture({ id: 'form-1' }), null, 'person-1')
  const executor = makeExecutor(state)

  const result = await issueFormDocument(
    { commandId: 'cmd-1', formInstanceId: 'form-1', actorAppUserId: 'user-1' },
    runFake(executor),
  )

  // proof 4 — a PDF was created through the media seam.
  assert.equal(result.outcome, 'success')
  const value = result.value as { documentId: string; mediaId: string; issuedVersion: number; checksum: string }
  assert.equal(value.issuedVersion, 1)
  assert.equal(state.mediaRows.length, 1)
  const pdf = state.mediaRows[0].bytes
  assert.ok(pdf.toString('latin1').startsWith('%PDF-1.4'))
  assert.equal(state.mediaRows[0].filename, 'offer-01-v1.pdf')

  // proof 5 — issued metadata + exact source snapshot persisted.
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

  // proof 6 — checksum matches the stored canonical artifact bytes.
  assert.equal(doc.issued_checksum_sha256, sha256(pdf))
  assert.equal(doc.issued_checksum_sha256, value.checksum)

  // proof 7 — repository lookup returns the issued document with evidence.
  const listing = await listIssuedDocuments(executor)
  assert.equal(listing.length, 1)
  assert.equal(listing[0].id, 'doc-1')
  assert.equal(listing[0].issuedVersion, 1)
  assert.equal(listing[0].issuedChecksumSha256, sha256(pdf))
  assert.equal(listing[0].state, 'ready')

  // the form instance was marked issued.
  assert.deepEqual(state.formStatusUpdates, ['issued'])
})

test('DOC-06/07 proofs 8-9: editing + issuing again creates v2; v1 stays byte-for-byte unchanged', async () => {
  const state = makeState(formFixture({ id: 'form-1' }), null, 'person-1')
  const executor = makeExecutor(state)

  // First issuance → v1.
  const v1 = await issueFormDocument(
    { commandId: 'cmd-1', formInstanceId: 'form-1', actorAppUserId: 'user-1' },
    runFake(executor),
  )
  assert.equal(v1.outcome, 'success')
  const v1Value = v1.value as { issuedVersion: number }
  assert.equal(v1Value.issuedVersion, 1)
  const v1Bytes = state.mediaRows[0].bytes
  const v1Checksum = sha256(v1Bytes)

  // Edited source on a NEW form instance for the same deal + template → v2.
  state.form = formFixture({
    id: 'form-2',
    fieldValues: { ...formFixture({ id: 'x' }).fieldValues, offerAmount: '1275000' },
  })
  state.prior = { id: state.docs[0].id as string, issued_version: 1 }

  const v2 = await issueFormDocument(
    { commandId: 'cmd-2', formInstanceId: 'form-2', actorAppUserId: 'user-1' },
    runFake(executor),
  )
  assert.equal(v2.outcome, 'success')
  const v2Value = v2.value as { issuedVersion: number }
  assert.equal(v2Value.issuedVersion, 2)
  assert.equal(state.mediaRows.length, 2)
  assert.equal(state.mediaRows[1].filename, 'offer-01-v2.pdf')
  const v2Checksum = sha256(state.mediaRows[1].bytes)

  // v2 supersedes v1 and v1 flips to 'superseded'.
  const v2Doc = state.docs[state.docs.length - 1]
  assert.equal(v2Doc.issued_version, 2)
  assert.equal(v2Doc.supersedes_document_id, 'doc-1')
  assert.equal(state.supersededCalls, 1)

  // proof 9 — v1 remains byte-for-byte unchanged and its evidence is intact.
  assert.deepEqual(state.mediaRows[0].bytes, v1Bytes, 'v1 bytes never mutated')
  const v1Doc = state.docs[0]
  assert.equal(v1Doc.issued_version, 1)
  assert.equal(v1Doc.media_id, 'media-1')
  assert.equal(v1Doc.issued_checksum_sha256, v1Checksum)
  assert.notEqual(v2Checksum, v1Checksum, 'v2 differs from v1')
  assert.ok(state.mediaRows[0].bytes.toString('latin1').includes('$1,250,000'))
  assert.ok(state.mediaRows[1].bytes.toString('latin1').includes('$1,275,000'))
})

test('DOC-06/07 proof 10: missing required fields never issue a malformed artifact', async () => {
  const incomplete = formFixture({ id: 'form-3' })
  incomplete.fieldValues = { ...incomplete.fieldValues, buyerName: '', offerAmount: '' }
  const state = makeState(incomplete, null, 'person-1')
  const executor = makeExecutor(state)

  const result = await issueFormDocument(
    { commandId: 'cmd-3', formInstanceId: 'form-3', actorAppUserId: 'user-1' },
    runFake(executor),
  )

  assert.equal(result.outcome, 'validation_failure')
  assert.match(String(result.message), /required/)
  // No canonical artifact, no media bytes, no form status change.
  assert.equal(state.mediaRows.length, 0)
  assert.equal(state.docs.length, 0)
  assert.deepEqual(state.formStatusUpdates, [])

  // Pure validation agrees.
  const issues = validateFormValues(OFFER_LETTER_TEMPLATE, incomplete.fieldValues)
  assert.ok(issues.some((i) => i.field === 'buyerName'))
  assert.ok(issues.some((i) => i.field === 'offerAmount'))
})

test('DOC-06/07 proof 10b: already-issued form instances cannot double-issue', async () => {
  const state = makeState(formFixture({ id: 'form-4', status: 'issued' }), null, 'person-1')
  const executor = makeExecutor(state)

  const result = await issueFormDocument(
    { commandId: 'cmd-4', formInstanceId: 'form-4', actorAppUserId: 'user-1' },
    runFake(executor),
  )
  assert.equal(result.outcome, 'conflict')
  assert.equal(state.mediaRows.length, 0)
  assert.equal(state.docs.length, 0)
})


