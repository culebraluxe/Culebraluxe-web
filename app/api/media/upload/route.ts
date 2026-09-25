import { NextResponse } from 'next/server'

import { guardPortalUpload } from '@/lib/auth/portal-session'
import { rustApiStandaloneMediaUpload } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

async function POSTHandler(request: Request) {
  const guard = await guardPortalUpload('listing.write')
  if (!guard.ok) {
    return NextResponse.json({ error: guard.error }, { status: guard.status })
  }

  const formData = await request.formData()
  const file = formData.get('file')
  if (!(file instanceof File)) {
    return NextResponse.json({ error: 'No file uploaded' }, { status: 400 })
  }

  try {
    return NextResponse.json(await rustApiStandaloneMediaUpload(file))
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : 'Media upload failed.',
      },
      { status: 400 },
    )
  }
}

export const POST = withApiHandler(
  { label: '/api/media/upload', route: '/api/media/upload' },
  POSTHandler,
)
