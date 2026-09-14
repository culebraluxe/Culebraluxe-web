import { NextResponse, type NextRequest } from 'next/server'
import { strToU8, zipSync } from 'fflate'
import { sql } from '@/db/client'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { photoDownloadBase, mediaPublicPath } from '@/lib/syndication/media'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

type PhotoRow = { media_id: string; filename: string | null; file_data: unknown }

function uniqueName(index: number, filename: string | null): string {
  const base = (filename || `${index + 1}.jpg`).replace(/[^\w.-]+/g, '_')
  return `${String(index + 1).padStart(2, '0')}-${base}`
}

export async function GET(request: NextRequest) {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'portal.read')
  if (!access.ok) return new NextResponse('Unauthorized', { status: 401 })

  const propertyId = request.nextUrl.searchParams.get('propertyId')
  if (!propertyId) return new NextResponse('Bad propertyId', { status: 400 })

  const meta = (await sql`
    select p.slug, p.name from property p where p.id = ${propertyId} limit 1
  `) as Array<{ slug: string | null; name: string }>
  if (!meta[0]) return new NextResponse('Property not found', { status: 404 })
  const base = photoDownloadBase(meta[0].slug, meta[0].name)

  const rows = (await sql`
    select m.id as media_id, m.filename, m.file_data
    from property_media pm
    join media m on m.id = pm.media_id
    where pm.property_id = ${propertyId} and m.media_type = 'image'
    order by (pm.role = 'hero') desc, pm.sort_order asc nulls last, m.created_at asc
    limit 25
  `) as PhotoRow[]

  const files: Record<string, Uint8Array> = {}
  const urlLines: string[] = []
  let hasBytes = false
  rows.forEach((row, i) => {
    const bytes = row.file_data as Uint8Array | Buffer | null | undefined
    if (bytes && bytes.length > 0) {
      files[uniqueName(i, row.filename)] = new Uint8Array(bytes)
      hasBytes = true
    } else {
      urlLines.push(mediaPublicPath(row.media_id))
    }
  })

  if (!hasBytes) {
    if (rows.length === 0) return new NextResponse('No photos on this property.', { status: 404 })
    return new NextResponse(urlLines.join('\n'), {
      status: 200,
      headers: {
        'Content-Type': 'text/plain',
        'Content-Disposition': `attachment; filename="${base}-urls.txt"`,
      },
    })
  }

  // Rows without bytes go in a sidecar .txt inside the zip.
  files['photos-urls.txt'] = strToU8(urlLines.join('\n'))
  const zipped = zipSync(files, { level: 6 })
  return new NextResponse(zipped, {
    status: 200,
    headers: {
      'Content-Type': 'application/zip',
      'Content-Disposition': `attachment; filename="${base}-photos.zip"`,
    },
  })
}
