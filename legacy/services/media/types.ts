import type { ServiceEnvelopeFor, ServiceOperationName } from '@/legacy/services/core'

export type MediaAssetDto = {
  id: string
  propertyId: string
  mediaType: string
  role: string
  sortOrder: number
  filename: string | null
  mimeType: string | null
  fileSize: number | null
  altText: string | null
  caption: string | null
  createdAt: string | null
  /** Existing media delivery route; this projection never copies the asset. */
  url: string
}

export type ListPropertyMediaRequest = { propertyId: string }

export const MEDIA_OPERATIONS = {
  FOR_PROPERTY: 'media.forProperty',
} as const

export type MediaOperationMap = {
  'media.forProperty': {
    request: ListPropertyMediaRequest
    response: MediaAssetDto[]
  }
}

export type MediaOperationName = ServiceOperationName<MediaOperationMap>
export type MediaEnvelope<K extends MediaOperationName = MediaOperationName> =
  ServiceEnvelopeFor<MediaOperationMap, K>
