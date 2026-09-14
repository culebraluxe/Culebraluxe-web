import type { MediaAssetDto } from './types'

/** Read boundary for canonical media attached to a Property. */
export interface MediaRepository {
  forProperty(propertyId: string): Promise<MediaAssetDto[]>
}
