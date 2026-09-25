import { NextResponse } from 'next/server'

import { captureServerError } from '@/lib/server-error-capture'
import { guardPortalUpload } from '@/lib/auth/portal-session'
import { MAX_MEDIA_UPLOAD_BYTES } from '@/lib/media/upload-policy'
import { rustApiWriteForm } from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

export const runtime = 'nodejs'

/**
 * The browser half of a chunked photo upload.
 *
 * WHY THIS ROUTE EXISTS. The browser cannot hold the Rust API's bridge key, so every call to that API goes through
 * the server — which is also where the acting user is resolved and `listing.write` is required, before any bytes
 * move. This is a thin proxy and nothing else: it checks the shape, forwards one step, returns what the container
 * said.
 *
 * WHY CHUNKS. The gateway refuses a request body over ~4.5 MB, and a photograph from a phone is 8–13 MB. The browser
 * splits the file; the container reassembles it.
 */
async function POSTHandler(request: Request) {
  try {
    const guard = await guardPortalUpload('listing.write')
    if (!guard.ok) {
      return NextResponse.json({ error: guard.error }, { status: guard.status })
    }

    const formData = await request.formData()
    const step = formData.get('step')
    const propertyId = formData.get('propertyId')

    if (typeof propertyId !== 'string' || propertyId.length === 0) {
      return NextResponse.json({ error: 'Property is required.' }, { status: 400 })
    }

    const propertyPath = `/v1/properties/${encodeURIComponent(propertyId)}/media/uploads`

    if (step === 'init') {
      const filename = formData.get('filename')
      const mimeType = formData.get('mimeType')
      const byteSize = Number(formData.get('byteSize'))
      const chunkCount = Number(formData.get('chunkCount'))
      const chunkSize = Number(formData.get('chunkSize'))
      const role = formData.get('role') ?? 'gallery'
      const altText = formData.get('altText')

      if (typeof filename !== 'string' || filename.length === 0) {
        return NextResponse.json({ error: 'An image filename is required.' }, { status: 400 })
      }
      if (
        !Number.isFinite(byteSize) ||
        !Number.isFinite(chunkCount) ||
        !Number.isFinite(chunkSize) ||
        byteSize <= 0 ||
        chunkCount <= 0 ||
        chunkSize <= 0
      ) {
        return NextResponse.json(
          { error: 'The upload must declare a positive size, chunk count and chunk size.' },
          { status: 400 },
        )
      }
      // The same 50 MB ceiling the single-shot path enforced, so the chunked path cannot smuggle past it.
      if (byteSize > MAX_MEDIA_UPLOAD_BYTES) {
        return NextResponse.json({ error: 'Image is too large (max 50 MB).' }, { status: 400 })
      }
      if (role !== 'hero' && role !== 'gallery') {
        return NextResponse.json({ error: 'Invalid media role.' }, { status: 400 })
      }
      if (typeof mimeType !== 'string' || !mimeType.startsWith('image/')) {
        return NextResponse.json({ error: 'Only image uploads are supported.' }, { status: 400 })
      }

      const rustFormData = new FormData()
      rustFormData.set('filename', filename)
      rustFormData.set('mimeType', mimeType)
      rustFormData.set('byteSize', String(byteSize))
      rustFormData.set('chunkCount', String(chunkCount))
      rustFormData.set('chunkSize', String(chunkSize))
      rustFormData.set('role', role)
      if (typeof altText === 'string' && altText.trim().length > 0) {
        rustFormData.set('altText', altText.trim())
      }
      const sha256 = formData.get('sha256')
      if (typeof sha256 === 'string' && sha256.length > 0) {
        rustFormData.set('sha256', sha256)
      }

      const result = await rustApiWriteForm<{
        ok: true
        uploadId: string
        chunkSize: number
        chunkCount: number
        byteSize: number
      }>(propertyPath, rustFormData)

      return NextResponse.json(result.value)
    }

    const uploadId = formData.get('uploadId')
    if (typeof uploadId !== 'string' || uploadId.length === 0) {
      return NextResponse.json({ error: 'An upload id is required.' }, { status: 400 })
    }

    if (step === 'chunk') {
      const chunk = formData.get('chunk')
      const chunkIndex = Number(formData.get('chunkIndex'))
      if (!(chunk instanceof File) || !Number.isFinite(chunkIndex) || chunkIndex < 0) {
        return NextResponse.json({ error: 'An image chunk is required.' }, { status: 400 })
      }

      const rustFormData = new FormData()
      rustFormData.set('chunk', chunk, 'chunk')

      const result = await rustApiWriteForm<{
        ok: true
        receivedChunks: number
        chunkCount: number
      }>(
        `${propertyPath}/${encodeURIComponent(uploadId)}/chunks/${chunkIndex}`,
        rustFormData,
      )

      return NextResponse.json(result.value)
    }

    if (step === 'complete') {
      const result = await rustApiWriteForm<{
        ok: true
        mediaId: string
        propertyId: string
        role: 'hero' | 'gallery'
      }>(`${propertyPath}/${encodeURIComponent(uploadId)}/complete`, new FormData())

      return NextResponse.json(result.value)
    }

    return NextResponse.json({ error: 'Unknown upload step.' }, { status: 400 })
  } catch (error) {
    captureServerError('/api/property-media/chunked', error, {
      route: '/api/property-media/chunked',
    })
    console.error('Property media chunked upload failed:', error)

    return NextResponse.json({ error: 'Property media upload failed.' }, { status: 500 })
  }
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const POST = withApiHandler(
  { label: '/api/property-media/chunked', route: '/api/property-media/chunked' },
  POSTHandler,
)
