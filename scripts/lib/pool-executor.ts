// REL-INTEL — WebSocket Pool-backed QueryExecutor adapter for DEV load tooling.
// The app's default `sql` executor (db/client) is the Neon HTTP driver, which is
// slow for thousands of sequential upserts/lookups. These dev-only load scripts
// use a persistent pooled connection instead. Equivalent tagged-template SQL;
// never used by app runtime code.
import { Pool } from '@neondatabase/serverless'
import type { QueryExecutor } from '../../db/query-executor'

export function createPoolExecutor(url: string): { execute: QueryExecutor; end: () => Promise<void> } {
  const pool = new Pool({ connectionString: url })
  const execute: QueryExecutor = (strings, ...params) => {
    let text = strings[0]
    for (let i = 0; i < params.length; i++) {
      text += `$${i + 1}${strings[i + 1] ?? ''}`
    }
    return pool.query(text, params).then((r) => r.rows)
  }
  return { execute, end: () => pool.end() }
}
