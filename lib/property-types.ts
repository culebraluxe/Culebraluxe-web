export type PropertyDetail = {
  _id: string
  title?: string | null
  listingId?: string | null
  standardStatus?: string | null
  propertyType?: string | null
  listPrice?: number | null
  city?: string | null
  stateOrProvince?: string | null
  neighborhood?: string | null
  latitude?: number | null
  longitude?: number | null
  bedroomsTotal?: number | null
  bathroomsFull?: number | null
  bathroomsHalf?: number | null
  bathroomsTotal?: number | null
  livingArea?: number | null
  lotSizeArea?: number | null
  lotSizeUnits?: string | null
  yearBuilt?: number | null
  stories?: number | null
  parkingSpaces?: number | null
  viewType?: string[] | null
  waterAccess?: boolean | null
  beachAccess?: boolean | null
  amenities?: string[] | null
  shortDescription?: string | null
  editorialDescription?: string | null
  architecture?: string | null
  lifestyleTags?: string[] | null
  listingAgentName?: string | null
  listingAgentEmail?: string | null
  listingAgentPhone?: string | null
  listingOffice?: string | null
}

export type GalleryImage = {
  url: string
  alt: string
  caption?: string | null
}

export type PropertyVideo = {
  id: string
  playbackId: string
  role: 'video' | 'short'
  title: string
  caption?: string | null
  aspectRatio?: string | null
  durationSeconds?: number | null
}

export type PropertyDocument = {
  id: string
  title: string
  filename: string
  mimeType: string
  fileSize?: number | null
  sortOrder: number
}

export type PropertyDetailResult = {
  property: PropertyDetail
  heroUrl: string | null
  galleryImages: GalleryImage[]
  videos: PropertyVideo[]
  documents: PropertyDocument[]
}
