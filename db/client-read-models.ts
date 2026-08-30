import { db, DbFailureError } from './database-gateway'

// ---------------------------------------------------------------------------
// CLIENTS — materialized application read-model refresh seam.
//
// The final cycle:
//   LOAD SOURCE -> L/ODS -> NORMALIZE -> RECONCILE -> PROMOTE / ENRICH
//   -> REFRESH MATERIALIZED CLIENT READ MODELS -> COMPLETE
//
// REFRESH ... CONCURRENTLY is used (both materialized views carry a unique
// index) so readers are never blocked. This seam is the SINGLE place the
// client read models are rebuilt; it is called at the ingestion/promotion
// boundaries and after the ClientManager's own canonical write actions — never
// by the web server on startup.
//
// DB-HARDEN-01: routes through the DatabaseGateway (no direct Neon/Pool
// import). A failure surfaces as a normalized DbFailureError so the caller
// never sees a false success.
// ---------------------------------------------------------------------------

export async function refreshClientReadModels(): Promise<void> {
  // Dependency-safe order: the source-grain relationship MV feeds the Client
  // directory freshness, so it must refresh FIRST.
  const r1 = await db.execute`refresh materialized view concurrently mv_client_relationship_channels`
  if (!r1.ok) throw new DbFailureError(r1.error)
  const r2 = await db.execute`refresh materialized view concurrently mv_client_directory`
  if (!r2.ok) throw new DbFailureError(r2.error)
  const r3 = await db.execute`refresh materialized view concurrently mv_client_contact_history`
  if (!r3.ok) throw new DbFailureError(r3.error)
}
