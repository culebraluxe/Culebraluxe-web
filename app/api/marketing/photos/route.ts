import { NextResponse, type NextRequest } from 'next/server'
import { sql } from '@/db/client'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { photoDownloadBase, photoUrlList } from '@/lib/syndication/media'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

type PhotoRow = { media_id: string }

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
    select m.id as media_id
    from property_media pm
    join media m on m.id = pm.media_id
    where pm.property_id = ${propertyId} and m.media_type = 'image'
    order by (pm.role = 'hero') desc, pm.sort_order asc nulls last, m.created_at asc
    limit 25
  `) as PhotoRow[]

  if (rows.length === 0) {
    return new NextResponse('No photos on this property.', { status: 404 })
  }

  // A server-side ZIP of `media.file_data` bytes needs an archive library whose
  // typings expose a callable factory; this pass ships the URL list (.txt), which
  // is the documented fallback when bytes are unavailable. See the notes doc.
  const txt = photoUrlList(rows).join('\n')
  return new NextResponse(txt, {
    status: 200,
    headers: {
      'Content-Type': 'text/plain',
      'Content-Disposition': `attachment; filename="${base}-urls.txt"`,
    },
  })
}
