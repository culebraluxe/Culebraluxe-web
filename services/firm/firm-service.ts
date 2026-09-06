import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { FirmRepository } from './repository'
import { FIRM_OPERATIONS, type FirmOperationMap } from './types'

/** Canonical Firm service: durable business/legal organization identity. */
export class FirmService extends BaseService<FirmOperationMap> {
  readonly domain = 'firm'
  readonly version = '1'
  readonly description = 'Owns canonical business/legal organization identity and intrinsic classification.'
  protected readonly operations: ServiceOperationDefinitions<FirmOperationMap>

  constructor(
    private readonly repository: FirmRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [FIRM_OPERATIONS.GET]: {
        kind: 'query',
        description: 'Return one canonical Firm by id.',
        authorization: 'firm.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.get(request.firmId),
      },
      [FIRM_OPERATIONS.FIND_BY_NAME]: {
        kind: 'query',
        description: 'Resolve a canonical Firm by legal or display name.',
        authorization: 'firm.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.findByName(request),
      },
      [FIRM_OPERATIONS.UPSERT]: {
        kind: 'command',
        description: 'Create or enrich a canonical Firm without assigning Contract roles intrinsically.',
        authorization: 'firm.write',
        execution: { mode: 'ordered', partitionBy: 'name' },
        handle: async (request, context) => {
          const firm = await this.repository.upsert(request)
          await this.emit(
            {
              type: 'firm.upserted',
              aggregateId: firm.id,
              payload: {
                firmId: firm.id,
                name: firm.name,
                legalName: firm.legalName,
                kind: firm.kind,
              },
            },
            context,
          )
          return firm
        },
      },
    }
  }

  invariants() {
    return [
      'Firm owns durable business/legal organization identity; Contract roles such as LENDER or SELLER_BROKERAGE are contextual mappings.',
      'Firm classification such as bank, brokerage, law firm, or LLC is intrinsic metadata and is not a Contract role.',
      'Firm addresses reuse canonical Property through relationships; Firm does not invent a second address model.',
      'Firm persistence is reachable only through the Firm repository boundary.',
    ] as const
  }
}
