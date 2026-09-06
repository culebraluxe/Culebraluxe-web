import type { ServiceQueue, ServiceQueueItem } from './types'

/**
 * Local mailbox with bounded concurrency and optional per-aggregate ordering.
 * It is intentionally transport-neutral: a future distributed queue can
 * implement ServiceQueue without changing any domain service.
 */
export class InMemoryServiceQueue implements ServiceQueue {
  private active = 0
  private readonly pending: Array<() => void> = []
  private readonly partitionTails = new Map<string, Promise<void>>()

  constructor(private readonly maxConcurrency = 8) {
    if (!Number.isInteger(maxConcurrency) || maxConcurrency < 1) {
      throw new Error('Service queue maxConcurrency must be a positive integer.')
    }
  }

  submit<T>(item: ServiceQueueItem, process: () => Promise<T>): Promise<T> {
    if (item.execution.mode === 'inline') return process()

    if (item.execution.mode === 'ordered' && item.partitionKey) {
      const key = `${item.domain}:${item.partitionKey}`
      const previous = this.partitionTails.get(key) ?? Promise.resolve()
      const run = previous.catch(() => undefined).then(() => this.schedule(process))
      const tail = run.then(() => undefined, () => undefined)
      this.partitionTails.set(key, tail)
      void tail.finally(() => {
        if (this.partitionTails.get(key) === tail) this.partitionTails.delete(key)
      })
      return run
    }

    return this.schedule(process)
  }

  private schedule<T>(process: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const start = () => {
        this.active += 1
        void process()
          .then(resolve, reject)
          .finally(() => {
            this.active -= 1
            this.pending.shift()?.()
          })
      }

      if (this.active < this.maxConcurrency) start()
      else this.pending.push(start)
    })
  }
}
