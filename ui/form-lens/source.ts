'use client'

import type {
  ListingCanonicalFields,
  ListingCanonicalSnapshot,
} from '@/lib/forms/listing-field-binding'
import type { Client } from '@/lib/portal/types'
import type { PersonPropertyContextDto, PropertyAddressDto } from '@/services/property'
import type { FormLensListItem, FormLensListPage } from './model'

export type FormLensLoadListRequest = { query: string; page: number; pageSize: number }
export type FormLensLoadOptions = { signal?: AbortSignal }

export interface FormLensSource {
  loadList(request: FormLensLoadListRequest, options?: FormLensLoadOptions): Promise<FormLensListPage>
  loadPerson(personId: string, options?: FormLensLoadOptions): Promise<Client | null>
  loadPropertyContext(personId: string, options?: FormLensLoadOptions): Promise<PersonPropertyContextDto>
  loadListingBinding(personId: string, options?: FormLensLoadOptions): Promise<ListingCanonicalSnapshot>
  saveListingBinding(
    personId: string,
    fields: ListingCanonicalFields,
    physicalPropertyId?: string,
  ): Promise<ListingCanonicalSnapshot>
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
    const response = await fetch(`/api/portal/clients?${params.toString()}`, { signal: options.signal })
    if (!response.ok) throw new Error(`Person list HTTP ${response.status}`)
    return await response.json() as FormLensListPage
  }

  async loadPerson(personId: string, options: FormLensLoadOptions = {}): Promise<Client | null> {
    const response = await fetch(`/api/portal/clients/${personId}`, { signal: options.signal })
    if (!response.ok) throw new Error(`Person detail HTTP ${response.status}`)
    const json = await response.json() as ClientResponse
    return json.client ?? null
  }

  async loadPropertyContext(
    personId: string,
    options: FormLensLoadOptions = {},
  ): Promise<PersonPropertyContextDto> {
    const response = await fetch(`/api/portal/clients/${personId}/property-context`, { signal: options.signal })
    if (!response.ok) throw new Error(`Property context HTTP ${response.status}`)
    return await response.json() as PersonPropertyContextDto
  }

  async loadListingBinding(
    personId: string,
    options: FormLensLoadOptions = {},
  ): Promise<ListingCanonicalSnapshot> {
    const params = new URLSearchParams({ personId })
    const response = await fetch(`/api/portal/form-sidecar/listing?${params.toString()}`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(await responseError(response))
    return await response.json() as ListingCanonicalSnapshot
  }

  async saveListingBinding(
    personId: string,
    fields: ListingCanonicalFields,
    physicalPropertyId?: string,
  ): Promise<ListingCanonicalSnapshot> {
    const response = await fetch('/api/portal/form-sidecar/listing', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ personId, fields, physicalPropertyId: physicalPropertyId ?? null }),
    })
    if (!response.ok) throw new Error(await responseError(response))
    return await response.json() as ListingCanonicalSnapshot
  }
}

function formatAddress(address: PropertyAddressDto): string {
  return [
    address.addressLine1,
    address.neighborhood,
    address.city,
    [address.stateOrProvince, address.postalCode].filter(Boolean).join(' ') || null,
    address.country,
  ].filter((value): value is string => Boolean(value?.trim())).join(', ')
}

export class InMemoryFormLensSource implements FormLensSource {
  private readonly people = new Map<string, Client>()
  private readonly propertyByPersonId = new Map<string, PersonPropertyContextDto>()
  private readonly bindingByPersonId = new Map<string, ListingCanonicalSnapshot>()

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
      .filter((person) => query === '' || [person.displayName, person.email, person.phone]
        .some((value) => value?.toLowerCase().includes(query)))
      .sort((left, right) => left.displayName.localeCompare(right.displayName))
    const pageSize = Math.max(1, request.pageSize)
    const pageCount = Math.max(1, Math.ceil(all.length / pageSize))
    const page = Math.min(Math.max(1, request.page), pageCount)
    const start = (page - 1) * pageSize
    const rows: FormLensListItem[] = all.slice(start, start + pageSize).map((person) => ({
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
    return this.propertyByPersonId.get(personId) ?? { personId, properties: [], observedAddresses: [] }
  }

  async loadListingBinding(personId: string): Promise<ListingCanonicalSnapshot> {
    const saved = this.bindingByPersonId.get(personId)
    if (saved) return saved
    const person = this.people.get(personId)
    if (!person) throw new Error(`Person not found: ${personId}`)
    const context = await this.loadPropertyContext(personId)
    const legal = context.properties.find((row) => row.relation === 'legal_address') ?? null
    const physical = context.properties.find((row) => row.relation === 'physical_property') ?? null
    const fields: ListingCanonicalFields = {
      sellerName: person.displayName,
      sellerResidenceAddress: legal ? formatAddress(legal.property.address) : '',
      property: physical?.property.localName ?? '',
      propertyLocation: physical ? formatAddress(physical.property.address) : '',
      legalOwnerName: physical?.property.legalOwnerName ?? '',
      catastroNumber: physical?.property.catastroNumber ?? '',
    }
    return {
      personId,
      personDisplayName: person.displayName,
      formInstanceId: null,
      formUpdatedAt: null,
      legalAddressPropertyId: legal?.property.id ?? null,
      physicalPropertyId: physical?.property.id ?? null,
      fields,
      origins: {
        sellerName: fields.sellerName ? 'person' : 'empty',
        sellerResidenceAddress: fields.sellerResidenceAddress ? 'property' : 'empty',
        property: fields.property ? 'property' : 'empty',
        propertyLocation: fields.propertyLocation ? 'property' : 'empty',
        legalOwnerName: fields.legalOwnerName ? 'property' : 'empty',
        catastroNumber: fields.catastroNumber ? 'property' : 'empty',
      },
    }
  }

  async saveListingBinding(
    personId: string,
    fields: ListingCanonicalFields,
    physicalPropertyId?: string,
  ): Promise<ListingCanonicalSnapshot> {
    const current = await this.loadListingBinding(personId)
    const snapshot: ListingCanonicalSnapshot = {
      ...current,
      personDisplayName: fields.sellerName || current.personDisplayName,
      physicalPropertyId: physicalPropertyId ?? current.physicalPropertyId,
      fields: { ...fields },
      origins: {
        sellerName: fields.sellerName ? 'person' : 'empty',
        sellerResidenceAddress: fields.sellerResidenceAddress ? 'property' : 'empty',
        property: fields.property ? 'property' : 'empty',
        propertyLocation: fields.propertyLocation ? 'property' : 'empty',
        legalOwnerName: fields.legalOwnerName ? 'property' : 'empty',
        catastroNumber: fields.catastroNumber ? 'property' : 'empty',
      },
    }
    this.bindingByPersonId.set(personId, snapshot)
    return snapshot
  }
}
