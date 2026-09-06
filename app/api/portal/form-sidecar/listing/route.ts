import { NextRequest, NextResponse } from 'next/server'

import { AuthError } from '@/lib/auth/errors'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import {
  loadListingCanonicalSnapshot,
  saveListingCanonicalFields,
} from '@/lib/forms/listing-canonical-binding'
import type { SaveListingCanonicalFieldsRequest } from '@/lib/forms/listing-field-binding'

function message(error: unknown): string {
  return error instanceof Error ? error.message : 'Listing canonical binding failed.'
}

export async function GET(request: NextRequest) {
  try {
    return await runAuthorized(
      getPortalSessionAdapter(),
      'portal.read',
      async () => {
        const personId = request.nextUrl.searchParams.get('personId')?.trim() || ''
        if (!personId) {
          return NextResponse.json({ error: 'personId is required.' }, { status: 400 })
        }
        const snapshot = await loadListingCanonicalSnapshot(personId)
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
      'listing.write',
      async (actor) => {
        const body = (await request.json()) as Partial<SaveListingCanonicalFieldsRequest>
        if (!body.personId?.trim() || !body.fields) {
          return NextResponse.json(
            { error: 'personId and fields are required.' },
            { status: 400 },
          )
        }
        const snapshot = await saveListingCanonicalFields(
          body.personId,
          body.fields,
          actor.appUserId,
          body.physicalPropertyId,
        )
        return NextResponse.json(snapshot)
      },
    )
  } catch (error) {
    const status = error instanceof AuthError ? 403 : 500
    return NextResponse.json({ error: message(error) }, { status })
  }
}
