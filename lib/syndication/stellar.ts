import type { ListingSource, TransportAttempt } from './types'

export function buildResoPropertyPayload(source: ListingSource) {
  return {
    ListingKey: source.id,
    ListingId: source.slug ?? source.id,
    StandardStatus: source.isPublished ? 'Active' : 'ComingSoon',
    ListingContractDate: null,
    ListPrice: source.listPrice,
    ListPriceCurrency: 'USD',
    PropertyType: mapResoPropertyType(source.propertyType),
    PropertySubType: source.propertyType,
    LivingArea: source.squareFeet,
    LivingAreaUnits: 'SquareFeet',
    BedroomsTotal: source.bedrooms,
    BathroomsTotalInteger: source.bathrooms,
    UnparsedAddress: source.location ?? source.name,
    City: source.city ?? 'Culebra',
    Township: source.neighborhood,
    StateOrProvince: 'PR',
    Country: 'PR',
    PostalCode: null,
    Latitude: 18.303,
    Longitude: -65.304,
    PublicRemarks: source.publicRemarks ?? source.shortDescription,
    PrivateRemarks: null,
    ListAgentFullName: source.listingAgentName,
    ListAgentPreferredPhone: source.listingAgentPhone,
    ListAgentEmail: source.listingAgentEmail,
    ListOfficeName: 'CulebraLuxe',
    ModificationTimestamp: new Date().toISOString(),
    OriginatingSystemName: 'MFR',
    SourceSystemName: 'CulebraLuxe',
    PhotosCount: source.imageCount,
    ListingURL: source.publicUrl,
  }
}

function mapResoPropertyType(value: string | null): string {
  const raw = (value ?? '').toLowerCase()
  if (raw.includes('land') || raw.includes('lot') || raw.includes('solar')) return 'Land'
  if (raw.includes('condo') || raw.includes('apart')) return 'Residential'
  if (raw.includes('commercial')) return 'Commercial Sale'
  return 'Residential'
}

export function stellarTransportPlan(source: ListingSource): TransportAttempt {
  return {
    kind: 'stellar.matrix_checklist',
    dryRun: true,
    liveEnabled: false,
    method: 'MANUAL',
    endpoint: 'https://www.stellarmls.com/prar-en',
    payload: {
      resoProperty: buildResoPropertyPayload(source),
      matrixInput: {
        system: 'Stellar Matrix',
        office: 'CulebraLuxe',
        municipality: source.city ?? 'Culebra',
        listingType: 'For Sale',
        photos: 'Upload the same set used on culebraluxe.com. Do not hotlink.',
      },
      distribution: {
        realtorCom: true,
        homesCom: true,
        homesnap: true,
        listHub: true,
        zillowRentals: false,
        note: 'Broker opt-in in the Stellar Portal overrides listing-level Matrix checkboxes.',
      },
      cannotDo: [
        'POST a new listing into Matrix from this app.',
        'Write RESO Web API — that feed is pull/IDX only.',
      ],
    },
    missingEnv: [],
    response: {
      status: 'dry_run',
      reason: 'Stellar does not expose a listing-write API to brokerages.',
    },
  }
}
