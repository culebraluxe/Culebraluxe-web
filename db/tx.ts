import type { QueryExecutor } from './query-executor'

// Interactive transaction runner over the Neon client. Services that need
// conditional multi-statement logic (validate → mutate → record receipt) run
// inside this; tests inject a fake runner instead. The client is imported
// lazily so tests (and any caller not using the real runner) never trigger
// db/client's module-load URL requirement.
export type TxRunner = <T>(cb: (tx: QueryExecutor) => Promise<T>) => Promise<T>

export const neonTx: TxRunner = async (cb) => {
  const { sql } = await import('./client')
  return (sql as any).transaction(async (tx: any) => cb(tx as QueryExecutor))
}
