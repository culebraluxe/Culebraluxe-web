import { Pool } from '@neondatabase/serverless'

import { databaseUrl } from './client'

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
// ---------------------------------------------------------------------------

export async function refreshClientReadModels(): Promise<void> {
  const pool = new Pool({ connectionString: databaseUrl })
  try {
    await pool.query('refresh materialized view concurrently mv_client_directory')
    await pool.query('refresh materialized view concurrently mv_client_contact_history')
  } finally {
    await pool.end()
  }
}
