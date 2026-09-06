'use client'

import type { Client } from '@/lib/portal/types'
import type { PersonPropertyContextDto } from '@/services/property'
import type { FormLensListItem, FormLensListPage } from './model'

export type FormLensLoadListRequest = {
  query: string
  page: number
  pageSize: number
}

export type FormLensLoadOptions = {
  signal?: AbortSignal
}

export interface FormLensSource {
  loadList(
    request: FormLensLoadListRequest,
    options?: FormLensLoadOptions,
  ): Promise<FormLensListPage>
  loadPerson(personId: string, options?: FormLensLoadOptions): Promise<Client | null>
  loadPropertyContext(
    personId: string,
    options?: FormLensLoadOptions,
  ): Promise<PersonPropertyContextDto>
}

type ClientResponse = { client?: Client | null }

/** Real browser adapter over bounded Portal transport seams. */
export class HttpFormLensSource implements FormLensSource {
  async loadList(
    request: FormLensLoadListRequest,
    options: FormLensLoadOptions = {},
  ): Promise<FormLensListPage> {
    const params = new URLSearchParams({
      view: 'directory',
      search: request.query,
      page: String(request.page),
      pageSize: String(request.pageSize),
      sort: 'name',
    })
    const response = await fetch(`/api/portal/clients?${params.toString()}`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Person list HTTP ${response.status}`)
    return (await response.json()) as FormLensListPage
  }

  async loadPerson(
    personId: string,
    options: FormLensLoadOptions = {},
  ): Promise<Client | null> {
    const response = await fetch(`/api/portal/clients/${personId}`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Person detail HTTP ${response.status}`)
    const json = (await response.json()) as ClientResponse
    return json.client ?? null
  }

  async loadPropertyContext(
    personId: string,
    options: FormLensLoadOptions = {},
  ): Promise<PersonPropertyContextDto> {
    const response = await fetch(`/api/portal/clients/${personId}/property-context`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Property context HTTP ${response.status}`)
    return (await response.json()) as PersonPropertyContextDto
  }
}

/** DB-free adapter for isolated Listing Agreement composition work. */
export class InMemoryFormLensSource implements FormLensSource {
  private readonly people = new Map<string, Client>()
  private readonly propertyByPersonId = new Map<string, PersonPropertyContextDto>()

  constructor(
    people: readonly Client[],
    propertyContext: Readonly<Record<string, PersonPropertyContextDto>> = {},
  ) {
    for (const person of people) this.people.set(person.id, { ...person })
    for (const [personId, context] of Object.entries(propertyContext)) {
      this.propertyByPersonId.set(personId, {
        ...context,
        properties: [...context.properties],
        observedAddresses: [...context.observedAddresses],
      })
    }
  }

  async loadList(request: FormLensLoadListRequest): Promise<FormLensListPage> {
    const query = request.query.trim().toLowerCase()
    const all = [...this.people.values()]
      .filter((person) =>
        query === '' ||
        [person.displayName, person.email, person.phone]
          .some((value) => value?.toLowerCase().includes(query)),
      )
      .sort((left, right) => left.displayName.localeCompare(right.displayName))

    const pageSize = Math.max(1, request.pageSize)
    const pageCount = Math.max(1, Math.ceil(all.length / pageSize))
    const page = Math.min(Math.max(1, request.page), pageCount)
    const start = (page - 1) * pageSize
    const rows: FormLensListItem[] = all
      .slice(start, start + pageSize)
      .map((person) => ({
        id: person.id,
        displayName: person.displayName,
        nameResolved: true,
        role: person.role,
        status: person.status,
        primaryEmail: person.email ?? null,
        primaryPhone: person.phone ?? null,
      }))

    return { rows, total: all.length, page, pageSize }
  }

  async loadPerson(personId: string): Promise<Client | null> {
    return this.people.get(personId) ?? null
  }

  async loadPropertyContext(personId: string): Promise<PersonPropertyContextDto> {
    return this.propertyByPersonId.get(personId) ?? {
      personId,
      properties: [],
      observedAddresses: [],
    }
  }
}
