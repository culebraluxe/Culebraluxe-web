import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildTransactionPacket,
  getTransactionPacketForDeal,
  requiredTransactionDocumentTypes,
  unresolvedPacketGates,
} from '../transaction-packet'
import type { PacketFacts } from '../transaction-packet'
import { createTransactionDocument } from '../../db/transaction-document'
import type { TransactionDocument, TransactionDocumentType } from '../../db/transaction-document'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// DOC-02 transaction packet tests. The packet is a pure, derived projection:
// required document types from deal stage + facts, compared against canonical
// transaction_document rows (present / missing / unresolved). No database, no
// signing concepts, no auto-creation.
// ---------------------------------------------------------------------------

// Defaults matching CULEBRA_JURISDICTION_CONFIG with financing + appraisal
// resolved (the Culebra operating posture).
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

const keys = (items: ReadonlyArray<{ key: string }>): string[] => items.map((i) => i.key)

// ---------------------------------------------------------------------------
// Pure required-document mapping
// ---------------------------------------------------------------------------

test('requiredTransactionDocumentTypes is deterministic per canonical stage', () => {
  const offer = requiredTransactionDocumentTypes('offer', FACTS)
  assert.deepEqual(keys(offer), ['agreement'])

  const underContract = requiredTransactionDocumentTypes('under_contract', FACTS)
  assert.deepEqual(keys(underContract), [
    'agreement',
    'disclosure',
    'title',
    'financing',
    'inspection',
    'appraisal',
    'tax_crim_clearance',
  ])

  const closed = requiredTransactionDocumentTypes('closed', FACTS)
  assert.deepEqual(keys(closed), [
    'closing_documents',
    'closing_statement',
    'registry_followup',
  ])

  // determinism: same inputs, same set
  assert.deepEqual(requiredTransactionDocumentTypes('under_contract', FACTS), underContract)
  assert.deepEqual(requiredTransactionDocumentTypes('closed', FACTS), closed)
})

test('requiredTransactionDocumentTypes gates optional documents on facts', () => {
  assert.ok(!keys(requiredTransactionDocumentTypes('under_contract', { ...FACTS, financingApplicable: false })).includes('financing'))
  assert.ok(!keys(requiredTransactionDocumentTypes('under_contract', { ...FACTS, inspectionApplicable: false })).includes('inspection'))
  assert.ok(!keys(requiredTransactionDocumentTypes('under_contract', { ...FACTS, appraisalApplicable: false })).includes('appraisal'))
  assert.ok(!keys(requiredTransactionDocumentTypes('under_contract', { ...FACTS, requiresCrimClearance: false })).includes('tax_crim_clearance'))
  assert.ok(!keys(requiredTransactionDocumentTypes('closed', { ...FACTS, requiresRegistryFollowup: false })).includes('registry_followup'))
})

test('requiredTransactionDocumentTypes emits HOA and survey when their flags apply', () => {
  const items = requiredTransactionDocumentTypes('under_contract', {
    ...FACTS,
    requiresHoaClearance: true,
    requiresSurvey: true,
  })
  assert.deepEqual(keys(items), [
    'agreement',
    'disclosure',
    'title',
    'financing',
    'inspection',
    'appraisal',
    'tax_crim_clearance',
    'hoa_clearance',
    'survey',
  ])
  const hoa = items.find((i) => i.key === 'hoa_clearance')!
  assert.equal(hoa.documentType, 'other')
  assert.equal(hoa.documentTypeLabel, 'HOA / condo clearance')
  const survey = items.find((i) => i.key === 'survey')!
  assert.equal(survey.documentTypeLabel, 'Survey')
})

test('long-tail required items reuse transaction_document categories plus curated labels', () => {
  const under = requiredTransactionDocumentTypes('under_contract', FACTS)
  const tax = under.find((i) => i.key === 'tax_crim_clearance')!
  assert.equal(tax.documentType, 'other')
  assert.equal(tax.documentTypeLabel, 'CRIM / tax clearance')

  const closed = requiredTransactionDocumentTypes('closed', FACTS)
  const statement = closed.find((i) => i.key === 'closing_statement')!
  assert.equal(statement.documentType, 'closing')
  assert.equal(statement.documentTypeLabel, 'Closing statement')
})

test('pre-packet and unknown stages deterministically require nothing', () => {
  for (const stage of ['new_lead', 'qualified', 'showing', 'archived']) {
    assert.deepEqual(requiredTransactionDocumentTypes(stage, FACTS), [])
    assert.deepEqual(unresolvedPacketGates(stage, FACTS), [])
  }
})

