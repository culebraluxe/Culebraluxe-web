import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

export type ShowingReportOutcome =
  | 'Interested'
  | 'Second showing'
  | 'Offer expected'
  | 'Not a fit'

export type ShowingDto = {
  id: string
  personId: string
  propertyId: string
  status: string
  showingDate: string | null
  duration: string | null
  outcome: ShowingReportOutcome | null
  interestScore: number | null
  feedback: string | null
  followUp: string | null
  completedAt: string | null
}

export type GetShowingRequest = { showingId: string }

export type SaveShowingReportRequest = {
  showingId: string
  personId: string
  propertyId: string
  showingDate: string | null
  duration: string | null
  outcome: ShowingReportOutcome | null
  interestScore: number | null
  feedback: string | null
  followUp: string | null
}

export const SHOWING_OPERATIONS = {
  GET: 'showing.get',
  SAVE_REPORT: 'showing.saveReport',
} as const

export type ShowingOperationMap = {
  'showing.get': { request: GetShowingRequest; response: ShowingDto | null }
  'showing.saveReport': { request: SaveShowingReportRequest; response: ShowingDto }
}

export type ShowingOperationName = ServiceOperationName<ShowingOperationMap>
export type ShowingEnvelope<K extends ShowingOperationName = ShowingOperationName> =
  ServiceEnvelopeFor<ShowingOperationMap, K>
