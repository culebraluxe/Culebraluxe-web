import { NextRequest, NextResponse } from 'next/server'

import { AuthError } from '@/lib/auth/errors'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import {
  loadPnsCanonicalSnapshot,
  savePnsCanonicalFields,
} from '@/lib/forms/pns-canonical-binding'
import type { SavePnsCanonicalRequest } from '@/lib/forms/pns-canonical-types'

function message(error: unknown): string {
  return error instanceof Error ? error.message : 'P&S canonical binding failed.'
}

export async function GET(request: NextRequest) {
  try {
    return await runAuthorized(
      getPortalSessionAdapter(),
      'portal.read',
      async () => {
        const personId = request.nextUrl.searchParams.get('personId')?.trim() || ''
        const contractId = request.nextUrl.searchParams.get('contractId')?.trim() || null
        if (!personId) {
          return NextResponse.json({ error: 'personId is required.' }, { status: 400 })
        }
        const snapshot = await loadPnsCanonicalSnapshot(personId, contractId)
        return NextResponse.json(snapshot)
      },
    )
  } catch (error) {
    const status = error instanceof AuthError ? 403 : 500
    return NextResponse.json({ error: message(error) }, { status })
  }
}

export async function POST(request: NextRequest) {
  try {
    return await runAuthorized(
      getPortalSessionAdapter(),
      // Compatibility authority while Contract is replacing Deal truth.
      // Business-service authorization remains contract.write at the service seam.
      'deal.write',
      async (actor) => {
        const body = (await request.json()) as Partial<SavePnsCanonicalRequest>
        if (!body.personId?.trim() || !body.fields) {
          return NextResponse.json(
            { error: 'personId and fields are required.' },
            { status: 400 },
          )
        }
        const snapshot = await savePnsCanonicalFields(
          {
            personId: body.personId,
            contractId: body.contractId ?? null,
            physicalPropertyId: body.physicalPropertyId ?? null,
            fields: body.fields,
          },
          actor.appUserId,
        )
        return NextResponse.json(snapshot)
      },
    )
  } catch (error) {
    const status = error instanceof AuthError ? 403 : 500
    return NextResponse.json({ error: message(error) }, { status })
  }
}
