import { sql } from './client'

/** Flip live/pending Clasificados and Facebook packs past expires_at. */
export async function expireStalePlacements(): Promise<{ expired: number }> {
  try {
    const rows = (await sql`
      update listing_syndication_placement
      set status = 'expired',
          last_error = coalesce(last_error, 'ttl_lapsed'),
          updated_at = now()
      where expires_at is not null
        and expires_at < now()
        and status in ('live', 'pending_manual')
        and channel in ('clasificados', 'facebook_marketplace')
      returning id
    `) as Array<{ id: string }>
    for (const row of rows) {
      await sql`
        insert into listing_syndication_event (placement_id, event_type, detail)
        values (${row.id}, 'note', ${JSON.stringify({ reason: 'ttl_lapsed' })})
      `
    }
    return { expired: rows.length }
  } catch {
    return { expired: 0 }
  }
}
