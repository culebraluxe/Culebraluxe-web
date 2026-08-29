import {
  getTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
} from '@/lib/forms/template-registry'
import { documentBodyText, renderFormPdfArtifact } from '@/lib/forms/pdf'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const SINGLE_VALUES: Record<string, string> = {
  sellerName: 'Isabel Rivera',
  sellerCivilStatus: 'single',
  sellerResidenceAddress: 'P.O. Box 214, Culebra, PR 00775',
  brokerName: 'Lisa Penfield',
  property: 'Casa Brisa',
  propertyLocation: 'Carretera 250, Km 2.1, Culebra, PR 00775',
  catastroNumber: '473-000-001-01',
  listPrice: '1250000',
  commission: '5% of sales price in a co-booking situation; 4% if Broker acts alone',
  startDate: '2026-09-01',
  endDate: '2027-09-01',
  listingType: 'Exclusive Right to Sell',
}

const MULTI_VALUES: Record<string, string> = {
  ...SINGLE_VALUES,
  sellerName: 'Carlos Vega and Elena Morales',
  sellerCivilStatus: 'married to each other',
  sellerResidenceAddress: 'P.O. Box 88, Culebra, PR 00775',
  property: 'Villa del Mar',
  propertyLocation: 'Punta Aloe, Culebra, PR 00775',
  catastroNumber: '473-000-015-02',
  listPrice: '5500000',
}

export async function GET(request: Request) {
  const template = getTemplate(LISTING_AGREEMENT_TEMPLATE_ID, 2)
  if (!template) return new Response('LISTING-01 v2 is not registered.', { status: 404 })

  const scenario = new URL(request.url).searchParams.get('scenario') === 'multi'
    ? 'multi'
    : 'single'
  const values = scenario === 'multi' ? MULTI_VALUES : SINGLE_VALUES
  const participants = scenario === 'multi'
    ? [
        { role: 'SELLER', slotId: 'SELLER:1', name: 'Carlos Vega' },
        { role: 'SELLER', slotId: 'SELLER:2', name: 'Elena Morales' },
        { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
      ]
    : [
        { role: 'SELLER', slotId: 'SELLER:1', name: 'Isabel Rivera' },
        { role: 'SELLER_BROKER', slotId: 'SELLER_BROKER:1', name: 'Lisa Penfield' },
      ]

  const body = documentBodyText(template, values)
  const artifact = await renderFormPdfArtifact(
    template,
    values,
    { body },
    1,
    { participants },
  )

  return new Response(new Uint8Array(artifact.bytes), {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': `inline; filename="listing-v2-${scenario}-dryrun.pdf"`,
      'Cache-Control': 'no-store',
      'X-Review-Page-Count': String(artifact.pageCount),
      'X-Review-Signature-Fields': String(artifact.signatureAnchors.length),
      'X-Review-Applied-Signatures': String(artifact.appliedSignatures.length),
    },
  })
}
