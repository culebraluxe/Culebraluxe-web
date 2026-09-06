import { NextRequest, NextResponse } from 'next/server'

import { getFormInstance } from '@/db/document-form-instance'
import { bindListingFormContext } from '@/db/form-service-lineage'
import { AuthError } from '@/lib/auth/errors'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import { loadListingCanonicalSnapshot } from '@/lib/forms/listing-canonical-binding'

function message(error: unknown): string {
  return error instanceof Error ? error.message : 'Could not select the Listing client.'
}

export async function POST(request: NextRequest) {
  try {
    return await runAuthorized(
      getPortalSessionAdapter(),
      'listing.write',
      async () => {
        const body = (await request.json()) as {
          formId?: string
          personId?: string
        }
        const formId = body.formId?.trim() ?? ''
        const personId = body.personId?.trim() ?? ''
        if (!formId || !personId) {
          return NextResponse.json(
            { error: 'formId and personId are required.' },
            { status: 400 },
          )
        }

        const form = await getFormInstance(formId)
        if (!form || form.templateId !== 'LISTING-01') {
          return NextResponse.json(
            { error: 'Listing Agreement draft not found.' },
            { status: 404 },
          )
        }
        if (form.status === 'issued') {
          return NextResponse.json(
            { error: 'Issued Listing Agreements cannot change client context.' },
            { status: 409 },
          )
        }

        const snapshot = await loadListingCanonicalSnapshot(personId)
        await bindListingFormContext({
          formInstanceId: formId,
          personId,
          propertyId: snapshot.physicalPropertyId,
        })
        return NextResponse.json(snapshot)
      },
    )
  } catch (error) {
    const status = error instanceof AuthError ? 403 : 409
    return NextResponse.json({ error: message(error) }, { status })
  }
}
