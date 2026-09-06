import type {
  PageIntentEnvelope,
  PageIntentMap,
  PageIntentName,
  PageIntentResponse,
  PageListener,
  PageOperationContext,
  PageOperationDefinition,
  PageOperationDefinitions,
  PageStore,
} from './types'

/**
 * TypeScript/MVI page runtime.
 *
 * Concrete controllers contribute a page model plus an intent/operation map.
 * This parent owns the stable dispatch ingress, model publication, and the
 * concurrency semantics that otherwise get reimplemented in React components.
 */
export abstract class BasePageController<
  TModel,
  TMap extends PageIntentMap,
> implements PageStore<TModel> {
  protected abstract readonly operations: PageOperationDefinitions<TModel, TMap>

  private model: TModel
  private readonly listeners = new Set<PageListener<TModel>>()
  private readonly serialTails = new Map<string, Promise<void>>()
  private readonly latestControllers = new Map<string, AbortController>()
  private disposed = false

  protected constructor(initialModel: TModel) {
    this.model = initialModel
  }

  snapshot = (): Readonly<TModel> => this.model

  subscribe = (listener: PageListener<TModel>): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  dispose(): void {
    this.disposed = true
    for (const controller of this.latestControllers.values()) controller.abort()
    this.latestControllers.clear()
    this.serialTails.clear()
    this.listeners.clear()
  }

  async dispatch<K extends PageIntentName<TMap>>(
    intent: PageIntentEnvelope<TMap, K>,
  ): Promise<PageIntentResponse<TMap, K>> {
    if (this.disposed) throw new Error('Page controller is disposed.')

    const definition = (
      this.operations as Record<
        string,
        PageOperationDefinition<TModel, unknown, unknown>
      >
    )[intent.operation]

    if (!definition) throw new Error(`Unknown page intent: ${intent.operation}`)

    const execution = definition.execution ?? 'parallel'

    if (execution === 'serial') {
      return this.dispatchSerial(intent.operation, definition, intent.payload) as Promise<
        PageIntentResponse<TMap, K>
      >
    }

    if (execution === 'latest') {
      this.latestControllers.get(intent.operation)?.abort()
      const controller = new AbortController()
      this.latestControllers.set(intent.operation, controller)

      try {
        return (await this.runOperation(
          intent.operation,
          definition,
          intent.payload,
          controller,
          () => this.latestControllers.get(intent.operation) === controller,
        )) as PageIntentResponse<TMap, K>
      } finally {
        if (this.latestControllers.get(intent.operation) === controller) {
          this.latestControllers.delete(intent.operation)
        }
      }
    }

    const controller = new AbortController()
    return this.runOperation(
      intent.operation,
      definition,
      intent.payload,
      controller,
      () => !controller.signal.aborted,
    ) as Promise<PageIntentResponse<TMap, K>>
  }

  protected replaceModel(model: TModel): void {
    if (this.disposed) return
    this.model = model
    this.publish()
  }

  protected updateModel(reducer: (model: Readonly<TModel>) => TModel): void {
    this.replaceModel(reducer(this.model))
  }

  private dispatchSerial(
    operation: string,
    definition: PageOperationDefinition<TModel, unknown, unknown>,
    payload: unknown,
  ): Promise<unknown> {
    const previous = this.serialTails.get(operation) ?? Promise.resolve()
    const controller = new AbortController()
    const run = previous
      .catch(() => undefined)
      .then(() =>
        this.runOperation(
          operation,
          definition,
          payload,
          controller,
          () => !controller.signal.aborted,
        ),
      )

    const tail = run.then(() => undefined, () => undefined)
    this.serialTails.set(operation, tail)
    void tail.finally(() => {
      if (this.serialTails.get(operation) === tail) this.serialTails.delete(operation)
    })
    return run
  }

  private async runOperation(
    _operation: string,
    definition: PageOperationDefinition<TModel, unknown, unknown>,
    payload: unknown,
    controller: AbortController,
    isCurrent: () => boolean,
  ): Promise<unknown> {
    const context: PageOperationContext<TModel> = {
      signal: controller.signal,
      snapshot: () => this.model,
      isCurrent: () => !this.disposed && !controller.signal.aborted && isCurrent(),
      update: (reducer) => {
        if (this.disposed || controller.signal.aborted || !isCurrent()) return
        this.model = reducer(this.model)
        this.publish()
      },
    }

    return definition.handle(payload, context)
  }

  private publish(): void {
    for (const listener of this.listeners) listener(this.model)
  }
}
