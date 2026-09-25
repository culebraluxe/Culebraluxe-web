import 'server-only'

import { rustApiRead } from '@/lib/rust-api/client'

export type PortalPaletteClient = {
  id: string
  name: string
}

export type PortalPaletteDeal = {
  id: string
  name: string
  client: string
}

type ClientsPageResult = {
  rows: Array<{
    id: string
    displayName: string
  }>
  total: number
  page: number
  pageSize: number
}

type DealPortfolioSnapshot = {
  deals: Array<{
    id: string
    propertyName: string
    clientName: string
  }>
}

/**
 * Lightweight command-palette projection. The application edge does not read
 * Neon: both source lists are owned by Rust services and their repositories.
 */
export async function getPortalPaletteClients(): Promise<PortalPaletteClient[]> {
  const first = await rustApiRead<ClientsPageResult>(
    '/v1/clients?sort=name&page=1&pageSize=100' as `/v1/${string}`,
  )

  const rows = [...first.value.rows]
  const pageSize = Math.max(1, first.value.pageSize)
  const pages = Math.ceil(first.value.total / pageSize)

  // The command palette is intentionally complete. Page through the bounded
  // Rust endpoint rather than reintroducing an unbounded SQL escape hatch.
  for (let page = 2; page <= pages; page += 1) {
    const next = await rustApiRead<ClientsPageResult>(
      (`/v1/clients?sort=name&page=${page}&pageSize=${pageSize}`) as `/v1/${string}`,
    )
    rows.push(...next.value.rows)
  }

  return rows.map((row) => ({ id: row.id, name: row.displayName }))
}

export async function getPortalPaletteDeals(): Promise<PortalPaletteDeal[]> {
  const result = await rustApiRead<DealPortfolioSnapshot>('/v1/deals')
  return result.value.deals.map((deal) => ({
    id: deal.id,
    name: deal.propertyName,
    client: deal.clientName,
  }))
}
