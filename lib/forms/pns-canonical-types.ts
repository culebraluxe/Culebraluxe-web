import type { ContractRoleDto } from '@/services/contract'

export type PnsFieldOrigin =
  | 'contract'
  | 'role'
  | 'person'
  | 'firm'
  | 'property'
  | 'pns_form'
  | 'template_default'
  | 'manual'
  | 'empty'

export type PnsCanonicalFields = Readonly<Record<string, string>>

export type PnsCanonicalSnapshot = {
  personId: string
  personDisplayName: string
  contractId: string | null
  contractStatus: string | null
  formInstanceId: string | null
  formUpdatedAt: string | null
  physicalPropertyId: string | null
  fields: PnsCanonicalFields
  origins: Readonly<Record<string, PnsFieldOrigin>>
  roles: readonly ContractRoleDto[]
}

export type SavePnsCanonicalRequest = {
  personId: string
  contractId?: string | null
  physicalPropertyId?: string | null
  fields: PnsCanonicalFields
}
