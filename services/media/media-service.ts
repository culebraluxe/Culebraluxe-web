import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { MediaRepository } from './repository'
import { MEDIA_OPERATIONS, type MediaOperationMap } from './types'

/** Read-only media service used by project/property surfaces. */
export class MediaService extends BaseService<MediaOperationMap> {
  readonly domain = 'media'
  readonly version = '1'
  readonly description = 'Reads canonical media assets linked to Properties.'
  protected readonly operations: ServiceOperationDefinitions<MediaOperationMap>

  constructor(
    private readonly repository: MediaRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)
    this.operations = {
      [MEDIA_OPERATIONS.FOR_PROPERTY]: {
        kind: 'query',
        description: 'List canonical media attached to one Property.',
        authorization: 'property.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.forProperty(request.propertyId),
      },
    }
  }

  invariants() {
    return [
      'MediaService is read-only in this slice; it never copies, renames, or deletes property media.',
      'Property media remains authoritative in media + property_media.',
    ] as const
  }
}
