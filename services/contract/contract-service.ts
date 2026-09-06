import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import {
  FIRM_OPERATIONS,
  type FirmOperationMap,
} from '../firm'
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
 * Required current truth is requested from the owning service. Contract owns
 * agreement-specific Role mappings/facts and announces lifecycle consequences.
 */
export class ContractService extends BaseService<ContractOperationMap> {
  readonly domain = 'contract'
  readonly version = '1'
  readonly description = 'Owns Form-created contract instances, contextual Roles, facts, lineage, and execution state.'
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
          if (!property) this.fail('PROPERTY_NOT_FOUND', `Property not found: ${request.propertyId}`)

          const invalidRole = request.roles.find((role) => !role.roleCode.trim())
          if (invalidRole) this.fail('ROLE_REQUIRED', 'Every Contract identity mapping requires a Role code.')

          const personIds = [...new Set(
            request.roles.flatMap((role) => role.kind === 'person' ? [role.personId] : []),
          )]
          const firmIds = [...new Set(
            request.roles.flatMap((role) => role.kind === 'firm' ? [role.firmId] : []),
          )]

          await Promise.all([
            ...personIds.map(async (personId) => {
              const person = await this.callService<
                PersonOperationMap,
                typeof PERSON_OPERATIONS.GET
              >('person', PERSON_OPERATIONS.GET, { personId }, context)
              if (!person) this.fail('PERSON_NOT_FOUND', `Person not found: ${personId}`)
            }),
            ...firmIds.map(async (firmId) => {
              const firm = await this.callService<
                FirmOperationMap,
                typeof FIRM_OPERATIONS.GET
              >('firm', FIRM_OPERATIONS.GET, { firmId }, context)
              if (!firm) this.fail('FIRM_NOT_FOUND', `Firm not found: ${firmId}`)
            }),
          ])

          if (request.predecessorContractId) {
            const predecessor = await this.repository.get(request.predecessorContractId)
            if (!predecessor) {
              this.fail(
                'PREDECESSOR_CONTRACT_NOT_FOUND',
                `Predecessor Contract not found: ${request.predecessorContractId}`,
              )
            }
          }

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
                predecessorContractId: contract.predecessorContractId,
                roleCount: contract.roles.length,
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
                evidenceDocumentId: contract.evidenceDocumentId,
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
    return ['person', 'firm', 'property'] as const
  }

  invariants() {
    return [
      'Every Form creates or changes a Contract; the Contract owns the scoped business facts defined by that Form.',
      'Person, Firm, and Property remain canonical independent entities referenced by Contract.',
      'A Contract Role is a contextual position; Person and Firm identities map to Roles and do not own those Roles intrinsically.',
      'Role vocabulary is normalized data, not a garden of buyer/seller/lender/broker service methods.',
      'Contract role attributes hold agreement-specific capacity/assertions; they are not silently promoted to Person or Firm truth.',
      'Workflow owns flow/lifecycle orchestration; Contract owns business truth.',
      'Cross-domain access occurs through the owning service contract, never another domain repository.',
      'Contract-chain effective state preserves lineage rather than duplicating truth into a separate Transaction domain.',
    ] as const
  }
}
