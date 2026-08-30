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
        and (
          lower(coalesce(mime_type, '')) = 'image/png'
          or octet_length(file_data) < 300000
          or lower((to_jsonb(media) - 'file_data')::text) like '%signature%'
          or lower((to_jsonb(media) - 'file_data')::text) like '%lisa%'
          or lower((to_jsonb(media) - 'file_data')::text) like '%penfield%'
        )
      order by created_at desc nulls last, id
    `
    const counts = await sql`
      select
        count(*) filter (where media_type = 'image') as image_count,
        count(*) filter (where media_type = 'image' and lower(coalesce(mime_type, '')) = 'image/png') as png_count
      from media
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
      { ok: true, counts: counts[0] ?? null, media, owners },
      { headers: { 'Cache-Control': 'no-store' } },
    )
  } catch (error) {
    return Response.json(
      { ok: false, message: error instanceof Error ? error.message : String(error) },
      { status: 500, headers: { 'Cache-Control': 'no-store' } },
    )
  }
}
