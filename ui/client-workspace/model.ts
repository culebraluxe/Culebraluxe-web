import type {
  Client,
  ClientRelationshipChannel,
  RelationshipActivity,
} from '@/lib/portal/types'
import type { PersonPropertyContextDto } from '@/services/property/types'

export type ClientWorkspaceListItem = {
  id: string
  displayName: string
  nameResolved: boolean
  role: string
  status: string
  primaryEmail: string | null
  primaryPhone: string | null
  relationshipActivity: RelationshipActivity
}

export type ClientWorkspaceListPage = {
  rows: ClientWorkspaceListItem[]
  total: number
  page: number
  pageSize: number
}

export type ClientWorkspaceChannelSlot =
  | 'phone'
  | 'imessage'
  | 'whatsapp'
  | 'gmail'
  | 'facetime'
  | 'calendar'

export type ClientWorkspaceChannelModel = {
  slot: ClientWorkspaceChannelSlot
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

export type ClientWorkspacePageModel = {
  query: string
  page: number
  pageSize: number
  pageCount: number
  total: number
  list: ClientWorkspaceListItem[]
  selectedClientId: string | null
  client: Client | null
  channels: ClientWorkspaceChannelModel[]
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

export const INITIAL_CLIENT_WORKSPACE_MODEL: ClientWorkspacePageModel = {
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

export type ClientWorkspaceIntentMap = {
  'clientWorkspace.load': { request: EmptyPayload; response: void }
  'clientWorkspace.queryChanged': { request: { query: string }; response: void }
  'clientWorkspace.previousPage': { request: EmptyPayload; response: void }
  'clientWorkspace.nextPage': { request: EmptyPayload; response: void }
  'clientWorkspace.selectClient': { request: { personId: string }; response: void }
  'clientWorkspace.loadClient': { request: { personId: string }; response: void }
  'clientWorkspace.loadChannels': { request: { personId: string }; response: void }
  'clientWorkspace.loadPropertyContext': { request: { personId: string }; response: void }
  'clientWorkspace.notesChanged': { request: { notes: string }; response: void }
  'clientWorkspace.saveNotes': { request: EmptyPayload; response: void }
}

export type ClientWorkspaceRawChannels = readonly ClientRelationshipChannel[]
