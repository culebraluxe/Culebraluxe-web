import { NextResponse } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiRead } from '@/lib/rust-api/client'

type PrincipalGrants = {
  accountType: string
  securityLevel: string
  roleCodes: string[]
  entitlementCodes: string[]
}

async function GETHandler(): Promise<Response> {
  // Transport only. Identity is resolved and grants are projected by Rust's
  // Security service; the browser receives no provider credentials.
  const result = await rustApiRead<PrincipalGrants>('/v1/whoami')
  return NextResponse.json({
    accountType: result.value.accountType,
    securityLevel: result.value.securityLevel,
    isRoot: result.value.roleCodes.includes('root'),
    entitlementCodes: result.value.entitlementCodes,
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/entitlements', route: '/api/portal/rust-ui/entitlements' },
  GETHandler,
)
