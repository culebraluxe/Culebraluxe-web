export type ServiceActor = {
  id: string | null
  kind: 'user' | 'system' | 'agent'
}

/** Per-call metadata. Services are stateless/singleton-safe; call context travels with the invocation. */
export type ServiceContext = {
  actor: ServiceActor
  correlationId: string
  causationId?: string
}

/** Infrastructure supplied once when the service is composed. */
export type ServiceInfrastructure = {
  authorization?: AuthorizationPort
  audit?: AuditPort
  events?: DomainEventPort
}

export type AuthorizationRequest = {
  domain: string
  action: string
  actor: ServiceActor
}

export interface AuthorizationPort {
  authorize(request: AuthorizationRequest): Promise<boolean>
}

export type ServiceAuditEvent = {
  domain: string
  operation: string
  actor: ServiceActor
  correlationId: string
  causationId?: string
  outcome: 'success' | 'failure'
  errorCode?: string
}

export interface AuditPort {
  record(event: ServiceAuditEvent): Promise<void>
}

export type ServiceDomainEvent = {
  type: string
  aggregateId?: string
  payload: Readonly<Record<string, unknown>>
  correlationId: string
  causationId?: string
}

export interface DomainEventPort {
  emit(event: ServiceDomainEvent): Promise<void>
}

export type ServiceOperationKind = 'query' | 'command'

export type ServiceCapability = {
  name: string
  kind: ServiceOperationKind
  description: string
  authorization?: string
  idempotent?: boolean
}

export type ServiceDescriptor = {
  domain: string
  version: string
  description: string
  capabilities: readonly ServiceCapability[]
  dependencies: readonly string[]
  invariants: readonly string[]
}

/** Transport-neutral service envelope: operation + simple DTO + per-call context. */
export type ServiceEnvelope<TOperation extends string = string, TPayload = unknown> = {
  operation: TOperation
  payload: TPayload
  context: ServiceContext
}

export type ServiceSuccess<T> = {
  ok: true
  value: T
  correlationId: string
}

export type ServiceFailure = {
  ok: false
  error: ServiceErrorShape
  correlationId: string
}

export type ServiceResult<T> = ServiceSuccess<T> | ServiceFailure

export type ServiceErrorShape = {
  code: string
  message: string
  retryable: boolean
  cause?: unknown
}
