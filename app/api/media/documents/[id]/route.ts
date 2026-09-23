import { sql } from '@/legacy/db/client'
import { getToken } from 'next-auth/jwt'

import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'
import { decideDocumentAccess } from '@/lib/auth/document-access'

import { buildDocumentResponse } from './document-response'

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

async function GETHandler(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params

  if (!UUID_PATTERN.test(id)) {
    return new Response('Not found', { status: 404 })
  }

  // Authenticated portal session — lightweight JWT decoder only, so this route
  // never constructs Auth.js. A document is not served without a session.
  const secret = process.env.AUTH_SECRET
  let hasPortalSession = false
  if (secret) {
    try {
      const token = await getToken({ req: request, secret })
      hasPortalSession = Boolean(token?.sub)
    } catch (err) {
      // A present secret that cannot decode is a real failure, not "anonymous":
      // capture it, then fail closed (no bytes).
      captureServerError('/api/media/documents/[id]', err, {
        route: '/api/media/documents/[id]',
      })
      hasPortalSession = false
    }
  }

  let result: Array<Record<string, unknown>>
  try {
    result = await sql`
      SELECT m.file_data, m.filename, m.mime_type, m.file_size, m.media_type,
        EXISTS (
          SELECT 1 FROM property_media pm
          WHERE pm.media_id = m.id AND pm.role = 'document'
        )
        AND NOT EXISTS (
          SELECT 1 FROM property_media pm
          LEFT JOIN property p ON p.id = pm.property_id
          WHERE pm.media_id = m.id
            AND (pm.role <> 'document' OR p.id IS NULL OR p.is_published IS DISTINCT FROM true
              OR p.is_active_listing IS DISTINCT FROM true OR p.archived_at IS NOT NULL)
        )
        AND NOT EXISTS (
          SELECT 1 FROM transaction_document td
          WHERE td.media_id = m.id OR td.signed_media_id = m.id
        ) AS is_public_listing_document
      FROM media m
      WHERE m.id = ${id}
      LIMIT 1
    `
  } catch (err) {
    captureServerError('/api/media/documents/[id]', err, { route: '/api/media/documents/[id]' })
    // DB-HARDEN-01C — media read: controlled 503 on DB failure.
    console.error(
      '[media:gateway] document read failed',
      err instanceof Error ? err.message : 'unknown',
    )
    return new Response('Service Unavailable', { status: 503 })
  }

  const row = result[0] as
    | {
        file_data: Buffer | Uint8Array | null
        filename: string | null
        mime_type: string | null
        file_size: string | number | null
        media_type: string | null
        is_public_listing_document: boolean
      }
    | undefined

  const isDocument =
    row != null && row.media_type === 'document' && row.file_data != null

  // ONE rule, ONE place: the access decision is decideDocumentAccess.
  const decision = decideDocumentAccess({
    isDocument,
    hasPortalSession,
    isPublicListingDocument: row?.is_public_listing_document === true,
  })
  if (!decision.allow) {
    return decision.reason === 'unauthenticated'
      ? new Response('Unauthorized', { status: 401 })
      : new Response('Not found', { status: 404 })
  }

  const document = row as {
    file_data: Buffer | Uint8Array
    filename: string | null
    mime_type: string | null
    file_size: string | number | null
  }
  const download = new URL(request.url).searchParams.get('download') === '1'

  // The length comes from the bytes sent, never from `file_size` — see
  // document-response.ts. `file_size` is passed only so a reader can see the
  // value the header deliberately does NOT use.
  return buildDocumentResponse({
    fileData: document.file_data,
    filename: document.filename,
    mimeType: document.mime_type,
    download,
    declaredFileSize: document.file_size,
  })
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/media/documents/[id]', route: '/api/media/documents/[id]' },
  GETHandler,
)
