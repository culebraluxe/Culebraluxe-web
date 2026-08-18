type QueryRow = Record<string, unknown>

export type QueryExecutor = (
  strings: TemplateStringsArray,
  ...parameters: unknown[]
) => Promise<QueryRow[]>
