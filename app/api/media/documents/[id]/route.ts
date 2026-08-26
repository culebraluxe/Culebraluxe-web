import { sql } from '@/db/client'

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

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params

  if (!UUID_PATTERN.test(id)) {
    return new Response('Not found', { status: 404 })
  }

  const result = await sql`
    SELECT file_data, filename, mime_type, file_size
    FROM media
    WHERE id = ${id}
      AND media_type = 'document'
      AND file_data IS NOT NULL
    LIMIT 1
  `

  if (result.length === 0) {
    return new Response('Not found', { status: 404 })
  }

  const row = result[0] as
    | {
        file_data: Buffer | Uint8Array | null
        filename: string | null
        mime_type: string | null
        file_size: string | number | null
      }
    | undefined
  if (!row) {
    return new Response('Not found', { status: 404 })
  }
  const download = new URL(request.url).searchParams.get('download') === '1'
  const headers = new Headers({
    'Content-Type': String(row.mime_type ?? 'application/octet-stream'),
    'Content-Disposition': contentDisposition(String(row.filename ?? 'document'), download),
    'Cache-Control': 'private, max-age=0, must-revalidate',
    'X-Content-Type-Options': 'nosniff',
  })

  if (row.file_size != null) {
    headers.set('Content-Length', String(row.file_size))
  }

  return new Response(row.file_data, {
    headers,
  })
}
