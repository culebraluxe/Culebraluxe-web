import { NextResponse } from 'next/server'

import { captureServerError } from '@/lib/server-error-capture'
import { guardPortalUpload } from '@/lib/auth/portal-session'
import { MAX_MEDIA_UPLOAD_BYTES } from '@/lib/media/upload-policy'
import { rustApiWriteForm } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

export const runtime = 'nodejs'

async function POSTHandler(request: Request) {
  try {
    // AUTH-03: authenticated Portal write — resolve the acting user and require
    // listing.write BEFORE any multipart/work. Denied callers get 401/403 and
    // never reach the inserts below.
    const guard = await guardPortalUpload('listing.write')
    if (!guard.ok) {
      return NextResponse.json(
        { error: guard.error },
        { status: guard.status },
      )
    }

    const formData = await request.formData()

    const propertyId = formData.get('propertyId')
    const role = formData.get('role')
    const altText = formData.get('altText')
    const file = formData.get('file')

    if (
      typeof propertyId !== 'string' ||
      propertyId.length === 0
    ) {
      return NextResponse.json(
        { error: 'Property is required.' },
        { status: 400 },
      )
    }

    if (
      role !== 'hero' &&
      role !== 'gallery'
    ) {
      return NextResponse.json(
        { error: 'Invalid media role.' },
        { status: 400 },
      )
    }

    if (!(file instanceof File)) {
      return NextResponse.json(
        { error: 'Image file is required.' },
        { status: 400 },
      )
    }

    if (!file.type.startsWith('image/')) {
      return NextResponse.json(
        { error: 'Only image uploads are supported.' },
        { status: 400 },
      )
    }

    // HARDEN-06: bounded image size + sanitized filename.
    if (file.size > MAX_MEDIA_UPLOAD_BYTES) {
      return NextResponse.json(
        { error: 'Image is too large (max 50 MB).' },
        { status: 400 },
      )
    }

    const rustFormData = new FormData()
    rustFormData.set('role', role)
    if (typeof altText === 'string' && altText.trim().length > 0) {
      rustFormData.set('altText', altText.trim())
    }
    rustFormData.set('file', file, file.name)

    const result = await rustApiWriteForm<{
      ok: true
      mediaId: string
      propertyId: string
      role: 'hero' | 'gallery'
    }>(
      `/v1/properties/${encodeURIComponent(propertyId)}/media`,
      rustFormData,
    )

    return NextResponse.json(result.value)
  } catch (error) {
    captureServerError('/api/property-media/upload', error, { route: '/api/property-media/upload' })
    console.error('Property media upload failed:', error)

    return NextResponse.json(
      { error: 'Property media upload failed.' },
      { status: 500 },
    )
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const POST = withApiHandler(
  { label: '/api/property-media/upload', route: '/api/property-media/upload' },
  POSTHandler,
)
