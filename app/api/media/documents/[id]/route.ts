import { sql } from '@/db/client'
import { getToken } from 'next-auth/jwt'

import { captureServerError } from '@/lib/server-error-capture'
import { withApiHandler } from '@/lib/error-capture-seam'
import { decideDocumentAccess } from '@/lib/auth/document-access'

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function contentDisposition(filename: string, download: boolean) {
  const disposition = download ? 'attachment' : 'inline'
  const safeFilename = filename
    .replace(/[\r\n]/g, '')
    .replace(/[^\x20-\x7E]/g, '_')
    .replace(/["\\]/g, '_')

  return `${disposition}; filename="${safeFilename}"; filename*=UTF-8''${encodeURIComponent(filename)}`
}

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
      SELECT file_data, filename, mime_type, file_size, media_type
      FROM media
      WHERE id = ${id}
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
      }
    | undefined

  const isDocument =
    row != null && row.media_type === 'document' && row.file_data != null

  // ONE rule, ONE place: the access decision is decideDocumentAccess.
  const decision = decideDocumentAccess({ isDocument, hasPortalSession })
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
  const headers = new Headers({
    'Content-Type': String(document.mime_type ?? 'application/octet-stream'),
    'Content-Disposition': contentDisposition(
      String(document.filename ?? 'document'),
      download,
    ),
    'Cache-Control': 'private, max-age=0, must-revalidate',
    'X-Content-Type-Options': 'nosniff',
  })

  if (document.file_size != null) {
    headers.set('Content-Length', String(document.file_size))
  }

  return new Response(document.file_data, {
    headers,
  })
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/media/documents/[id]', route: '/api/media/documents/[id]' },
  GETHandler,
)
