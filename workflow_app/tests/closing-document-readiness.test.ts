import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  deriveClosingDocumentReadiness,
  requiredClosingDocumentPackage,
} from '../transaction-packet'
import type { PacketFacts } from '../transaction-packet'
import type { TransactionDocument, TransactionDocumentType } from '../../db/transaction-document'

// ---------------------------------------------------------------------------
// CRM-21 — closing-document readiness (the derived workflow fact).
//
// The fact is deterministic and derived: required set = the packet catalog's
// closing package (never invented); ready only when every required item is
// present AND in a final (signed) state of the DOC-01 draft -> ready -> sent
// -> signed lineage. Covers the acceptance cases: missing, complete,
// signed-reconciled (DOC-05 artifacts), and duplicate/replay.
// ---------------------------------------------------------------------------

const FACTS: PacketFacts = {
  financingApplicable: true,
  closingDateScheduled: true,
  appraisalApplicable: true,
  requiresNotario: true,
  requiresTitleCompany: false,
  requiresCrimClearance: true,
  requiresRegistryFollowup: true,
  inspectionApplicable: true,
  insuranceApplicable: true,
  requiresSurvey: false,
  requiresHoaClearance: false,
}

function doc(overrides: Partial<TransactionDocument> & { id: string; documentType: TransactionDocumentType }): TransactionDocument {
  return {
    id: 'd',
    dealId: 'deal-1',
    documentType: 'other',
    documentTypeLabel: null,
    title: null,
    state: 'draft',
    source: 'generated',
    sourceSystem: null,
    sourceExternalId: null,
    preparedByUserId: null,
    partyPersonId: null,
    mediaId: null,
    signedMediaId: null,
    signedAt: null,
    supersedesDocumentId: null,
    createdAt: '',
    updatedAt: '',
    ...overrides,
  }
}

const keys = (items: ReadonlyArray<{ key: string }>): string[] => items.map((i) => i.key)

// ---------------------------------------------------------------------------
// Required closing package — catalog-derived, never invented
// ---------------------------------------------------------------------------

test('requiredClosingDocumentPackage derives exactly the packet closing items', () => {
  const pkg = requiredClosingDocumentPackage(FACTS)
  assert.deepEqual(keys(pkg), ['closing_documents', 'closing_statement'])
  const closingDocs = pkg.find((i) => i.key === 'closing_documents')!
  assert.equal(closingDocs.documentType, 'closing')
  assert.equal(closingDocs.documentTypeLabel, null)
  const statement = pkg.find((i) => i.key === 'closing_statement')!
  assert.equal(statement.documentType, 'closing')
  assert.equal(statement.documentTypeLabel, 'Closing statement')

  // Post-closing registry follow-up is NOT part of pre-closing closing-document
  // readiness — even when requiresRegistryFollowup is true. The fact never
  // invents requirements beyond the packet catalog.
  assert.deepEqual(keys(requiredClosingDocumentPackage(FACTS)), keys(requiredClosingDocumentPackage({ ...FACTS, requiresRegistryFollowup: false })))
})

test('requiredClosingDocumentPackage is deterministic', () => {
  const a = requiredClosingDocumentPackage(FACTS)
  const b = requiredClosingDocumentPackage({ ...FACTS, financingApplicable: false })
  assert.deepEqual(a, b)
  assert.deepEqual(a, requiredClosingDocumentPackage(FACTS))
})

// ---------------------------------------------------------------------------
// Missing / incomplete → false
// ---------------------------------------------------------------------------

test('missing: empty closing packet reports ready=false with both items missing', () => {
  const r = deriveClosingDocumentReadiness(FACTS, [])
  assert.equal(r.ready, false)
  assert.deepEqual(r.missing, ['closing_documents', 'closing_statement'])
  assert.deepEqual(r.unsigned, [])
  assert.ok(r.items.every((i) => i.status === 'missing'))
})

