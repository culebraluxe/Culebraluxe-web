// DB-HARDEN-01 — the application database facade.
//
// Everything routes through the single DatabaseGateway. Importing this module
// never throws and never loads the Neon driver — the connection is resolved
// lazily on first database use. Repositories import `sql` (a contained tagged
// executor that returns rows and throws a NORMALIZED DbFailureError on
// failure) or use `db.query`/`db.queryOne`/`db.transaction` for typed Results.

export { db, sql, raw, getDatabaseUrl, resolveDbTarget, dbTargetInfo, DatabaseGateway } from './database-gateway'
export {
  DbConfigError,
  DbFailureError,
  setDatabaseTestExecutor,
  setDatabaseTestTransaction,
} from './database-gateway'
export type {
  DbFailure,
  DbFailureKind,
  Result,
} from './database-gateway'