// ---------------------------------------------------------------------------
// Unresolved (Class C) facts — surface, never fabricate
// ---------------------------------------------------------------------------

test('unresolved gating facts never produce a required document', () => {
  const under = requiredTransactionDocumentTypes('under_contract', {
    ...FACTS,
    appraisalApplicable: null,
    financingApplicable: null,
  })
  assert.ok(!keys(under).includes('appraisal'))
  assert.ok(!keys(under).includes('financing'))
})

test('unresolvedPacketGates surfaces the unresolved fact at the applicable stage only', () => {
  const under = unresolvedPacketGates('under_contract', {
    ...FACTS,
    appraisalApplicable: null,
    financingApplicable: null,
  })
  assert.deepEqual(under.map((g) => g.key), ['financing', 'appraisal'])
  assert.equal(under.find((g) => g.key === 'appraisal')!.fact, 'appraisalApplicable')
  assert.match(under.find((g) => g.key === 'appraisal')!.message, /appraisalApplicable.*unresolved/i)

  // at offer the appraisal gate does not apply -> no unresolved entry
  assert.deepEqual(unresolvedPacketGates('offer', { ...FACTS, appraisalApplicable: null }), [])
  // resolved facts produce no unresolved entries
  assert.deepEqual(unresolvedPacketGates('under_contract', FACTS), [])
})

// ---------------------------------------------------------------------------
// Pure completeness projection (present / missing / unresolved)
// ---------------------------------------------------------------------------

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

test('buildTransactionPacket lists every required item as missing for an empty packet', () => {
  const packet = buildTransactionPacket('deal-1', 'under_contract', FACTS, [])
  assert.equal(packet.dealId, 'deal-1')
  assert.equal(packet.stage, 'under_contract')
  assert.equal(packet.presentCount, 0)
  assert.equal(packet.missingCount, 7)
  assert.equal(packet.unresolvedCount, 0)
  assert.equal(packet.complete, false)
  assert.ok(packet.items.every((i) => i.status === 'missing'))
  assert.ok(packet.items.every((i) => i.documentIds.length === 0))
})

test('buildTransactionPacket marks present items by type — signature state is irrelevant', () => {
  const docs = [
    doc({ id: 'a1', documentType: 'agreement', state: 'draft' }),
    doc({ id: 'd1', documentType: 'disclosure', state: 'signed' }),
    doc({ id: 't1', documentType: 'title', state: 'sent' }),
    doc({ id: 'f1', documentType: 'financing', state: 'ready' }),
    doc({ id: 'i1', documentType: 'inspection', state: 'draft' }),
    doc({ id: 'ap1', documentType: 'appraisal', state: 'draft' }),
    doc({ id: 'c1', documentType: 'other', documentTypeLabel: 'CRIM / tax clearance', state: 'draft' }),
  ]
  const packet = buildTransactionPacket('deal-1', 'under_contract', FACTS, docs)
  assert.equal(packet.presentCount, 7)
  assert.equal(packet.missingCount, 0)
  assert.equal(packet.unresolvedCount, 0)
  assert.equal(packet.complete, true)
  const agreement = packet.items.find((i) => i.key === 'agreement')!
  assert.equal(agreement.status, 'present')
  assert.deepEqual(agreement.documentIds, ['a1'])
  assert.ok(packet.items.every((i) => i.status === 'present'))
})

test('voided and superseded documents do not count as present', () => {
  const docs = [
    doc({ id: 'v1', documentType: 'agreement', state: 'voided' }),
    doc({ id: 's1', documentType: 'disclosure', state: 'superseded' }),
  ]
  const packet = buildTransactionPacket('deal-1', 'under_contract', FACTS, docs)
  assert.equal(packet.items.find((i) => i.key === 'agreement')!.status, 'missing')
  assert.equal(packet.items.find((i) => i.key === 'disclosure')!.status, 'missing')
})

test('long-tail presence matches the curated label exactly', () => {
  const wrongLabel = doc({ id: 'x1', documentType: 'other', documentTypeLabel: 'Municipal clearance' })
  const packet = buildTransactionPacket('deal-1', 'under_contract', FACTS, [wrongLabel])
  assert.equal(packet.items.find((i) => i.key === 'tax_crim_clearance')!.status, 'missing')

  const exact = doc({ id: 'x2', documentType: 'other', documentTypeLabel: 'CRIM / tax clearance' })
  const ok = buildTransactionPacket('deal-1', 'under_contract', FACTS, [exact])
  const tax = ok.items.find((i) => i.key === 'tax_crim_clearance')!
  assert.equal(tax.status, 'present')
  assert.deepEqual(tax.documentIds, ['x2'])
})

