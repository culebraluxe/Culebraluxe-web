import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

export type FirmDto = {
  id: string
  name: string
  legalName: string | null
  kind: string | null
  status: string
}

export type GetFirmRequest = { firmId: string }
export type FindFirmByNameRequest = { name: string }
export type UpsertFirmRequest = {
  firmId?: string
  name: string
  legalName?: string | null
  kind?: string | null
  status?: string
}

export const FIRM_OPERATIONS = {
  GET: 'firm.get',
  FIND_BY_NAME: 'firm.findByName',
  UPSERT: 'firm.upsert',
} as const

export type FirmOperationMap = {
  'firm.get': { request: GetFirmRequest; response: FirmDto | null }
  'firm.findByName': { request: FindFirmByNameRequest; response: FirmDto | null }
  'firm.upsert': { request: UpsertFirmRequest; response: FirmDto }
}

export type FirmOperationName = ServiceOperationName<FirmOperationMap>
export type FirmEnvelope<K extends FirmOperationName = FirmOperationName> =
  ServiceEnvelopeFor<FirmOperationMap, K>
