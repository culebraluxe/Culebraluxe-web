import type { MediaAssetDto } from '@/legacy/services/media/types'

/** Read boundary for canonical media attached to a Property. */
export interface MediaRepository {
  forProperty(propertyId: string): Promise<MediaAssetDto[]>
}
