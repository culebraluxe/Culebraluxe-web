import type {
  Client,
  ClientRelationshipChannel,
  RelationshipActivity,
} from '@/lib/portal/types'
import type { PersonPropertyContextDto } from '@/services/property/types'
import type { PageIntentMap } from '../runtime'

export type ClientLensListItem = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  primaryEmail: string | null
  primaryPhone: string | null
  relationshipActivity: RelationshipActivity
}

export type ClientLensListPage = {
  rows: ClientLensListItem[]
  total: number
  page: number
  pageSize: number
}

export type ClientLensChannelSlot =
  | 'phone'
  | 'imessage'
  | 'whatsapp'
  | 'gmail'
  | 'facetime'
  | 'calendar'

export type ClientLensChannelModel = {
  slot: ClientLensChannelSlot
  label: string
  connected: boolean
  source: string | null
  channel: string | null
  firstObservedAt: string | null
  lastContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  inboundCount: number
  outboundCount: number
  totalCount: number
  twoWay: boolean
  lastContext: string | null
  lastContextAt: string | null
  lastContextDirection: 'inbound' | 'outbound' | null
}

export type ClientLensPageModel = {
  query: string
  page: number
  pageSize: number
  pageCount: number
  total: number
  list: ClientLensListItem[]
  selectedClientId: string | null
  client: Client | null
  channels: ClientLensChannelModel[]
  propertyContext: PersonPropertyContextDto | null
  notesDraft: string
  notesSaved: string
  listLoading: boolean
  clientLoading: boolean
  channelsLoading: boolean
  propertyLoading: boolean
  notesSaving: boolean
  listError: string | null
  clientError: string | null
  channelsError: string | null
  propertyError: string | null
  notesStatus: string | null
}

export const INITIAL_CLIENT_LENS_MODEL: ClientLensPageModel = {
  query: '',
  page: 1,
  pageSize: 50,
  pageCount: 1,
  total: 0,
  list: [],
  selectedClientId: null,
  client: null,
  channels: [],
  propertyContext: null,
  notesDraft: '',
  notesSaved: '',
  listLoading: true,
  clientLoading: false,
  channelsLoading: false,
  propertyLoading: false,
  notesSaving: false,
  listError: null,
  clientError: null,
  channelsError: null,
  propertyError: null,
  notesStatus: null,
}

type EmptyPayload = Record<string, never>

export type ClientLensIntentMap = {
  'clientLens.load': { request: EmptyPayload; response: void }
  'clientLens.queryChanged': { request: { query: string }; response: void }
  'clientLens.previousPage': { request: EmptyPayload; response: void }
  'clientLens.nextPage': { request: EmptyPayload; response: void }
  'clientLens.selectClient': { request: { personId: string }; response: void }
  'clientLens.loadClient': { request: { personId: string }; response: void }
  'clientLens.loadChannels': { request: { personId: string }; response: void }
  'clientLens.loadPropertyContext': { request: { personId: string }; response: void }
  'clientLens.notesChanged': { request: { notes: string }; response: void }
  'clientLens.saveNotes': { request: EmptyPayload; response: void }
} satisfies PageIntentMap

export type ClientLensRawChannels = readonly ClientRelationshipChannel[]
