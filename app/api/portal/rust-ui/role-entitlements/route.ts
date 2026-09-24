import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { RustApiError, rustApiSetRoleEntitlement } from '@/lib/rust-api/client'

type RoleGrant = { roleCode: string; accountType: string; entitlementCodes: string[] }

async function PUTHandler(req: NextRequest): Promise<Response> {
  let body: unknown
  try {
    body = await req.json()
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body.' }, { status: 400 })
  }
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return NextResponse.json({ error: 'Invalid role grant.' }, { status: 400 })
  }
  const value = body as Record<string, unknown>
  if (typeof value.roleCode !== 'string' || !/^[a-z_]{1,64}$/.test(value.roleCode)
      || typeof value.action !== 'string' || !/^[a-z][a-z.]{1,100}$/.test(value.action)
      || typeof value.granted !== 'boolean') {
    return NextResponse.json({ error: 'Invalid role or action.' }, { status: 400 })
  }

  try {
    const result = await rustApiSetRoleEntitlement<RoleGrant[]>({
      roleCode: value.roleCode,
      action: value.action,
      granted: value.granted,
    })
    return NextResponse.json({ roles: result.value })
  } catch (error) {
    if (error instanceof RustApiError && error.status < 500) {
      return NextResponse.json({ error: error.message }, { status: error.status })
    }
    throw error
  }
}

export const PUT = withApiHandler(
  { label: '/api/portal/rust-ui/role-entitlements', route: '/api/portal/rust-ui/role-entitlements' },
  PUTHandler,
)
