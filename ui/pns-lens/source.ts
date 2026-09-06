'use client'

import type { PnsCanonicalFields, PnsCanonicalSnapshot } from '@/lib/forms/pns-canonical-types'
import type { Client } from '@/lib/portal/types'
import type { PersonPropertyContextDto } from '@/services/property'
import type { PnsLensListPage } from './model'

export type PnsLensLoadListRequest = { query: string; page: number; pageSize: number }
export type PnsLensLoadOptions = { signal?: AbortSignal }

export interface PnsLensSource {
  loadList(request: PnsLensLoadListRequest, options?: PnsLensLoadOptions): Promise<PnsLensListPage>
  loadPerson(personId: string, options?: PnsLensLoadOptions): Promise<Client | null>
  loadPropertyContext(personId: string, options?: PnsLensLoadOptions): Promise<PersonPropertyContextDto>
  loadBinding(personId: string, contractId?: string | null, options?: PnsLensLoadOptions): Promise<PnsCanonicalSnapshot>
  saveBinding(input: {
    personId: string
    contractId?: string | null
    physicalPropertyId?: string | null
    fields: PnsCanonicalFields
  }): Promise<PnsCanonicalSnapshot>
}

type ClientResponse = { client?: Client | null }

async function responseError(response: Response): Promise<string> {
  try {
    const body = await response.json() as { error?: string }
    return body.error || `HTTP ${response.status}`
  } catch {
    return `HTTP ${response.status}`
  }
}

export class HttpPnsLensSource implements PnsLensSource {
  async loadList(request: PnsLensLoadListRequest, options: PnsLensLoadOptions = {}): Promise<PnsLensListPage> {
    const params = new URLSearchParams({
      view: 'directory',
      search: request.query,
      page: String(request.page),
      pageSize: String(request.pageSize),
      sort: 'name',
    })
    const response = await fetch(`/api/portal/clients?${params.toString()}`, { signal: options.signal })
    if (!response.ok) throw new Error(`Person list HTTP ${response.status}`)
    return await response.json() as PnsLensListPage
  }

  async loadPerson(personId: string, options: PnsLensLoadOptions = {}): Promise<Client | null> {
    const response = await fetch(`/api/portal/clients/${personId}`, { signal: options.signal })
    if (!response.ok) throw new Error(`Person detail HTTP ${response.status}`)
    const json = await response.json() as ClientResponse
    return json.client ?? null
  }

  async loadPropertyContext(personId: string, options: PnsLensLoadOptions = {}): Promise<PersonPropertyContextDto> {
    const response = await fetch(`/api/portal/clients/${personId}/property-context`, { signal: options.signal })
    if (!response.ok) throw new Error(`Property context HTTP ${response.status}`)
    return await response.json() as PersonPropertyContextDto
  }

  async loadBinding(
    personId: string,
    contractId?: string | null,
    options: PnsLensLoadOptions = {},
  ): Promise<PnsCanonicalSnapshot> {
    const params = new URLSearchParams({ personId })
    if (contractId) params.set('contractId', contractId)
    const response = await fetch(`/api/portal/form-sidecar/pns?${params.toString()}`, { signal: options.signal })
    if (!response.ok) throw new Error(await responseError(response))
    return await response.json() as PnsCanonicalSnapshot
  }

  async saveBinding(input: {
    personId: string
    contractId?: string | null
    physicalPropertyId?: string | null
    fields: PnsCanonicalFields
  }): Promise<PnsCanonicalSnapshot> {
    const response = await fetch('/api/portal/form-sidecar/pns', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    })
    if (!response.ok) throw new Error(await responseError(response))
    return await response.json() as PnsCanonicalSnapshot
  }
}
