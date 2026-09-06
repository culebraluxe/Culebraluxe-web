import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import { PERSON_OPERATIONS, type PersonOperationMap } from '../person'
import { PROPERTY_OPERATIONS, type PropertyOperationMap } from '../property'
import type { ShowingRepository } from './repository'
import { SHOWING_OPERATIONS, type ShowingOperationMap } from './types'

export class ShowingService extends BaseService<ShowingOperationMap> {
  readonly domain = 'showing'
  readonly version = '1'
  readonly description = 'Owns one Person visiting one Property and the resulting showing report facts.'
  protected readonly operations: ServiceOperationDefinitions<ShowingOperationMap>

  constructor(
    private readonly repository: ShowingRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [SHOWING_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical Showing by id.',
        authorization: 'showing.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.showingId),
      },
      [SHOWING_OPERATIONS.SAVE_REPORT]: {
        kind: 'command',
        description: 'Create or enrich a Showing from a SHOW-RPT form without creating Deal truth.',
        authorization: 'showing.write',
        execution: { mode: 'ordered', partitionBy: 'showingId' },
        handle: async (request, context) => {
          if (!request.showingId.trim()) this.fail('SHOWING_ID_REQUIRED', 'showingId is required.')
          if (!request.personId.trim()) this.fail('PERSON_REQUIRED', 'Showing requires a Person.')
          if (!request.propertyId.trim()) this.fail('PROPERTY_REQUIRED', 'Showing requires a Property.')
          if (
            request.interestScore !== null &&
            (!Number.isInteger(request.interestScore) || request.interestScore < 1 || request.interestScore > 5)
          ) {
            this.fail('INTEREST_SCORE_INVALID', 'Showing interest score must be an integer from 1 to 5.')
          }

          const [person, property] = await Promise.all([
            this.callService<PersonOperationMap, typeof PERSON_OPERATIONS.GET>(
              'person',
              PERSON_OPERATIONS.GET,
              { personId: request.personId },
              context,
            ),
            this.callService<PropertyOperationMap, typeof PROPERTY_OPERATIONS.GET>(
              'property',
              PROPERTY_OPERATIONS.GET,
              { propertyId: request.propertyId },
              context,
            ),
          ])
          if (!person) this.fail('PERSON_NOT_FOUND', `Person not found: ${request.personId}`)
          if (!property) this.fail('PROPERTY_NOT_FOUND', `Property not found: ${request.propertyId}`)

          const showing = await this.repository.saveReport(request)
          await this.emit(
            {
              type: 'showing.report_saved',
              aggregateId: showing.id,
              payload: {
                showingId: showing.id,
                personId: showing.personId,
                propertyId: showing.propertyId,
                status: showing.status,
                outcome: showing.outcome,
                interestScore: showing.interestScore,
              },
            },
            context,
          )
          return showing
        },
      },
    }
  }

  dependencies() {
    return ['person', 'property'] as const
  }

  invariants() {
    return [
      'Showing is explicit Person + Property context; BUYER/PROSPECT is contextual and is never intrinsic Person truth.',
      'SHOW-RPT enriches the existing Showing record; it never creates a parallel report aggregate.',
      'A Form binds to Showing by explicit showing_id; latest Person/Property rows are never guessed.',
      'Showing business date is date-only unless an actual time source exists.',
      'Deal is not canonical truth for new Showing Reports.',
    ] as const
  }
}
