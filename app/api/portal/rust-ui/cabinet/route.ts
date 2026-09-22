import { NextResponse } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

type CabinetDocument = {
  id: string
  dealId: string | null
  propertyId: string | null
  documentTypeLabel: string | null
  title: string | null
  state: string
  templateId: string | null
  templateVersion: number | null
  issuedVersion: number | null
  issuedChecksumSha256: string | null
  issuedByDisplayName: string | null
  partyName: string | null
  propertyName: string | null
  dealName: string | null
  createdAt: string
  signedArtifactAvailable: boolean
  signedAuditAvailable: boolean
}

async function GETHandler(): Promise<Response> {
  const result = await rustApiRead<CabinetDocument[]>('/v1/vault/documents')
  return NextResponse.json({ cabinet: { documents: result.value } })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/cabinet', route: '/api/portal/rust-ui/cabinet' },
  GETHandler,
)
