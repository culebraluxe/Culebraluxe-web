// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// OPERATOR TOOLING — pooled QueryExecutor adapter, backed by ForgeDB.
//
// This used to create its own WebSocket Pool "for DEV load tooling", which made it
// a second place that decided its own connection and its own environment. It now
// adopts the application's single pool: the caller's url is mapped to the target
// ForgeDB already owns, and a url we do not own REFUSES rather than opening a
// private connection.
import { forgeDb, forgeDbTargetForUrl, type ForgeDbTarget } from '@/legacy/db/forge-db'
import type { QueryExecutor } from '@/legacy/db/query-executor'

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
