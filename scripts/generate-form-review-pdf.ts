import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

import {
  getTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  SHOWING_INFO_TEMPLATE_ID,
  SHOWING_REPORT_TEMPLATE_ID,
} from '../lib/forms/template-registry'
import { documentBodyText, renderFormPdfArtifact } from '../lib/forms/pdf'

const samples: Record<string, Record<string, string>> = {
  [OFFER_LETTER_TEMPLATE_ID]: {
    buyerName: 'Ana María Rivera',
    sellerName: 'Carlos Vega',
    brokerName: 'Lisa Penfield',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    offerAmount: '4950000',
    deposit: '150000',
    financing: 'Cash',
    closingDate: '2026-11-15',
    expiration: '2026-09-01',
    contingencies: 'Inspection, clear title, and attorney approval.',
  },
  [PURCHASE_SALE_TEMPLATE_ID]: {
    buyerName: 'Ana María Rivera',
    sellerName: 'Carlos Vega',
    buyerBrokerName: 'María Torres',
    sellerBrokerName: 'Lisa Penfield',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    municipality: 'Culebra',
    catastroNumber: '473-000-001-01',
    registryEntry: 'Finca 1241, Registro de Fajardo',
    purchasePrice: '4950000',
    deposit: '150000',
    financing: 'Cash',
    financingDeadline: '2026-09-25',
    appraisalWaived: 'No',
    surveyDeadline: '2026-10-05',
    inspectionDeadline: '2026-09-20',
    closingDate: '2026-11-15',
  },
  [PURCHASE_SALE_AMENDMENT_TEMPLATE_ID]: {
    buyerName: 'Ana María Rivera',
    sellerName: 'Carlos Vega',
    sellerBrokerName: 'Lisa Penfield',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    originalDate: '2026-09-01',
    amendmentDate: '2026-09-18',
    purchasePrice: '4875000',
    closingDate: '2026-11-30',
  },
  [LISTING_AGREEMENT_TEMPLATE_ID]: {
    sellerName: 'Carlos Vega and Elena Morales',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    propertyLocation: 'Punta Aloe, Culebra',
    listPrice: '5500000',
    commission: 'Six percent (6%)',
    startDate: '2026-09-01',
    endDate: '2027-03-01',
    brokerName: 'Lisa Penfield',
    listingType: 'Exclusive Right to Sell',
  },
  [SHOWING_INFO_TEMPLATE_ID]: {
    buyerName: 'Ana María Rivera',
    sellerName: 'Carlos Vega',
    buyerBrokerName: 'Lisa Penfield',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    showingDate: '2026-09-12',
    showingTime: '10:30 AM',
    offerAmount: '4950000',
    financing: 'Cash',
    occupants: '2',
  },
  [SHOWING_REPORT_TEMPLATE_ID]: {
    visitorName: 'José Muñoz',
    propertyOwnerName: 'Carlos Vega',
    agentName: 'Lisa Penfield',
    property: 'Villa del Mar, Culebra, Puerto Rico',
    showingDate: '2026-09-01',
    duration: '45 minutes',
    outcome: 'Interested',
    feedbackScore: '5',
  },
}

const sectionSamples: Record<string, Record<string, string>> = {
  [OFFER_LETTER_TEMPLATE_ID]: {
    specialTerms:
      'The offer includes the existing furnishings listed in the attached inventory. Seller will deliver the property vacant and broom-clean at closing.',
  },
  [PURCHASE_SALE_TEMPLATE_ID]: {
    financingTerms:
      'Buyer will purchase with verified cash funds. No financing contingency applies.',
    appraisalSurveyInspection:
      'Buyer may complete inspections by September 20, 2026 and a boundary survey by October 5, 2026. Material findings require written notice to Seller.',
    additionalTerms:
      'Indoor and outdoor furnishings identified in the signed inventory are included in the purchase price.',
    specialConditions:
      'Closing is conditioned on clear title and delivery of the current property registry certification.',
  },
  [PURCHASE_SALE_AMENDMENT_TEMPLATE_ID]: {
    amendments:
      'The purchase price is amended to $4,875,000 and the closing date is extended to November 30, 2026. All other terms remain unchanged.',
  },
  [LISTING_AGREEMENT_TEMPLATE_ID]: {
    marketing:
      'Seller authorizes professional photography, video, broker-network distribution, qualified-buyer showings, and publication on CulebraLuxe digital channels.',
    additional:
      'Showings require at least twenty-four hours notice and confirmation from the listing broker.',
  },
  [SHOWING_INFO_TEMPLATE_ID]: {
    access:
      'Broker will meet the visitors at the main gate. Seller-approved showing window is 10:30 AM to 11:30 AM; all exterior doors must be secured on departure.',
    interest:
      'Buyer is evaluating a cash offer near $4,950,000 and will confirm next steps after reviewing the property disclosures.',
  },
  [SHOWING_REPORT_TEMPLATE_ID]: {
    feedback:
      'Visitor responded strongly to the waterfront terrace, primary suite, and dock access. Questions remain about generator capacity and the furniture inventory.',
    followUp:
      'Send disclosures and furniture inventory today. Schedule a second showing with the buyer by September 15, 2026.',
  },
}

