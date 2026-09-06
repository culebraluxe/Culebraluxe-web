export type PageExecutionMode = 'parallel' | 'latest' | 'serial'

export type PageIntentMap = Record<string, {
  request: unknown
  response: unknown
}>

export type PageIntentName<TMap extends PageIntentMap> = Extract<keyof TMap, string>

export type PageIntentRequest<
  TMap extends PageIntentMap,
  K extends PageIntentName<TMap>,
> = TMap[K]['request']

export type PageIntentResponse<
  TMap extends PageIntentMap,
  K extends PageIntentName<TMap>,
> = TMap[K]['response']

export type PageIntentEnvelope<
  TMap extends PageIntentMap,
  K extends PageIntentName<TMap> = PageIntentName<TMap>,
> = {
  operation: K
  payload: PageIntentRequest<TMap, K>
}

export type PageModelReducer<TModel> = (model: Readonly<TModel>) => TModel

export type PageOperationContext<TModel> = {
  readonly signal: AbortSignal
  snapshot(): Readonly<TModel>
  update(reducer: PageModelReducer<TModel>): void
  isCurrent(): boolean
}

export type PageOperationDefinition<TModel, TRequest, TResponse> = {
  description: string
  execution?: PageExecutionMode
  handle(
    request: TRequest,
    context: PageOperationContext<TModel>,
  ): Promise<TResponse> | TResponse
}

export type PageOperationDefinitions<
  TModel,
  TMap extends PageIntentMap,
> = {
  [K in PageIntentName<TMap>]: PageOperationDefinition<
    TModel,
    PageIntentRequest<TMap, K>,
    PageIntentResponse<TMap, K>
  >
}

export type PageListener<TModel> = (model: Readonly<TModel>) => void

export interface PageStore<TModel> {
  snapshot(): Readonly<TModel>
  subscribe(listener: PageListener<TModel>): () => void
}
