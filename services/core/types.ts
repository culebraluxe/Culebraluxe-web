export type ServiceActor = {
  id: string | null
  kind: 'user' | 'system' | 'agent'
}

/** Per-call metadata. Service instances stay stateless/singleton-safe. */
export type ServiceContext = {
  actor: ServiceActor
  correlationId: string
  causationId?: string
}

export type ServiceOperationKind = 'query' | 'command'
export type ServiceExecutionMode = 'inline' | 'queued' | 'ordered'

export type ServiceExecutionPolicy = {
  mode: ServiceExecutionMode
  /** DTO field used as the ordering/partition key when mode = ordered. */
  partitionBy?: string
}

export type ServiceCapability = {
  name: string
  kind: ServiceOperationKind
  description: string
  authorization?: string
  idempotent?: boolean
  execution?: ServiceExecutionPolicy
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

export type ServiceOperationContract<TRequest = unknown, TResponse = unknown> = {
  request: TRequest
  response: TResponse
}

export type ServiceOperationMap = Record<string, ServiceOperationContract>
export type ServiceOperationName<TMap extends ServiceOperationMap> = Extract<keyof TMap, string>

export type ServiceOperationRequest<
  TMap extends ServiceOperationMap,
  K extends ServiceOperationName<TMap>,
> = TMap[K] extends ServiceOperationContract<infer TRequest, unknown> ? TRequest : never

export type ServiceOperationResponse<
  TMap extends ServiceOperationMap,
  K extends ServiceOperationName<TMap>,
> = TMap[K] extends ServiceOperationContract<unknown, infer TResponse> ? TResponse : never

export type ServiceEnvelopeFor<
  TMap extends ServiceOperationMap,
  K extends ServiceOperationName<TMap> = ServiceOperationName<TMap>,
> = ServiceEnvelope<K, ServiceOperationRequest<TMap, K>>

export type ServiceOperationDefinition<TRequest, TResponse> = Omit<ServiceCapability, 'name'> & {
  handle(request: TRequest, context: ServiceContext): Promise<TResponse>
}

/** One typed definition binds metadata + request + response + handler together. */
export type ServiceOperationDefinitions<TMap extends ServiceOperationMap> = {
  [K in keyof TMap]: TMap[K] extends ServiceOperationContract<infer TRequest, infer TResponse>
    ? ServiceOperationDefinition<TRequest, TResponse>
    : never
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

export type ServiceQueueItem = {
  domain: string
  envelope: ServiceEnvelope
  execution: ServiceExecutionPolicy
  partitionKey?: string
}

/** Scheduling/mailbox seam owned by BaseService. */
export interface ServiceQueue {
  submit<T>(item: ServiceQueueItem, process: () => Promise<T>): Promise<T>
}

/** Cross-service transport seam. Local registry today; remote/message transport later. */
export interface ServiceRouter {
  dispatch<TResponse = unknown>(
    domain: string,
    envelope: ServiceEnvelope,
  ): Promise<ServiceResult<TResponse>>
}

/** Infrastructure supplied once when the service is composed. */
export type ServiceInfrastructure = {
  authorization?: AuthorizationPort
  audit?: AuditPort
  events?: DomainEventPort
  queue?: ServiceQueue
  router?: ServiceRouter
}

/** Untyped transport edge; concrete services preserve their typed execute() surface. */
export interface ServiceEndpoint {
  readonly domain: string
  describe(): ServiceDescriptor
  dispatch(envelope: ServiceEnvelope): Promise<ServiceResult<unknown>>
}
