import type { PlacementStatus, PublishMode, SyndicationChannel } from './channels'

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
}

export type AdapterResult = {
  ok: boolean
  mode: PublishMode
  status: PlacementStatus
  pack: ListingPack
  message: string
  ttlDays: number | null
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
}

export type SyndicationEventRow = {
  id: string
  placementId: string
  eventType: string
  detail: Record<string, unknown>
  createdAt: string
}
