import { createHash } from 'node:crypto'

import { resolveBrokerSignatureForIssuance } from '@/db/broker-signature'
import { sql } from '@/db/client'
import { renderFormPdfArtifact } from '@/lib/forms/pdf'
import { getTemplate } from '@/lib/forms/template-registry'
import type { IssuedExecutionSlot } from '@/lib/agreements/execution'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const TOKEN_SHA256 = '6974fc494777bd69e35c2e64c6042a18fc93dc0ebdf357430500a70887300227'

export async function GET(request: Request) {
  const url = new URL(request.url)
  const token = url.searchParams.get('token') ?? ''
  const digest = createHash('sha256').update(token).digest('hex')
  if (process.env.VERCEL_ENV !== 'production' || !token || digest !== TOKEN_SHA256) {
    return new Response('not found', { status: 404 })
  }

  const template = getTemplate('LISTING-01', 2)
  if (!template) return Response.json({ ok: false, error: 'template missing' }, { status: 500 })

  const values = {
    sellerName: 'Production Signature Proof',
    sellerCivilStatus: 'Single',
    sellerResidenceAddress: 'Culebra, Puerto Rico',
    brokerName: 'Lisa Penfield',
    property: 'Signature Proof Property',
    propertyLocation: 'Culebra, Puerto Rico',
    catastroNumber: 'PROOF',
    listPrice: '650000',
    commission: '5%',
    startDate: '2026-08-30',
    endDate: '2027-08-30',
    listingType: 'Exclusive Right to Sell',
  }
  const participants: IssuedExecutionSlot[] = [
    {
      slotId: 'SELLER:1',
      role: 'SELLER',
      personId: null,
      name: 'Production Signature Proof',
      email: 'proof@example.com',
      required: true,
      order: 1,
    },
    {
      slotId: 'SELLER_BROKER:1',
      role: 'SELLER_BROKER',
      personId: null,
      name: 'Lisa Penfield',
      email: 'lisa@culebraluxe.com',
      required: true,
      order: 2,
    },
  ]
  const resolved = await resolveBrokerSignatureForIssuance(
    {
      template,
      values,
      participants,
      actorAppUserId: 'authenticated-production-proof-operator',
      issuedAt: '2026-08-30T14:00:00.000Z',
    },
    sql,
  )
  if (!resolved.ok) return Response.json({ ok: false, resolver: resolved }, { status: 500 })
  const artifact = await renderFormPdfArtifact(template, values, {}, 1, {
    participants,
    appliedSignatures: resolved.signatures,
  })
  const evidence = artifact.appliedSignatures[0] ?? null
  return Response.json(
    {
      ok: Boolean(
        evidence &&
          evidence.signerName === 'Lisa Penfield' &&
          evidence.renderedInitials === 'LP' &&
          evidence.renderedDate &&
          evidence.credentialLine === 'Real Estate Broker License #: C-9931',
      ),
      resolverSignatureCount: resolved.signatures.length,
      renderedSignatureCount: artifact.appliedSignatures.length,
      evidence: evidence
        ? {
            signerName: evidence.signerName,
            renderedInitials: evidence.renderedInitials,
            renderedDate: evidence.renderedDate,
            credentialLine: evidence.credentialLine,
            assetMediaId: evidence.assetMediaId,
            assetChecksumSha256: evidence.assetChecksumSha256,
            slotId: evidence.slotId,
            signatureRect: evidence.signatureRect,
            initialsRect: evidence.initialsRect,
            dateRect: evidence.dateRect,
          }
        : null,
      pageCount: artifact.pageCount,
    },
    { headers: { 'Cache-Control': 'no-store' } },
  )
}
