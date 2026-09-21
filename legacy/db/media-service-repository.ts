import { sql } from '@/legacy/db/client'
import type { QueryExecutor } from '@/legacy/db/query-executor'
import type { MediaAssetDto, MediaRepository } from '@/legacy/services/media'

type MediaRow = {
  property_id: string
  media_id: string
  media_type: string
  role: string
  sort_order: number | string | null
  filename: string | null
  mime_type: string | null
  file_size: number | string | null
  alt_text: string | null
  caption: string | null
  created_at: string | Date | null
}

function finiteNumber(value: number | string | null | undefined): number | null {
  if (value == null) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

function toIso(value: string | Date | null | undefined): string | null {
  if (value == null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

/** SQL adapter behind MediaService. No screen reaches media tables directly. */
export class SqlMediaRepository implements MediaRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async forProperty(propertyId: string): Promise<MediaAssetDto[]> {
    const rows = (await this.execute`
      select
        pm.property_id,
        m.id as media_id,
        m.media_type,
        pm.role,
        pm.sort_order,
        m.filename,
        m.mime_type,
        m.file_size,
        m.alt_text,
        m.caption,
        pm.created_at
      from property_media pm
      join media m on m.id = pm.media_id
      where pm.property_id = ${propertyId}
      order by pm.sort_order asc nulls last, pm.created_at asc, m.id asc
    `) as MediaRow[]

    return rows.map((row) => ({
      id: row.media_id,
      propertyId: row.property_id,
      mediaType: row.media_type,
      role: row.role,
      sortOrder: finiteNumber(row.sort_order) ?? 0,
      filename: row.filename,
      mimeType: row.mime_type,
      fileSize: finiteNumber(row.file_size),
      altText: row.alt_text,
      caption: row.caption,
      createdAt: toIso(row.created_at),
      url: `/api/media/${row.media_id}`,
    }))
  }
}
