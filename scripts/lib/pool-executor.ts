// OPERATOR TOOLING — pooled QueryExecutor adapter, backed by ForgeDB.
//
// This used to create its own WebSocket Pool "for DEV load tooling", which made it
// a second place that decided its own connection and its own environment. It now
// adopts the application's single pool: the caller's url is mapped to the target
// ForgeDB already owns, and a url we do not own REFUSES rather than opening a
// private connection.
import { forgeDb, forgeDbTargetForUrl, type ForgeDbTarget } from '../../db/forge-db'
import type { QueryExecutor } from '../../db/query-executor'

export type PoolExecutor = {
  execute: QueryExecutor
  end: () => Promise<void>
}

export function createPoolExecutor(
  urlOrTarget: string,
  target?: ForgeDbTarget,
): PoolExecutor {
  const handle = forgeDb.forTarget(target ?? forgeDbTargetForUrl(urlOrTarget))
  return {
    execute: handle.sql as QueryExecutor,
    end: () => handle.end(),
  }
}
