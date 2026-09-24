import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import { RustApiError, rustApiSetUserPrimaryRole } from '@/lib/rust-api/client'

type SecurityUser = {
  appUserId: string
  displayName: string
  email: string | null
  accountType: string
  active: boolean
  roleCodes: string[]
  primaryRoleCode: string | null
}

const CANONICAL_INTERNAL_ROLES = new Set([
  'internal_guest',
  'user',
  'business_power_user',
  'owner',
  'root',
])

async function PUTHandler(req: NextRequest): Promise<Response> {
  let body: unknown
  try {
    body = await req.json()
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body.' }, { status: 400 })
  }
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return NextResponse.json({ error: 'Invalid role assignment.' }, { status: 400 })
  }

  const value = body as Record<string, unknown>
  if (
    typeof value.appUserId !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value.appUserId) ||
    typeof value.roleCode !== 'string' ||
    !CANONICAL_INTERNAL_ROLES.has(value.roleCode)
  ) {
    return NextResponse.json({ error: 'Invalid user or canonical role.' }, { status: 400 })
  }

  try {
    const result = await rustApiSetUserPrimaryRole<SecurityUser[]>({
      appUserId: value.appUserId,
      roleCode: value.roleCode,
    })
    return NextResponse.json({ users: result.value })
  } catch (error) {
    if (error instanceof RustApiError && error.status < 500) {
      return NextResponse.json({ error: error.message }, { status: error.status })
    }
    throw error
  }
}

export const PUT = withApiHandler(
  { label: '/api/portal/rust-ui/security-users', route: '/api/portal/rust-ui/security-users' },
  PUTHandler,
)