test('an unresolved gate is surfaced as unresolved, never fabricated, and the packet still computes', () => {
  const packet = buildTransactionPacket(
    'deal-1',
    'under_contract',
    { ...FACTS, appraisalApplicable: null },
    [],
  )
  const appraisal = packet.items.find((i) => i.key === 'appraisal')!
  assert.equal(appraisal.status, 'unresolved')
  assert.deepEqual(appraisal.documentIds, [])
  assert.equal(packet.unresolvedCount, 1)
  assert.equal(packet.missingCount, 6)
  assert.equal(packet.presentCount, 0)
  assert.equal(packet.complete, false, 'an unresolved gate keeps the packet incomplete but computed')
})

// ---------------------------------------------------------------------------
// Read seam over transaction_document (fake executor, no database)
// ---------------------------------------------------------------------------

type Row = Record<string, any>

class FakeDb {
  documents: Row[] = []
  seq = 0
  now = '2026-08-21T12:00:00Z'

  private norm(s: string) {
    return s.replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const t = this.norm(
      strings.reduce((acc, s, i) => acc + s + (i < params.length ? '$' + (i + 1) : ''), ''),
    )
    const p = params as any[]

    if (t.includes('insert into transaction_document') && t.includes('on conflict')) {
      this.seq += 1
      const row = {
        id: `doc-${this.seq}`,
        deal_id: p[0],
        document_type: p[1],
        document_type_label: p[2],
        title: p[3],
        state: p[4],
        source: p[5],
        source_system: p[6],
        source_external_id: p[7],
        prepared_by_user_id: p[8],
        party_person_id: p[9],
        media_id: p[10],
        signed_media_id: p[11],
        signed_at: p[12],
        supersedes_document_id: p[13],
        created_at: this.now,
        updated_at: this.now,
      }
      this.documents.push(row)
      return Promise.resolve([row])
    }
    if (t.includes('from transaction_document') && t.includes('where deal_id =')) {
      return Promise.resolve(this.documents.filter((d) => d.deal_id === p[0]))
    }

    throw new Error(`PACKET_FAKE_UNHANDLED: ${t}`)
  }
}

async function seedDoc(f: FakeDb, dealId: string, overrides: Record<string, any> = {}): Promise<void> {
  await createTransactionDocument(
    { dealId, documentType: 'agreement', source: 'generated', ...overrides },
    f.tx,
  )
}

test('getTransactionPacketForDeal projects per-deal completeness and never writes', async () => {
  const f = new FakeDb()
  await seedDoc(f, 'deal-1', { documentType: 'agreement', title: 'P&S' })
  await seedDoc(f, 'deal-1', { documentType: 'other', documentTypeLabel: 'CRIM / tax clearance' })
  await seedDoc(f, 'deal-2', { documentType: 'agreement' }) // another deal must not count
  const before = f.documents.length

  const packet = await getTransactionPacketForDeal('deal-1', 'under_contract', FACTS, f.tx)
  assert.equal(packet.items.find((i) => i.key === 'agreement')!.status, 'present')
  assert.equal(packet.items.find((i) => i.key === 'tax_crim_clearance')!.status, 'present')
  assert.equal(packet.items.find((i) => i.key === 'title')!.status, 'missing')
  assert.equal(packet.presentCount, 2)
  assert.equal(packet.missingCount, 5)
  assert.equal(f.documents.length, before, 'the packet never auto-creates documents')
})

test('getTransactionPacketForDeal at closed reports the closing package state', async () => {
  const f = new FakeDb()
  await seedDoc(f, 'deal-1', { documentType: 'closing' })
  await seedDoc(f, 'deal-1', { documentType: 'closing', documentTypeLabel: 'Closing statement' })

  const packet = await getTransactionPacketForDeal('deal-1', 'closed', FACTS, f.tx)
  assert.deepEqual(
    packet.items.map((i) => i.key),
    ['closing_documents', 'closing_statement', 'registry_followup'],
  )
  assert.equal(packet.items.find((i) => i.key === 'closing_documents')!.status, 'present')
  assert.equal(packet.items.find((i) => i.key === 'closing_statement')!.status, 'present')
  assert.equal(packet.items.find((i) => i.key === 'registry_followup')!.status, 'missing')
  assert.equal(packet.presentCount, 2)
  assert.equal(packet.missingCount, 1)
})
