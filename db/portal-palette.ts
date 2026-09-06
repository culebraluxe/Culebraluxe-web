import { sql } from './client'

export type PortalPaletteClient = {
  id: string
  name: string
}

export type PortalPaletteDeal = {
  id: string
  name: string
  client: string
}

/**
 * Lightweight shell read for the command palette.
 * Do not hydrate full Client aggregates here: PortalLayout runs on every portal route.
 */
export async function getPortalPaletteClients(): Promise<PortalPaletteClient[]> {
  const rows = await sql`
    select
      mv.person_id as id,
      mv.display_name as name
    from mv_client_directory mv
    order by mv.name_sort_priority desc, mv.display_name asc, mv.person_id asc
  `

  return rows as PortalPaletteClient[]
}

/**
 * Lightweight shell read for the command palette.
 * Seven-ish active deals are cheap to resolve directly and this avoids the full Deal aggregate.
 */
export async function getPortalPaletteDeals(): Promise<PortalPaletteDeal[]> {
  const rows = await sql`
    select
      d.id,
      p.name,
      person.display_name as client
    from deal d
    join property p
      on p.id = d.property_id
    join lateral (
      select person.display_name
      from deal_participant dp
      join person
        on person.id = dp.person_id
      where dp.deal_id = d.id
        and dp.role = 'client'
        and dp.active = true
      order by dp.created_at asc
      limit 1
    ) person on true
    order by p.name asc, d.id asc
  `

  return rows as PortalPaletteDeal[]
}
