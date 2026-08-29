import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'

import {
  getActiveTemplate,
  getLatestTemplate,
  getTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
} from '../lib/forms/template-registry'
import { documentBodyText, renderFormPdfArtifact } from '../lib/forms/pdf'

const singleValues: Record<string, string> = {
  sellerName: 'Isabel Rivera',
  sellerCivilStatus: 'single',
  sellerResidenceAddress: 'P.O. Box 214, Culebra, PR 00775',
  brokerName: 'Lisa Penfield',
  property: 'Casa Brisa',
  propertyLocation: 'Carretera 250, Km 2.1, Culebra, PR 00775',
  catastroNumber: '473-000-001-01',
  listPrice: '1250000',
  commission: '5% co-booking / 4% Broker-only',
  startDate: '2026-09-01',
  endDate: '2027-09-01',
  listingType: 'Exclusive Right to Sell',
}

const multiValues: Record<string, string> = {
  ...singleValues,
  sellerName: 'Carlos Vega and Elena Morales',
  sellerCivilStatus: 'married to each other',
  sellerResidenceAddress: 'P.O. Box 88, Culebra, PR 00775',
  property: 'Villa del Mar',
  propertyLocation: 'Punta Aloe, Culebra, PR 00775',
  catastroNumber: '473-000-015-02',
  listPrice: '5500000',
}

const singleParticipants = [
  { role: 'SELLER', slotId: 'SELLER:1', name: 'Isabel Rivera' },
  { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
]

const multiParticipants = [
  { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
  { role: 'SELLER', slotId: 'SELLER:2', name: 'Elena Morales' },
  { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
]

async function renderScenario(
  label: string,
  values: Record<string, string>,
  participants: typeof singleParticipants,
) {
  const template = getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 2)
  assert(template, 'LISTING-01 v2 must resolve exactly')

  for (const field of template.fields) {
    assert(
      (values[field.name] ?? '').trim(),
      `${label}: sample missing field ${field.name}`,
    )
  }

  const body = documentBodyText(template, values)

  // Exact formatting / punctuation assertions before the same body reaches PDF.
  const requiredText = [
    label === 'single' ? '$1,250,000' : '$5,500,000',
    '5% co-booking / 4% Broker-only',
    '$500',
    '$600',
    '50/50',
    '50%',
    '60%',
    'September 1, 2026',
    'September 1, 2027',
  ]
  for (const expected of requiredText) {
    assert(body.includes(expected), `${label}: rendered body missing ${expected}`)
  }
  assert(!body.includes('$$'), `${label}: duplicate dollar sign found`)
  assert(!body.includes('%%'), `${label}: duplicate percent sign found`)

  const artifact = await renderFormPdfArtifact(
    template,
    values,
    { body },
    1,
    { participants },
  )

  assert.equal(
    artifact.bytes.subarray(0, 5).toString('ascii'),
    '%PDF-',
    `${label}: output is not a PDF`,
  )
  assert(artifact.pageCount >= 2, `${label}: agreement unexpectedly fit on one page`)
  const expectedAnchorCount = participants.length * 3
  assert.equal(
    artifact.signatureAnchors.length,
    expectedAnchorCount,
    `${label}: signature/initial/date anchor count`,
  )
  assert.equal(artifact.appliedSignatures.length, 0)

  const anchorsBySlot = new Map<string, Set<string>>()
  for (const anchor of artifact.signatureAnchors) {
    const slot = anchor.slotId ?? '(none)'
    const kinds = anchorsBySlot.get(slot) ?? new Set<string>()
    kinds.add(anchor.kind)
    anchorsBySlot.set(slot, kinds)
    assert(anchor.pageIndex >= 0 && anchor.pageIndex < artifact.pageCount)
    assert(anchor.rect.width > 0 && anchor.rect.height > 0)
  }
  for (const participant of participants) {
    assert.deepEqual(
      [...(anchorsBySlot.get(participant.slotId) ?? [])].sort(),
      ['date', 'initial', 'signature'],
      `${label}: complete anchor set for ${participant.slotId}`,
    )
  }

  return {
    label,
    pages: artifact.pageCount,
    anchors: artifact.signatureAnchors.length,
    sha256: createHash('sha256').update(artifact.bytes).digest('hex'),
    bytes: artifact.bytes.length,
    bodyChars: body.length,
  }
}

async function main() {
  assert.equal(getActiveTemplate(LISTING_AGREEMENT_TEMPLATE_ID)?.version, 1)
  assert.equal(getLatestTemplate(LISTING_AGREEMENT_TEMPLATE_ID)?.version, 2)
  assert.equal(getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 2)?.version, 2)

  const single = await renderScenario('single', singleValues, singleParticipants)
  const multi = await renderScenario('multi', multiValues, multiParticipants)

  process.stdout.write(
    `LISTING-V2 DRY RUN PASS\n` +
      `active=v1 registered-latest=v2\n` +
      `${single.label}: ${single.pages} pages · ${single.anchors} anchors · ${single.bytes} bytes · sha256 ${single.sha256}\n` +
      `${multi.label}: ${multi.pages} pages · ${multi.anchors} anchors · ${multi.bytes} bytes · sha256 ${multi.sha256}\n` +
      `symbols: $1,250,000 / $5,500,000 / 5% / 4% / $500 / $600 / 50/50 / 50% / 60% verified\n`,
  )
}

void main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : error)
  process.exitCode = 1
})
