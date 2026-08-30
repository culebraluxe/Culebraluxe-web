import { sql } from '@/db/client'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    const media = await sql`
      select
        id,
        media_type,
        mime_type,
        octet_length(file_data) as byte_count,
        to_jsonb(media) - 'file_data' as metadata
      from media
      where media_type = 'image'
      order by created_at desc nulls last, id
      limit 50
    `
    const owners = await sql`
      select
        u.id,
        u.display_name,
        u.email,
        u.person_id,
        u.active,
        exists (
          select 1
          from app_user_role aur
          join role r on r.id = aur.role_id and r.active = true
          where aur.app_user_id = u.id and r.code = 'owner'
        ) as is_owner
      from app_user u
      where u.active = true
      order by u.display_name
    `
    return Response.json(
      { ok: true, media, owners },
      { headers: { 'Cache-Control': 'no-store' } },
    )
  } catch (error) {
    return Response.json(
      { ok: false, message: error instanceof Error ? error.message : String(error) },
      { status: 500, headers: { 'Cache-Control': 'no-store' } },
    )
  }
}
