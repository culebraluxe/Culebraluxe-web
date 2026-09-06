'use client'

import { updatePersonNotesAction } from '@/app/portal/actions'
import type {
  Client,
  ClientRelationshipChannel,
  RelationshipActivity,
} from '@/lib/portal/types'
import type { PersonPropertyContextDto } from '@/services/property/types'
import type {
  ClientLensListItem,
  ClientLensListPage,
} from './model'

export type ClientLensLoadListRequest = {
  query: string
  page: number
  pageSize: number
}

export type ClientLensLoadOptions = {
  signal?: AbortSignal
}

export interface ClientLensSource {
  loadList(
    request: ClientLensLoadListRequest,
    options?: ClientLensLoadOptions,
  ): Promise<ClientLensListPage>
  loadClient(personId: string, options?: ClientLensLoadOptions): Promise<Client | null>
  loadChannels(
    personId: string,
    options?: ClientLensLoadOptions,
  ): Promise<ClientRelationshipChannel[]>
  loadPropertyContext(
    personId: string,
    options?: ClientLensLoadOptions,
  ): Promise<PersonPropertyContextDto>
  saveNotes(personId: string, notes: string): Promise<void>
}

type DirectoryResponse = ClientLensListPage
type ClientResponse = { client?: Client | null }
type ChannelsResponse = { channels?: ClientRelationshipChannel[] }

const EMPTY_RELATIONSHIP_ACTIVITY: RelationshipActivity = {
  hasEvidence: false,
  sources: [],
  firstObservedAt: null,
  inboundCount: 0,
  outboundCount: 0,
  observedCommunicationCount: 0,
  twoWay: false,
  lastObservedAt: null,
  lastMeaningfulContactAt: null,
  lastInboundAt: null,
  lastOutboundAt: null,
  coverageLimited: false,
  channels: [],
}

/** Real adapter over the current production-safe Portal seams. */
export class HttpClientLensSource implements ClientLensSource {
  async loadList(
    request: ClientLensLoadListRequest,
    options: ClientLensLoadOptions = {},
  ): Promise<ClientLensListPage> {
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
    if (!response.ok) throw new Error(`Client list HTTP ${response.status}`)
    return (await response.json()) as DirectoryResponse
  }

  async loadClient(
    personId: string,
    options: ClientLensLoadOptions = {},
  ): Promise<Client | null> {
    const response = await fetch(`/api/portal/clients/${personId}`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Client detail HTTP ${response.status}`)
    const json = (await response.json()) as ClientResponse
    return json.client ?? null
  }

  async loadChannels(
    personId: string,
    options: ClientLensLoadOptions = {},
  ): Promise<ClientRelationshipChannel[]> {
    const response = await fetch(`/api/portal/clients/${personId}/relationship-channels`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Relationship channels HTTP ${response.status}`)
    const json = (await response.json()) as ChannelsResponse
    return json.channels ?? []
  }

  async loadPropertyContext(
    personId: string,
    options: ClientLensLoadOptions = {},
  ): Promise<PersonPropertyContextDto> {
    const response = await fetch(`/api/portal/clients/${personId}/property-context`, {
      signal: options.signal,
    })
    if (!response.ok) throw new Error(`Property context HTTP ${response.status}`)
    return (await response.json()) as PersonPropertyContextDto
  }

  async saveNotes(personId: string, notes: string): Promise<void> {
    const result = await updatePersonNotesAction(personId, notes)
    if (!result.ok) throw new Error(result.message ?? 'Could not save notes.')
  }
}

/** DB-free adapter for isolated view/controller development and replay. */
export class InMemoryClientLensSource implements ClientLensSource {
  private readonly byId = new Map<string, Client>()
  private readonly channelsById = new Map<string, ClientRelationshipChannel[]>()
  private readonly propertyByPersonId = new Map<string, PersonPropertyContextDto>()

  constructor(
    clients: readonly Client[],
    channels: Readonly<Record<string, readonly ClientRelationshipChannel[]>> = {},
    propertyContext: Readonly<Record<string, PersonPropertyContextDto>> = {},
  ) {
    for (const client of clients) this.byId.set(client.id, { ...client })
    for (const [personId, rows] of Object.entries(channels)) {
      this.channelsById.set(personId, [...rows])
    }
    for (const [personId, context] of Object.entries(propertyContext)) {
      this.propertyByPersonId.set(personId, {
        ...context,
        properties: [...context.properties],
        observedAddresses: [...context.observedAddresses],
      })
    }
  }

  async loadList(request: ClientLensLoadListRequest): Promise<ClientLensListPage> {
    const query = request.query.trim().toLowerCase()
    const all = [...this.byId.values()]
      .filter((client) =>
        query === '' ||
        [client.displayName, client.email, client.phone]
          .some((value) => value?.toLowerCase().includes(query)),
      )
      .sort((left, right) => left.displayName.localeCompare(right.displayName))

    const pageSize = Math.max(1, request.pageSize)
    const pageCount = Math.max(1, Math.ceil(all.length / pageSize))
    const page = Math.min(Math.max(1, request.page), pageCount)
    const start = (page - 1) * pageSize

    const rows: ClientLensListItem[] = all
      .slice(start, start + pageSize)
      .map((client) => ({
        id: client.id,
        displayName: client.displayName,
        nameResolved: true,
        role: client.role,
        status: client.status,
        primaryEmail: client.email ?? null,
        primaryPhone: client.phone ?? null,
        relationshipActivity: client.relationshipActivity ?? EMPTY_RELATIONSHIP_ACTIVITY,
      }))

    return { rows, total: all.length, page, pageSize }
  }

  async loadClient(personId: string): Promise<Client | null> {
    return this.byId.get(personId) ?? null
  }

  async loadChannels(personId: string): Promise<ClientRelationshipChannel[]> {
    return [...(this.channelsById.get(personId) ?? [])]
  }

  async loadPropertyContext(personId: string): Promise<PersonPropertyContextDto> {
    const context = this.propertyByPersonId.get(personId)
    return context ?? { personId, properties: [], observedAddresses: [] }
  }

  async saveNotes(personId: string, notes: string): Promise<void> {
    const client = this.byId.get(personId)
    if (!client) throw new Error(`Client not found: ${personId}`)
    this.byId.set(personId, { ...client, notes })
  }
}
