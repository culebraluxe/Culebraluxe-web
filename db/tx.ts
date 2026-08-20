import type { QueryExecutor } from './query-executor'
import { withTransaction } from '../lib/neon-interactive'

// Interactive transaction runner over the Neon database. Services that need
// conditional multi-statement logic (validate → mutate → record receipt) run
// inside this; tests inject a fake runner instead. The adapter is lazy (the
// Pool and connection URL are resolved on first use), so importing this module
// never requires a DATABASE_URL.
export type TxRunner = <T>(cb: (tx: QueryExecutor) => Promise<T>) => Promise<T>

export const neonTx: TxRunner = async (cb) => withTransaction(cb)
