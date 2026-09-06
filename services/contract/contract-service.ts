import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import {
  PERSON_OPERATIONS,
  type PersonOperationMap,
} from '../person'
import {
  PROPERTY_OPERATIONS,
  type PropertyOperationMap,
} from '../property'
import type { ContractRepository } from './repository'
import {
  CONTRACT_OPERATIONS,
  type ContractOperationMap,
} from './types'

/**
 * Canonical Contract service.
 *
 * Interaction rule demonstrated here:
 * - required synchronous truth is requested from the owning service through ServiceRouter
 * - no cross-domain repository/concrete-service import is allowed
 * - consequences (including Workflow advancement) are announced as domain events
 */
export class ContractService extends BaseService<ContractOperationMap> {
  readonly domain = 'contract'
  readonly version = '1'
  readonly description = 'Owns Form-created contract instances, participants, facts, and execution state.'
  protected readonly operations: ServiceOperationDefinitions<ContractOperationMap>

  constructor(
    private readonly repository: ContractRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [CONTRACT_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical Contract by id.',
        authorization: 'contract.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.contractId),
      },
      [CONTRACT_OPERATIONS.CREATE_FROM_FORM]: {
        kind: 'command',
        description: 'Create a durable Contract instance from a Form-defined contract type.',
        authorization: 'contract.write',
        execution: { mode: 'ordered', partitionBy: 'contractId' },
        handle: async (request, context) => {
          const property = await this.callService<
            PropertyOperationMap,
            typeof PROPERTY_OPERATIONS.GET
          >(
            'property',
            PROPERTY_OPERATIONS.GET,
            { propertyId: request.propertyId },
            context,
          )
          if (!property) {
            this.fail('PROPERTY_NOT_FOUND', `Property not found: ${request.propertyId}`)
          }

          const personIds = [...new Set(request.participants.map((participant) => participant.personId))]
          await Promise.all(
            personIds.map(async (personId) => {
              const person = await this.callService<
                PersonOperationMap,
                typeof PERSON_OPERATIONS.GET
              >(
                'person',
                PERSON_OPERATIONS.GET,
                { personId },
                context,
              )
              if (!person) this.fail('PERSON_NOT_FOUND', `Person not found: ${personId}`)
            }),
          )

          const contract = await this.repository.createFromForm(request)
          await this.emit(
            {
              type: 'contract.created',
              aggregateId: contract.id,
              payload: {
                contractId: contract.id,
                contractType: contract.contractType,
                formTemplateId: contract.formTemplateId,
                propertyId: contract.propertyId,
                participantCount: contract.participants.length,
              },
            },
            context,
          )
          return contract
        },
      },
      [CONTRACT_OPERATIONS.GET_EFFECTIVE_STATE]: {
        kind: 'query',
        description: 'Project the effective facts for a Contract chain without creating duplicate business truth.',
        authorization: 'contract.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.getEffectiveState(request.contractId),
      },
      [CONTRACT_OPERATIONS.EXECUTE]: {
        kind: 'command',
        description: 'Record Contract execution and announce the lifecycle fact for Workflow and other consumers.',
        authorization: 'contract.execute',
        execution: { mode: 'ordered', partitionBy: 'contractId' },
        handle: async (request, context) => {
          const contract = await this.repository.execute(request)
          await this.emit(
            {
              type: 'contract.executed',
              aggregateId: contract.id,
              payload: {
                contractId: contract.id,
                contractType: contract.contractType,
                evidenceDocumentId: request.evidenceDocumentId,
              },
            },
            context,
          )
          return contract
        },
      },
    }
  }

  dependencies() {
    return ['person', 'property'] as const
  }

  invariants() {
    return [
      'Every Form creates or changes a Contract; the Contract owns the scoped business facts defined by that Form.',
      'Person and Property remain canonical independent entities referenced by Contract.',
      'Contract roles such as buyer, seller, owner, and agent are contextual participant roles, not Person identities.',
      'Workflow owns flow/lifecycle orchestration; Contract owns business truth.',
      'Cross-domain access occurs through the owning service contract, never another domain repository.',
      'Contract-chain effective state preserves lineage rather than duplicating truth into a separate Transaction domain.',
    ] as const
  }
}
