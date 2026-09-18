import type { ListingSource, TransportAttempt } from './types'
import { stellarMapping } from './stellar-fields'

const CULEBRA_LAT = 18.303
const CULEBRA_LNG = -65.304

function resolveLat(source: ListingSource): number {
  return typeof source.latitude === 'number' ? source.latitude : CULEBRA_LAT
}

function resolveLng(source: ListingSource): number {
  return typeof source.longitude === 'number' ? source.longitude : CULEBRA_LNG
}

export function buildResoPropertyPayload(source: ListingSource) {
  const payload: Record<string, unknown> = {
    ListingKey: source.id,
    ListingId: source.slug ?? source.id,
    StandardStatus: source.isPublished ? 'Active' : 'ComingSoon',
    ListingContractDate: source.stellar?.listingContractDate ?? null,
    ListPrice: source.listPrice,
    ListPriceCurrency: 'USD',
    PropertyType: mapResoPropertyType(source.propertyType),
    PropertySubType: source.propertyType,
    LivingArea: source.squareFeet,
    LivingAreaUnits: 'SquareFeet',
    BedroomsTotal: source.bedrooms,
    BathroomsTotalInteger: source.bathrooms,
    BathroomsFull: source.bathroomsFull ?? null,
    BathroomsHalf: source.bathroomsHalf ?? null,
    UnparsedAddress: source.streetAddress ?? source.location ?? source.name,
    City: source.city ?? 'Culebra',
    Township: source.neighborhood,
    StateOrProvince: source.stateOrProvince ?? 'PR',
    Country: 'PR',
    PostalCode: source.postalCode ?? null,
    Latitude: resolveLat(source),
    Longitude: resolveLng(source),
    PublicRemarks: source.publicRemarks ?? source.shortDescription,
    PrivateRemarks: null,
    ListAgentFullName: source.listingAgentName,
    ListAgentPreferredPhone: source.listingAgentPhone,
    ListAgentEmail: source.listingAgentEmail,
    ListOfficeName: 'CulebraLuxe',
    ModificationTimestamp: new Date().toISOString(),
    OriginatingSystemName: 'MFR',
    SourceSystemName: 'CulebraLuxe',
    PhotosCount: source.photos.length || source.imageCount,
    ListingURL: source.publicUrl,
  }
  if (source.yearBuilt && source.yearBuilt > 0) payload.YearBuilt = source.yearBuilt
  Object.assign(payload, {
    StreetNumber: source.streetNumber ?? null,
    StreetName: source.streetName ?? null,
    UnitNumber: source.unitNumber ?? null,
    LotSize: source.lotSize ?? null,
    LotSizeUnits: source.lotSizeUnits ?? null,
    ...(source.stellar ? {
      ExpirationDate: source.stellar.expirationDate,
      ListingType: source.stellar.listingType,
      ListAgentMlsId: source.stellar.agentMlsId,
      ParcelNumber: source.stellar.taxId,
      TaxYear: source.stellar.taxYear,
      TaxAnnualAmount: source.stellar.annualTax,
      LegalDescription: source.stellar.legalDescription,
      Zoning: source.stellar.zoning,
      BuildingAreaTotal: source.stellar.totalAreaSqft,
      LivingAreaSource: source.stellar.heatedAreaSource,
      Ownership: source.stellar.ownershipType,
      AssociationDetails: source.stellar.hoaDetails,
      ShowingInstructions: source.stellar.showingInstructions,
      OccupantType: source.stellar.occupantType,
    } : {}),
  })
  return payload
}

function mapResoPropertyType(value: string | null): string {
  const raw = (value ?? '').toLowerCase()
  if (raw.includes('land') || raw.includes('lot') || raw.includes('solar')) return 'Land'
  if (raw.includes('condo') || raw.includes('apart')) return 'Residential'
  if (raw.includes('commercial')) return 'Commercial Sale'
  return 'Residential'
}

export function stellarTransportPlan(source: ListingSource): TransportAttempt {
  const mapping = stellarMapping(source)
  return {
    kind: 'stellar.matrix_checklist',
    dryRun: true,
    liveEnabled: false,
    method: 'MANUAL',
    endpoint: 'https://www.stellarmls.com/prar-en',
    payload: {
      resoProperty: buildResoPropertyPayload(source),
      mapping,
      matrixInput: {
        system: 'Stellar Matrix',
        office: 'CulebraLuxe',
        municipality: source.city ?? 'Culebra',
        listingType: source.stellar?.listingType ?? null,
        photos: source.photos.length
          ? `${source.photos.length} images ready to upload (Property Media). Do not hotlink culebraluxe.com.`
          : 'No photos attached yet — add media before entering Matrix.',
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
        'Submitting a listing to SkySlope Forms or Matrix until an approved intake is verified.',
        'Treating this preparation payload as an accepted MLS listing.',
      ],
    },
    missingEnv: [],
    response: {
      status: 'dry_run',
      reason: 'SkySlope Forms may create a Matrix draft; external write access and field vocabulary need confirmation with Stellar.',
    },
  }
}
