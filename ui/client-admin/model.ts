import type { PageIntentMap } from '../runtime'

export type ClientAdminRow = {
  id: string
  displayName: string
  role: string
  status: string
  location: string | null
  assignedAgent: string | null
  primaryEmail: string | null
  primaryPhone: string | null
  lastInteractionLabel: string | null
  openTaskCount: number
  activeDealCount: number
  interestCount: number
}

export type ClientAdminPageData = {
  rows: ClientAdminRow[]
  total: number
  page: number
  pageSize: number
}

export type ClientAdminPageModel = {
  search: string
  page: number
  pageSize: number
  pageCount: number
  rows: ClientAdminRow[]
  total: number
  loading: boolean
  error: string | null
}

export const INITIAL_CLIENT_ADMIN_MODEL: ClientAdminPageModel = {
  search: '',
  page: 1,
  pageSize: 50,
  pageCount: 1,
  rows: [],
  total: 0,
  loading: true,
  error: null,
}

type EmptyPayload = Record<string, never>

export type ClientAdminIntentMap = {
  'clientAdmin.load': {
    request: EmptyPayload
    response: void
  }
  'clientAdmin.searchChanged': {
    request: { search: string }
    response: void
  }
  'clientAdmin.previousPage': {
    request: EmptyPayload
    response: void
  }
  'clientAdmin.nextPage': {
    request: EmptyPayload
    response: void
  }
} satisfies PageIntentMap
