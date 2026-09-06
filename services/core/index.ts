export { BaseService } from './base-service'
export { ServiceError } from './service-error'
export { InMemoryServiceQueue } from './service-queue'
export { ServiceRegistry } from './service-registry'
export {
  RelationRoleCatalog,
  normalizeRelationRoleAlias,
  type RelationRoleDefinition,
  type RelationScope,
  type ResolvedRelationRole,
} from './relation-role'
export type {
  AuditPort,
  AuthorizationPort,
  AuthorizationRequest,
  DomainEventPort,
  ServiceActor,
  ServiceAuditEvent,
  ServiceCapability,
  ServiceContext,
  ServiceDescriptor,
  ServiceDomainEvent,
  ServiceEndpoint,
  ServiceEnvelope,
  ServiceEnvelopeFor,
  ServiceErrorShape,
  ServiceExecutionMode,
  ServiceExecutionPolicy,
  ServiceFailure,
  ServiceInfrastructure,
  ServiceOperationContract,
  ServiceOperationDefinition,
  ServiceOperationDefinitions,
  ServiceOperationKind,
  ServiceOperationMap,
  ServiceOperationName,
  ServiceOperationRequest,
  ServiceOperationResponse,
  ServiceQueue,
  ServiceQueueItem,
  ServiceResult,
  ServiceRouter,
  ServiceSuccess,
} from './types'
