import { createHash } from 'node:crypto'
import { sql } from '@/db/client'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const TOKEN_SHA256 = '6974fc494777bd69e35c2e64c6042a18fc93dc0ebdf357430500a70887300227'
const MEDIA_ID = '2647926b-8ed5-401d-a8d6-81c9fd166ea1'
const PURPOSE = 'broker_signature:lisa_penfield'
const EXPECTED_SHA256 = 'db2d027da0b8c71192b02da22034e60773691a2a11c9280719fd6598a597e375'

export async function GET(request: Request) {
  const url = new URL(request.url)
  const token = url.searchParams.get('token') ?? ''
  const digest = createHash('sha256').update(token).digest('hex')
  if (process.env.VERCEL_ENV !== 'production' || !token || digest !== TOKEN_SHA256) {
    return new Response('not found', { status: 404 })
  }
  const rows = await sql`
    update media
    set file_data = ${Buffer.alloc(0)},
        filename = 'protected-broker-signature-lisa-penfield.png',
        mime_type = 'image/png',
        media_type = 'image',
        file_size = 10676,
        width = 1000,
        height = 313,
        alt_text = ${PURPOSE},
        caption = ${`sha256:${EXPECTED_SHA256}`},
        updated_at = now()
    where id = ${MEDIA_ID} and alt_text = ${PURPOSE}
    returning id, octet_length(file_data) as stored_bytes, file_size, width, height, caption
  `
  return Response.json({ ok: rows.length === 1, row: rows[0] ?? null }, { headers: { 'Cache-Control': 'no-store' } })
}
