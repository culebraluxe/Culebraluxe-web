import {
  BaseService,
  RoleCatalog,
  type RoleDefinition,
  type RoleScope,
  type ServiceContext,
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
  type ContractRoleDto,
  type SaveContractDraftRequest,
} from './types'

/** Canonical Contract Role vocabulary — codes sourced from lib/forms (pns-field-binding). */
const CONTRACT_ROLE_DEFINITIONS: readonly RoleDefinition[] = [
  { scope: 'contract_person', code: 'BUYER', name: 'Buyer' },
  { scope: 'contract_person', code: 'SELLER', name: 'Seller' },
  { scope: 'contract_person', code: 'SELLER_REPRESENTATIVE', name: 'Seller Representative' },
  { scope: 'contract_person', code: 'BUYER_BROKER', name: 'Buyer Broker' },
  { scope: 'contract_person', code: 'SELLER_BROKER', name: 'Seller Broker' },
  { scope: 'contract_person', code: 'SELLER_SPOUSE', name: 'Seller Spouse' },
  { scope: 'contract_person', code: 'CLOSING_NOTARY', name: 'Closing Notary' },
  { scope: 'contract_firm', code: 'BUYER', name: 'Buyer' },
  { scope: 'contract_firm', code: 'SELLER', name: 'Seller' },
  { scope: 'contract_firm', code: 'BUYER_BROKERAGE', name: 'Buyer Brokerage' },
  { scope: 'contract_firm', code: 'SELLER_BROKERAGE', name: 'Seller Brokerage' },
  { scope: 'contract_firm', code: 'ESCROW_HOLDER', name: 'Escrow Holder' },
  { scope: 'contract_firm', code: 'LENDER', name: 'Lender' },
  { scope: 'contract_firm', code: 'CLOSING_NOTARY', name: 'Closing Notary' },
]

const contractRoleCatalog = new RoleCatalog(CONTRACT_ROLE_DEFINITIONS)

function roleScopeFor(kind: ContractRoleDto['kind']): RoleScope {
  return kind === 'person' ? 'contract_person' : 'contract_firm'
}

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
          this.assertKnownRoleCodes(request.roles)
          await this.assertContractParties(request, context)
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
      [CONTRACT_OPERATIONS.SAVE_DRAFT]: {
        kind: 'command',
        description: 'Create or replace mutable Contract draft truth from current Form working state.',
        authorization: 'contract.write',
        execution: { mode: 'ordered', partitionBy: 'contractId' },
        handle: async (request, context) => {
          this.assertKnownRoleCodes(request.roles)
          const existing = await this.repository.get(request.contractId)
          if (existing && existing.status !== 'draft') {
            this.fail(
              'CONTRACT_IMMUTABLE',
              `Contract ${request.contractId} is ${existing.status}; it is no longer an editable draft.`,
            )
          }
          await this.assertContractParties(request, context)
          const contract = await this.repository.saveDraft(request)
          await this.emit(
            {
              type: 'contract.draft_saved',
              aggregateId: contract.id,
              payload: {
                contractId: contract.id,
                contractType: contract.contractType,
                formTemplateId: contract.formTemplateId,
                propertyId: contract.propertyId,
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
          const existing = await this.repository.get(request.contractId)
          if (!existing) {
            this.fail('CONTRACT_NOT_FOUND', `Contract not found: ${request.contractId}`)
          }
          if (existing.status === 'executed') {
            this.fail(
              'CONTRACT_ALREADY_EXECUTED',
              `Contract ${request.contractId} is already executed.`,
            )
          }
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

  /**
   * Single ownership of the parties invariant shared by createFromForm and
   * saveDraft: property must exist via Property, every Person/Firm id must exist
   * via its owning service, and a provided predecessor Contract must exist.
   */
  private async assertContractParties(
    request: SaveContractDraftRequest,
    context: ServiceContext,
  ): Promise<void> {
    const property = await this.callService<
      PropertyOperationMap,
      typeof PROPERTY_OPERATIONS.GET
    >('property', PROPERTY_OPERATIONS.GET, { propertyId: request.propertyId }, context)
    if (!property) this.fail('PROPERTY_NOT_FOUND', `Property not found: ${request.propertyId}`)

    const personIds = [
      ...new Set(
        request.roles.flatMap((role) => (role.kind === 'person' ? [role.personId] : [])),
      ),
    ]
    const firmIds = [
      ...new Set(
        request.roles.flatMap((role) => (role.kind === 'firm' ? [role.firmId] : [])),
      ),
    ]

    await Promise.all([
      ...personIds.map(async (personId) => {
        const person = await this.callService<PersonOperationMap, typeof PERSON_OPERATIONS.GET>(
          'person',
          PERSON_OPERATIONS.GET,
          { personId },
          context,
        )
        if (!person) this.fail('PERSON_NOT_FOUND', `Person not found: ${personId}`)
      }),
      ...firmIds.map(async (firmId) => {
        const firm = await this.callService<FirmOperationMap, typeof FIRM_OPERATIONS.GET>(
          'firm',
          FIRM_OPERATIONS.GET,
          { firmId },
          context,
        )
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
  }

  /** Reject empty or unknown Contract Role codes. */
  private assertKnownRoleCodes(roles: readonly ContractRoleDto[]): void {
    for (const role of roles) {
      if (!role.roleCode.trim()) {
        this.fail('ROLE_REQUIRED', 'Every Contract identity mapping requires a Role code.')
      }
      const scope = roleScopeFor(role.kind)
      if (!contractRoleCatalog.resolve(scope, role.roleCode)) {
        this.fail('ROLE_UNKNOWN', `Unknown ${scope} role code: ${role.roleCode}`)
      }
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
      'Draft Contract truth is mutable; executed/issued Contract truth is not rewritten through draft save.',
      'Workflow owns flow/lifecycle orchestration; Contract owns business truth.',
      'Cross-domain access occurs through the owning service contract, never another domain repository.',
      'Contract-chain effective state preserves lineage rather than duplicating truth into a separate Transaction domain.',
    ] as const
  }
}
