import { createHash } from 'node:crypto'

import { sql } from '@/db/client'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const TOKEN_SHA256 = '6974fc494777bd69e35c2e64c6042a18fc93dc0ebdf357430500a70887300227'
const LOGICAL_FILENAME = 'protected-broker-signature-lisa-penfield.png'
const PURPOSE = 'broker_signature:lisa_penfield'

function authorized(url: URL): boolean {
  const token = url.searchParams.get('token') ?? ''
  const digest = createHash('sha256').update(token).digest('hex')
  return token.length > 0 && digest === TOKEN_SHA256
}

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { 'Cache-Control': 'no-store' },
  })
}

export async function GET(request: Request) {
  const url = new URL(request.url)
  if (process.env.VERCEL_ENV !== 'production') return json({ ok: false, error: 'production only' }, 404)
  if (!authorized(url)) return json({ ok: false, error: 'not found' }, 404)

  const action = url.searchParams.get('action') ?? 'status'

  if (action === 'status') {
    const [columns, users, media] = await Promise.all([
      sql`
        select column_name
        from information_schema.columns
        where table_schema = 'public' and table_name = 'media'
        order by ordinal_position
      `,
      sql`
        select u.id, u.display_name, u.email, u.person_id, u.active,
          exists (
            select 1
            from app_user_role aur
            join role r on r.id = aur.role_id and r.active = true
            where aur.app_user_id = u.id and r.code = 'owner'
          ) as is_owner
        from app_user u
        where u.active = true
        order by u.display_name
      `,
      sql`
        select id, filename, mime_type, file_size, width, height, alt_text, caption,
               octet_length(file_data) as stored_bytes
        from media
        where filename = ${LOGICAL_FILENAME} or alt_text = ${PURPOSE}
        order by created_at desc
      `,
    ])
    return json({ ok: true, columns, users, media })
  }

  if (action === 'init') {
    const expectedSize = Number(url.searchParams.get('size') ?? '')
    const expectedSha = (url.searchParams.get('sha256') ?? '').trim().toLowerCase()
    const width = Number(url.searchParams.get('width') ?? '')
    const height = Number(url.searchParams.get('height') ?? '')
    if (!Number.isInteger(expectedSize) || expectedSize <= 0 || !/^[a-f0-9]{64}$/.test(expectedSha)) {
      return json({ ok: false, error: 'invalid metadata' }, 400)
    }
    if (!Number.isInteger(width) || width <= 0 || !Number.isInteger(height) || height <= 0) {
      return json({ ok: false, error: 'invalid dimensions' }, 400)
    }

    const existing = await sql`
      select id, octet_length(file_data) as stored_bytes, caption
      from media
      where filename = ${LOGICAL_FILENAME} or alt_text = ${PURPOSE}
      order by created_at desc
      limit 1
    `
    if (existing[0]) {
      const caption = String(existing[0].caption ?? '')
      if (caption.includes(expectedSha) && Number(existing[0].stored_bytes ?? 0) === expectedSize) {
        return json({ ok: true, mediaId: existing[0].id, storedBytes: existing[0].stored_bytes, reused: true })
      }
      return json({ ok: false, error: 'existing logical signature asset does not match requested checksum/size' }, 409)
    }

    const rows = await sql`
      insert into media (
        file_data, filename, mime_type, file_size, width, height, alt_text, caption
      ) values (
        ${Buffer.alloc(0)}, ${LOGICAL_FILENAME}, 'image/png', ${expectedSize}, ${width}, ${height},
        ${PURPOSE}, ${`sha256:${expectedSha}`}
      )
      returning id
    `
    return json({ ok: true, mediaId: rows[0]?.id ?? null, storedBytes: 0, reused: false })
  }

  if (action === 'chunk') {
    const mediaId = (url.searchParams.get('mediaId') ?? '').trim()
    const offset = Number(url.searchParams.get('offset') ?? '')
    const data = url.searchParams.get('data') ?? ''
    if (!mediaId || !Number.isInteger(offset) || offset < 0 || !data) {
      return json({ ok: false, error: 'invalid chunk metadata' }, 400)
    }
    let chunk: Buffer
    try {
      chunk = Buffer.from(data, 'base64url')
    } catch {
      return json({ ok: false, error: 'invalid chunk' }, 400)
    }
    if (chunk.length === 0 || chunk.length > 8192) return json({ ok: false, error: 'invalid chunk length' }, 400)

    const rows = await sql`
      update media
      set file_data = case
            when octet_length(file_data) = ${offset} then file_data || ${chunk}
            else file_data
          end,
          updated_at = now()
      where id = ${mediaId}
        and filename = ${LOGICAL_FILENAME}
        and alt_text = ${PURPOSE}
      returning octet_length(file_data) as stored_bytes
    `
    if (!rows[0]) return json({ ok: false, error: 'asset not found' }, 404)
    const storedBytes = Number(rows[0].stored_bytes ?? 0)
    const expectedAfter = offset + chunk.length
    if (storedBytes !== expectedAfter) {
      return json({ ok: false, error: 'chunk offset mismatch', storedBytes, expectedOffset: offset }, 409)
    }
    return json({ ok: true, storedBytes })
  }

  if (action === 'finalize') {
    const mediaId = (url.searchParams.get('mediaId') ?? '').trim()
    const expectedSize = Number(url.searchParams.get('size') ?? '')
    const expectedSha = (url.searchParams.get('sha256') ?? '').trim().toLowerCase()
    if (!mediaId || !Number.isInteger(expectedSize) || expectedSize <= 0 || !/^[a-f0-9]{64}$/.test(expectedSha)) {
      return json({ ok: false, error: 'invalid finalize metadata' }, 400)
    }
    const rows = await sql`
      select id, file_data, mime_type, file_size, width, height, alt_text, caption,
             octet_length(file_data) as stored_bytes
      from media
      where id = ${mediaId}
        and filename = ${LOGICAL_FILENAME}
        and alt_text = ${PURPOSE}
      limit 1
    `
    const row = rows[0]
    if (!row) return json({ ok: false, error: 'asset not found' }, 404)
    const bytes = Buffer.isBuffer(row.file_data)
      ? row.file_data
      : row.file_data instanceof Uint8Array
        ? Buffer.from(row.file_data)
        : Buffer.alloc(0)
    const actualSha = createHash('sha256').update(bytes).digest('hex')
    const storedBytes = bytes.length
    if (storedBytes !== expectedSize || actualSha !== expectedSha || row.mime_type !== 'image/png') {
      return json({ ok: false, error: 'asset verification failed', storedBytes, actualSha }, 409)
    }
    return json({
      ok: true,
      mediaId: row.id,
      storedBytes,
      sha256: actualSha,
      mimeType: row.mime_type,
      width: row.width,
      height: row.height,
      purpose: row.alt_text,
    })
  }

  return json({ ok: false, error: 'unknown action' }, 400)
}
