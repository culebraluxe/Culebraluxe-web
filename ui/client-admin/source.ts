import type { ClientAdminPageData, ClientAdminRow } from './model'

export type ClientAdminLoadRequest = {
  search: string
  page: number
  pageSize: number
}

export type ClientAdminLoadOptions = {
  signal?: AbortSignal
}

/**
 * Presentation data seam. The controller does not know whether rows come from
 * the real application, a service adapter, replayed production data, or a fixture.
 */
export interface ClientAdminSource {
  load(
    request: ClientAdminLoadRequest,
    options?: ClientAdminLoadOptions,
  ): Promise<ClientAdminPageData>
}

/** Current real-client adapter. It preserves the existing portal endpoint while
 * the service layer is being introduced underneath it. */
export class HttpClientAdminSource implements ClientAdminSource {
  async load(
    request: ClientAdminLoadRequest,
    options: ClientAdminLoadOptions = {},
  ): Promise<ClientAdminPageData> {
    const params = new URLSearchParams({
      view: 'admin',
      search: request.search,
      page: String(request.page),
      pageSize: String(request.pageSize),
    })
    const response = await fetch(`/api/portal/clients?${params.toString()}`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
    return (await response.json()) as ClientAdminPageData
  }
}

/**
 * DB-free source for isolated UI work, replay, Storybook-like scenarios, and
 * parallel Smith development. Feed the same controller/view arbitrary rows.
 */
export class InMemoryClientAdminSource implements ClientAdminSource {
  constructor(private readonly rows: readonly ClientAdminRow[]) {}

  async load(request: ClientAdminLoadRequest): Promise<ClientAdminPageData> {
    const query = request.search.trim().toLowerCase()
    const filtered = query === ''
      ? [...this.rows]
      : this.rows.filter((row) =>
          [row.displayName, row.primaryEmail, row.primaryPhone, row.location]
            .some((value) => value?.toLowerCase().includes(query)),
        )

    const pageSize = Math.max(1, request.pageSize)
    const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize))
    const page = Math.min(Math.max(1, request.page), pageCount)
    const start = (page - 1) * pageSize

    return {
      rows: filtered.slice(start, start + pageSize),
      total: filtered.length,
      page,
      pageSize,
    }
  }
}
