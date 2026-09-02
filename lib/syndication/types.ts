import type { PlacementStatus, PublishMode, SyndicationChannel } from './channels'

export type PhotoManifestItem = {
  mediaId: string
  url: string
  role: 'hero' | 'gallery'
  sortOrder: number
  width?: number
  height?: number
  contentType?: string
}

export type ListingSource = {
  id: string
  name: string
  slug: string | null
  status: string
  isPublished: boolean
  listPrice: number | null
  location: string | null
  city: string | null
  neighborhood: string | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  propertyType: string | null
  shortDescription: string | null
  publicRemarks: string | null
  listingAgentName: string | null
  listingAgentPhone: string | null
  listingAgentEmail: string | null
  publicUrl: string | null
  heroMediaId: string | null
  imageCount: number
  latitude?: number | null
  longitude?: number | null
  yearBuilt?: number | null
  postalCode?: string | null
  streetAddress?: string | null
  photos: PhotoManifestItem[]
}

export type TransportAttempt = {
  kind: string
  dryRun: boolean
  liveEnabled: boolean
  method: string
  endpoint: string
  payload: Record<string, unknown>
  missingEnv: string[]
  response?: Record<string, unknown>
}

export type ListingPack = {
  channel: SyndicationChannel
  titleEs: string
  titleEn: string
  bodyEs: string
  bodyEn: string
  priceLabel: string
  locationLine: string
  factsLine: string
  publicUrl: string | null
  contactLine: string
  photoHint: string
  instructions: string
  pasteTargetUrl: string | null
  transport?: TransportAttempt | null
}

export type AdapterResult = {
  ok: boolean
  mode: PublishMode
  status: PlacementStatus
  pack: ListingPack
  message: string
  ttlDays: number | null
  externalId?: string | null
  externalUrl?: string | null
  transport?: TransportAttempt | null
}

export type PlacementRow = {
  id: string
  propertyId: string
  propertyName: string
  channel: SyndicationChannel
  status: PlacementStatus
  publishMode: PublishMode
  externalUrl: string | null
  externalId: string | null
  pack: ListingPack | Record<string, never>
  lastError: string | null
  publishedAt: string | null
  expiresAt: string | null
  confirmedAt: string | null
  lastAttemptAt: string | null
  updatedAt: string | null
  /** Root-fact fingerprint captured at last prepare (V3 §2.1). */
  sourceHash: string | null
  /** Raw (ISO) first-publication timestamp for days-on-market; null if never live. */
  publishedAtIso?: string | null
}

export type SightingNetwork = 'zillow' | 'realtor_com' | 'homes_com' | 'other'

export type SightingRow = {
  id: string
  propertyId: string
  network: SightingNetwork
  url: string
  notedAt: string | null
  notes: string | null
}

export type SyndicationEventRow = {
  id: string
  placementId: string
  eventType: string
  detail: Record<string, unknown>
  createdAt: string
}