test('missing: a partial packet with only an unrelated document is not ready', () => {
  const docs = [
    doc({ id: 'ag1', documentType: 'agreement', state: 'signed' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, false)
  assert.deepEqual(r.missing, ['closing_documents', 'closing_statement'])
  assert.deepEqual(r.unsigned, [])
  assert.ok(r.items.every((i) => i.status === 'missing'))
})

test('DOC-02 matching preserved: a signed closing-statement row also satisfies the unlabeled closing_documents item', () => {
  // The packet's closing_documents item is type/label-based (label null → any
  // 'closing' row matches, including the closing statement). The readiness fact
  // inherits that matching — it never redefines presence.
  const docs = [
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'signed' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.items.find((i) => i.key === 'closing_documents')!.status, 'ready')
  assert.equal(r.items.find((i) => i.key === 'closing_statement')!.status, 'ready')
  assert.equal(r.ready, true)
})

test('unsigned: pre-signed states (draft/ready/sent) never satisfy readiness', () => {
  const docs = [
    doc({ id: 'cd1', documentType: 'closing', state: 'draft' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'sent' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, false, 'pre-signed documents are not closing-ready')
  assert.deepEqual(r.missing, [])
  assert.deepEqual(r.unsigned, ['closing_documents', 'closing_statement'])
  assert.ok(r.items.every((i) => i.status === 'unsigned'))
})

test('mixed: one item signed, the other still pre-signed → not ready', () => {
  const docs = [
    doc({ id: 'cd1', documentType: 'closing', state: 'signed' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'ready' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, false)
  assert.deepEqual(r.unsigned, ['closing_statement'])
  assert.deepEqual(r.items.find((i) => i.key === 'closing_documents')!.finalDocumentIds, ['cd1'])
})

// ---------------------------------------------------------------------------
// Complete → true
// ---------------------------------------------------------------------------

test('complete: every required closing item present and signed → ready=true', () => {
  const docs = [
    doc({ id: 'cd1', documentType: 'closing', state: 'signed' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'signed' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, true)
  assert.deepEqual(r.missing, [])
  assert.deepEqual(r.unsigned, [])
  assert.ok(r.items.every((i) => i.status === 'ready'))
  // Both signed rows are final for the unlabeled closing_documents item
  // (DOC-02 type matching: label null matches any 'closing' row).
  assert.deepEqual(r.items.find((i) => i.key === 'closing_documents')!.finalDocumentIds, ['cd1', 'cs1'])
})

// ---------------------------------------------------------------------------
// Signed-reconciled — DOC-05 artifacts (signed lineage) count as final
// ---------------------------------------------------------------------------

test('signed-reconciled: a DOC-05 reconciled artifact (signed + signed lineage) flips readiness', () => {
  // Before reconciliation the document is 'sent' (awaiting the signed artifact).
  const before = deriveClosingDocumentReadiness(FACTS, [
    doc({ id: 'cd1', documentType: 'closing', state: 'sent' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'sent' }),
  ])
  assert.equal(before.ready, false)
  assert.deepEqual(before.unsigned, ['closing_documents', 'closing_statement'])

  // DOC-05 reconciliation appends a NEW signed media row and transitions the
  // document to 'signed' (signed_media_id / signed_at set; the draft bytes are
  // never mutated). The same rows now satisfy readiness.
  const after = deriveClosingDocumentReadiness(FACTS, [
    doc({
      id: 'cd1',
      documentType: 'closing',
      state: 'signed',
      signedMediaId: 'media-signed-deed',
      signedAt: '2026-08-21T12:00:00Z',
    }),
    doc({
      id: 'cs1',
      documentType: 'closing',
      documentTypeLabel: 'Closing statement',
      state: 'signed',
      signedMediaId: 'media-signed-statement',
      signedAt: '2026-08-21T12:00:00Z',
    }),
  ])
  assert.equal(after.ready, true, 'signed-reconciled artifacts are final')
  assert.deepEqual(after.missing, [])
  assert.deepEqual(after.unsigned, [])
})

test('signed-reconciled: the draft and signed artifacts coexist (lineage preserved), still ready', () => {
  // The DOC-01 lineage keeps the draft media row AND the signed artifact row on
  // the same document record — one row with state 'signed'. Readiness reads
  // the document row's state, not the bytes.
  const docs = [
    doc({ id: 'cd1', documentType: 'closing', state: 'signed', mediaId: 'media-draft-deed', signedMediaId: 'media-signed-deed', signedAt: '2026-08-21T12:00:00Z' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'signed', mediaId: 'media-draft-statement', signedMediaId: 'media-signed-statement', signedAt: '2026-08-21T12:00:00Z' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, true)
})

// ---------------------------------------------------------------------------
// Duplicate / replay — the fact stays deterministic
// ---------------------------------------------------------------------------

test('duplicate/replay: duplicate matching rows cannot change or break the fact', () => {
  // Two matching closing-document rows (e.g. a replayed create/upload) — the
  // fact still reports ready and deterministically.
  const docs = [
    doc({ id: 'cd1', documentType: 'closing', state: 'signed' }),
    doc({ id: 'cd1-dup', documentType: 'closing', state: 'signed' }),
    doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'signed' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, true)
  const closing = r.items.find((i) => i.key === 'closing_documents')!
  // Both closing rows AND the statement row match the unlabeled item (DOC-02
  // type matching); the duplicates cannot break or change the fact.
  assert.deepEqual(closing.documentIds, ['cd1', 'cd1-dup', 'cs1'])
  assert.deepEqual(closing.finalDocumentIds, ['cd1', 'cd1-dup', 'cs1'])

  // Determinism: identical inputs produce identical output.
  assert.deepEqual(deriveClosingDocumentReadiness(FACTS, docs), r)
})

test('duplicate/replay: superseded signed artifacts lose readiness — only the current lineage head counts', () => {
  // A signed artifact that was superseded (a new version drafted) is no longer
  // final: the current lineage head is a draft, so readiness is lost until the
  // replacement is signed. Voided duplicates never count. (Per DOC-02 matching,
  // the unlabeled closing_documents item is satisfied by ANY 'closing' row, so
  // the statement is kept draft here to avoid accidentally satisfying it.)
  const superseded = doc({ id: 'cd-old', documentType: 'closing', state: 'superseded', signedMediaId: 'media-old', signedAt: '2026-01-01T00:00:00Z' })
  const draftReplacement = doc({ id: 'cd-new', documentType: 'closing', state: 'draft' })
  const voidedDup = doc({ id: 'cd-void', documentType: 'closing', state: 'voided' })
  const statementDraft = doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'draft' })

  const r = deriveClosingDocumentReadiness(FACTS, [superseded, draftReplacement, voidedDup, statementDraft])
  assert.equal(r.ready, false, 'superseded signed + draft replacement is not final')
  assert.deepEqual(r.unsigned, ['closing_documents', 'closing_statement'])
  assert.deepEqual(r.items.find((i) => i.key === 'closing_documents')!.finalDocumentIds, [])

  // Once the replacement (and the statement) are signed, the lineage is final
  // again and readiness returns.
  const signedReplacement = doc({ id: 'cd-new', documentType: 'closing', state: 'signed', signedMediaId: 'media-new', signedAt: '2026-08-21T12:00:00Z' })
  const statementSigned = doc({ id: 'cs1', documentType: 'closing', documentTypeLabel: 'Closing statement', state: 'signed' })
  const ready = deriveClosingDocumentReadiness(FACTS, [superseded, signedReplacement, voidedDup, statementSigned])
  assert.equal(ready.ready, true, 'signed replacement restores readiness')
})

test('registry follow-up documents never satisfy the closing package (post-closing item)', () => {
  // A signed registry/recording follow-up row (documentType 'other') cannot
  // satisfy the closing-document readiness requirement — it is not a closing
  // document.
  const docs = [
    doc({ id: 'reg1', documentType: 'other', documentTypeLabel: 'Registry / recording follow-up', state: 'signed' }),
  ]
  const r = deriveClosingDocumentReadiness(FACTS, docs)
  assert.equal(r.ready, false)
  assert.deepEqual(r.missing, ['closing_documents', 'closing_statement'])
})
