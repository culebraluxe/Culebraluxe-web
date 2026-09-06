import type { ContractRoleDto } from '@/services/contract'

/**
 * Working P&S sentinel for facts that are not known yet.
 *
 * This is deliberately draft/Form vocabulary, not canonical Person/Firm/
 * Property truth. The Contract draft may retain TBD so the user's work is
 * explicit and revisitable; canonical promotion treats it as absent.
 */
export const PNS_TBD = 'TBD' as const

export function isPnsTbd(value: unknown): boolean {
  if (typeof value !== 'string') return false
  const normalized = value.trim().toUpperCase()
  return normalized === PNS_TBD || normalized === 'NAN' || normalized === 'NULL' || normalized === 'UNDEFINED'
}

/** Normalize empty/obvious null-like working values to one visible draft truth. */
export function toPnsWorkingValue(value: unknown): string {
  if (typeof value !== 'string') return PNS_TBD
  const next = value.trim()
  return !next || isPnsTbd(next) ? PNS_TBD : next
}

/**
 * Value safe to promote into a canonical owning service.
 * TBD/null-like working values intentionally collapse to empty here.
 */
export function toPnsCanonicalValue(value: unknown): string {
  if (typeof value !== 'string') return ''
  const next = value.trim()
  return !next || isPnsTbd(next) ? '' : next
}

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
