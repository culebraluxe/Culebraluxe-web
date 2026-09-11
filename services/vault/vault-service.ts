import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { VaultRepository } from './repository'
import { VAULT_OPERATIONS, type VaultOperationMap } from './types'

/**
 * Canonical Vault service — the issued-document domain.
 *
 * Forms owns the mutable instance; the Vault owns the immutable record the
 * instance becomes when it is issued: the rendered artifact and the
 * transaction_document row that can only be superseded, never edited.
 *
 * The Cabinet (/portal/documents) reads through here.
 */
export class VaultService extends BaseService<VaultOperationMap> {
  readonly domain = 'vault'
  readonly version = '1'
  readonly description = 'Owns issued documents: the immutable rendered artifacts and their lineage.'
  protected readonly operations: ServiceOperationDefinitions<VaultOperationMap>

  constructor(
    private readonly repository: VaultRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [VAULT_OPERATIONS.LIST_ISSUED_DOCUMENTS]: {
        kind: 'query',
        description: 'List issued documents newest first, scoped for external actors.',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.listIssuedDocuments(request),
      },
      [VAULT_OPERATIONS.GET_DOCUMENT]: {
        kind: 'query',
        description: 'Return one document by id.',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.getDocument(request),
      },
      [VAULT_OPERATIONS.LIST_BY_DEAL]: {
        kind: 'query',
        description: 'List the documents attached to a deal.',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.listByDeal(request),
      },
      [VAULT_OPERATIONS.CREATE_DOCUMENT]: {
        kind: 'command',
        description: 'Register a document row (externally-sourced documents are idempotent).',
        authorization: 'vault.write',
        execution: { mode: 'ordered', partitionBy: 'dealId' },
        handle: async (request, context) => {
          const document = await this.repository.createDocument(request)
          await this.emit(
            {
              type: 'vault.document_created',
              aggregateId: document.id,
              payload: {
                documentId: document.id,
                dealId: document.dealId,
                documentType: document.documentType,
                source: document.source,
              },
            },
            context,
          )
          return document
        },
      },
      [VAULT_OPERATIONS.TRANSITION_STATE]: {
        kind: 'command',
        description: 'Move a document through its state machine (claim-first receipt).',
        authorization: 'vault.write',
        execution: { mode: 'ordered', partitionBy: 'documentId' },
        handle: async (request, context) => {
          const result = await this.repository.transitionState(request)
          if (result.outcome === 'success') {
            await this.emit(
              {
                type: 'vault.document_state_changed',
                aggregateId: request.documentId,
                payload: { documentId: request.documentId, to: request.input.to },
              },
              context,
            )
          }
          return result
        },
      },
      [VAULT_OPERATIONS.ISSUE_FROM_FORM_INSTANCE]: {
        kind: 'command',
        description:
          'Issue a form instance as immutable evidence: render, store the artifact, append the document, mark the instance issued — one transaction.',
        authorization: 'vault.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          const result = await this.repository.issueFromFormInstance(request)
          if (result.outcome === 'success') {
            await this.emit(
              {
                type: 'vault.document_issued',
                aggregateId: result.aggregateId ?? request.formInstanceId,
                payload: { formInstanceId: request.formInstanceId, commandId: request.commandId },
              },
              context,
            )
          }
          return result
        },
      },
      [VAULT_OPERATIONS.ISSUED_FOR_FORM_INSTANCE]: {
        kind: 'query',
        description: 'The issued document a form instance produced, when any.',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.issuedForFormInstance(request),
      },
      [VAULT_OPERATIONS.NEXT_ISSUED_VERSION]: {
        kind: 'query',
        description:
          'The lineage version the next issuance will use (preview and issuance must agree).',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.nextIssuedVersion(request),
      },
      [VAULT_OPERATIONS.MEDIA_BYTES]: {
        kind: 'query',
        description: 'The exact stored bytes of an issued artifact (download / verification).',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.mediaBytes(request),
      },
      [VAULT_OPERATIONS.FORM_CONTRACT_ID]: {
        kind: 'query',
        description: 'The Contract a form instance is bound to, when any.',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.formContractId(request),
      },
      [VAULT_OPERATIONS.BIND_FORM_TO_CONTRACT]: {
        kind: 'command',
        description: 'Bind a form instance to its Contract (never transferable).',
        authorization: 'vault.write',
        execution: { mode: 'ordered', partitionBy: 'formInstanceId' },
        handle: async (request, context) => {
          await this.repository.bindFormToContract(request)
          await this.emit(
            {
              type: 'vault.form_bound_to_contract',
              aggregateId: request.formInstanceId,
              payload: { formInstanceId: request.formInstanceId, contractId: request.contractId },
            },
            context,
          )
        },
      },
      [VAULT_OPERATIONS.PRIOR_CONTRACT_DOCUMENT]: {
        kind: 'query',
        description: 'The prior issued document for a Contract + template (lineage/supersession).',
        authorization: 'vault.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => this.repository.priorContractDocument(request),
      },
    }
  }

  invariants() {
    return [
      'An issued document is immutable: it is superseded, never edited.',
      'Issuance is one transaction: render, store the artifact, append the document, mark the form issued.',
      'The form instance is the mutable draft; the Vault owns the record it becomes.',
      'Document persistence is reachable only through the Vault repository boundary.',
    ] as const
  }
}
