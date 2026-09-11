import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { FormRepository } from './repository'
import { FORM_OPERATIONS, type FormOperationMap } from './types'

/**
 * Canonical Forms service.
 *
 * Owns the form instance lifecycle (draft -> ready -> issued), its participants,
 * and the bounded canonical facts used to prefill it. The form BINDINGS stay in
 * lib/forms and continue to reach Person/Property/Contract through the kernel —
 * this service never re-implements another domain.
 */
export class FormService extends BaseService<FormOperationMap> {
  readonly domain = 'form'
  readonly version = '1'
  readonly description =
    'Owns document form instances: the mutable assembly of a template against a deal/client/property.'
  protected readonly operations: ServiceOperationDefinitions<FormOperationMap>

  constructor(
    private readonly repository: FormRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [FORM_OPERATIONS.CREATE_INSTANCE]: {
        kind: 'command',
        description: 'Create a draft form instance for a deal, client, or property.',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'dealId' },
        handle: async (request, context) => {
          const instance = await this.repository.createInstance(request)
          await this.emit(
            {
              type: 'form.instance_created',
              aggregateId: instance.id,
              payload: {
                formInstanceId: instance.id,
                templateId: instance.templateId,
                templateVersion: instance.templateVersion,
                dealId: instance.dealId,
                personId: instance.personId,
                propertyId: instance.propertyId,
              },
            },
            context,
          )
          return instance
        },
      },
      [FORM_OPERATIONS.GET_INSTANCE]: {
        kind: 'query',
        description: 'Return one form instance by id.',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.getInstance(request),
      },
      [FORM_OPERATIONS.UPDATE_INSTANCE]: {
        kind: 'command',
        description: 'Update a form instance field values, sections, or lifecycle status.',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          const instance = await this.repository.updateInstance(request)
          if (instance) {
            await this.emit(
              {
                type: 'form.instance_updated',
                aggregateId: instance.id,
                payload: {
                  formInstanceId: instance.id,
                  status: instance.status,
                  changed: Object.keys(request.input),
                },
              },
              context,
            )
          }
          return instance
        },
      },
      [FORM_OPERATIONS.LIST_INSTANCES]: {
        kind: 'query',
        description: 'List form instances with deal/property/client labels, newest first.',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async () => this.repository.listInstances(),
      },
      [FORM_OPERATIONS.DEAL_FACTS]: {
        kind: 'query',
        description: 'Bounded canonical deal facts used to prefill a form (never mutates).',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.dealFacts(request),
      },
      [FORM_OPERATIONS.SEED_PARTICIPANTS]: {
        kind: 'command',
        description: 'Seed the form participants from the canonical deal participants.',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request) => this.repository.seedParticipantsFromDeal(request),
      },
      [FORM_OPERATIONS.LATEST_EVIDENCE]: {
        kind: 'query',
        description: 'The latest instance of one template for a person — the form evidence a binding reads.',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.latestEvidence(request),
      },
      [FORM_OPERATIONS.RESOLVE_DEAL_LAUNCH_CONTEXT]: {
        kind: 'query',
        description: 'Resolve the client + property a deal launches a form with.',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.resolveDealLaunchContext(request),
      },
      [FORM_OPERATIONS.BIND_DIRECT_CONTEXT]: {
        kind: 'command',
        description: 'Bind a draft instance to an explicit Person/Property context (clears the deal link).',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          await this.repository.bindDirectContext(request)
          await this.emit(
            {
              type: 'form.instance_bound_direct',
              aggregateId: request.formInstanceId,
              payload: {
                formInstanceId: request.formInstanceId,
                personId: request.personId,
                propertyId: request.propertyId,
              },
            },
            context,
          )
        },
      },
      [FORM_OPERATIONS.BIND_LISTING_CONTEXT]: {
        kind: 'command',
        description: 'Change a mutable Listing draft to the selected client + physical property.',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          await this.repository.bindListingContext(request)
          await this.emit(
            {
              type: 'form.listing_context_bound',
              aggregateId: request.formInstanceId,
              payload: {
                formInstanceId: request.formInstanceId,
                personId: request.personId,
                propertyId: request.propertyId,
              },
            },
            context,
          )
        },
      },
      [FORM_OPERATIONS.GET_SHOWING_ID]: {
        kind: 'query',
        description: 'The Showing this instance is bound to, when any.',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.getShowingId(request),
      },
      [FORM_OPERATIONS.BIND_SHOWING]: {
        kind: 'command',
        description: 'Bind a form instance to a Showing.',
        authorization: 'form.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          await this.repository.bindShowing(request)
          await this.emit(
            {
              type: 'form.instance_bound_showing',
              aggregateId: request.formInstanceId,
              payload: {
                formInstanceId: request.formInstanceId,
                showingId: request.showingId,
              },
            },
            context,
          )
        },
      },
      [FORM_OPERATIONS.LIST_SIGNER_PEOPLE]: {
        kind: 'query',
        description: 'Resolve the signer people for an instance (client/seller parties + the broker).',
        authorization: 'form.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.listSignerPeople(request),
      },
    }
  }

  invariants() {
    return [
      'A form instance is mutable working state; the issued document is the immutable record.',
      'Issuance snapshots the instance into a transaction document and marks the instance issued.',
      'Field bindings belong to lib/forms and reach other domains through the kernel.',
      'Form persistence is reachable only through the Form repository boundary.',
    ] as const
  }
}
