export type ServiceActor = {
  id: string | null
  kind: 'user' | 'system' | 'agent'
}

export type ServiceContext = {
  actor: ServiceActor
  correlationId: string
  causationId?: string
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

export type ServiceCapability = {
  name: string
  description: string
}

export type ServiceDescriptor = {
  domain: string
  version: string
  description: string
  capabilities: readonly ServiceCapability[]
  dependencies: readonly string[]
  invariants: readonly string[]
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