const participantSamples: Record<
  string,
  Array<{ role: string; slotId: string; name: string }>
> = {
  [OFFER_LETTER_TEMPLATE_ID]: [
    { role: 'BUYER', slotId: 'BUYER:1', name: 'Ana María Rivera' },
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'BUYER_BROKER', slotId: 'BUYER_BROKER:1', name: 'Lisa Penfield' },
  ],
  [PURCHASE_SALE_TEMPLATE_ID]: [
    { role: 'BUYER', slotId: 'BUYER:1', name: 'Ana María Rivera' },
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
  ],
  [PURCHASE_SALE_AMENDMENT_TEMPLATE_ID]: [
    { role: 'BUYER', slotId: 'BUYER:1', name: 'Ana María Rivera' },
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
  ],
  [LISTING_AGREEMENT_TEMPLATE_ID]: [
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'SELLER', slotId: 'SELLER:2', name: 'Elena Morales' },
    { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
  ],
  [SHOWING_INFO_TEMPLATE_ID]: [
    { role: 'BUYER', slotId: 'BUYER:1', name: 'Ana María Rivera' },
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'BUYER_BROKER', slotId: 'BUYER_BROKER:1', name: 'Lisa Penfield' },
  ],
  [SHOWING_REPORT_TEMPLATE_ID]: [
    { role: 'BUYER', slotId: 'BUYER:1', name: 'José Muñoz' },
    { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
    { role: 'BUYER_BROKER', slotId: 'BUYER_BROKER:1', name: 'Lisa Penfield' },
  ],
}

const templateId = process.argv[2] ?? PURCHASE_SALE_TEMPLATE_ID
const output = resolve(
  process.argv[3] ?? `output/pdf/${templateId.toLowerCase()}-review.pdf`,
)
const signatureAssetPath = process.argv[4]
const template = getTemplate(templateId)
if (!template) throw new Error(`Unknown template ${templateId}`)

const values: Record<string, string> = {}
for (const field of template.fields) {
  const value = samples[templateId]?.[field.name]
  if (!value) {
    throw new Error(`Review sample ${templateId} is missing field ${field.name}.`)
  }
  values[field.name] = value
}

const participants = participantSamples[templateId]
if (!participants || participants.length < 3) {
  throw new Error(`Review sample ${templateId} requires at least three signer participants.`)
}

const body = template.sections
  .map((section) => {
    const override = (sectionSamples[templateId]?.[section.name] ?? '').trim()
    if (override) return `${section.label}\n${override}`
    return documentBodyText(
      { fields: template.fields, sections: [section] },
      values,
    )
  })
  .join('\n\n')

const brokerSignatureGroup = template.signatureGroups.find(
  (group) => group.role.endsWith('_BROKER'),
)
const brokerParticipant = brokerSignatureGroup
  ? participants.find(
      (participant) => participant.role === brokerSignatureGroup.role,
    )
  : null
const signatureBytes =
  signatureAssetPath && brokerSignatureGroup
    ? await readFile(resolve(signatureAssetPath))
    : null
const appliedSignatures =
  signatureBytes && brokerSignatureGroup
    ? [
        {
          role: brokerSignatureGroup.role,
          slotId: brokerParticipant?.slotId ?? null,
          signerName: 'Lisa Penfield',
          credentialLine: 'Real Estate Broker License #: C-9931',
          signerAppUserId: 'review-owner-user',
          imageBytes: signatureBytes,
          imageMimeType: 'image/png' as const,
          assetMediaId: 'review-protected-signature-asset',
          assetChecksumSha256: createHash('sha256')
            .update(signatureBytes)
            .digest('hex'),
          appliedAt: '2026-08-26T18:30:00.000Z',
          consentBasis: 'authenticated-owner-issuance' as const,
          dateSemantic: 'issuance-requested-at' as const,
        },
      ]
    : []

const artifact = await renderFormPdfArtifact(
  template,
  values,
  { body },
  1,
  { participants, appliedSignatures },
)
await mkdir(dirname(output), { recursive: true })
await writeFile(output, artifact.bytes)
process.stdout.write(
  `${output}\n${artifact.pageCount} pages · ${artifact.signatureAnchors.length} signature fields · ${artifact.appliedSignatures.length} composed owner signatures\n`,
)
