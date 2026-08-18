export type QueryRow = Record<string, unknown>

export type QueryExecutor = (
  strings: TemplateStringsArray,
  ...parameters: unknown[]
) => Promise<QueryRow[]>

export type TransactionExecutor = (
  buildQueries: (execute: QueryExecutor) => Array<Promise<QueryRow[]>>,
) => Promise<QueryRow[][]>
