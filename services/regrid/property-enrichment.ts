import type {
  RegridAddressClient,
  RegridPropertyEnrichmentRepository,
  RegridPropertyEnrichmentResult,
  RegridPropertyTarget,
} from './types'

export function buildRegridPropertyQuery(target: RegridPropertyTarget): string {
  const parts = [
    target.addressLine1,
    target.city,
    target.stateOrProvince,
    target.postalCode,
  ]
    .map((value) => value?.trim())
    .filter((value): value is string => Boolean(value))

  if (parts.length === 0) {
    throw new Error(`Property ${target.id} has no address to enrich through Regrid`)
  }
  return parts.join(', ')
}

/**
 * One Property -> one Regrid address request. A unique match is persisted;
 * zero or multiple matches never mutate Property.
 */
export async function enrichPropertyFromRegrid(input: {
  propertyId: string
  client: RegridAddressClient
  repository: RegridPropertyEnrichmentRepository
  path?: string | null
  queryOverride?: string | null
}): Promise<RegridPropertyEnrichmentResult> {
  const target = await input.repository.getTarget(input.propertyId)
  if (!target) throw new Error(`Property '${input.propertyId}' was not found`)

  const query = input.queryOverride?.trim() || buildRegridPropertyQuery(target)
  const lookup = await input.client.lookupAddress({
    query,
    path: input.path,
    // Two results is enough to distinguish a unique answer from ambiguity,
    // while preserving the one-call-per-property contract.
    limit: 2,
  })

  if (lookup.status === 'not_found') {
    return { status: 'not_found', propertyId: target.id, query }
  }
  if (lookup.status === 'ambiguous') {
    return {
      status: 'ambiguous',
      propertyId: target.id,
      query,
      matches: lookup.matches,
    }
  }

  const receipt = await input.repository.persist({
    propertyId: target.id,
    lookupQuery: query,
    parcel: lookup.parcel,
  })

  return {
    status: 'enriched',
    propertyId: target.id,
    query,
    parcel: lookup.parcel,
    receipt,
  }
}
