// ---------------------------------------------------------------------------
// The SQL template layer — ONE definition of what a template becomes.
//
// Moved out of db/database-gateway.ts when ForgeDB (the pooled wrapper) needed
// the same conversion. It has to be SHARED rather than copied: a fragment built
// by one module's `raw()` and interpolated into another module's `sql` would
// otherwise be bound as a parameter instead of becoming SQL. That class of bug
// is silent — the query still runs, just wrong.
//
// Two outputs, because two callers legitimately differ:
//   * flattenSqlTemplate() keeps the template shape (the gateway's executor seam
//     and neon-interactive's thenable both consume it), and
//   * toPgQuery() renders the final `$n` statement for `pg`, with every value
//     passed as a bind parameter and never interpolated into text.
// ---------------------------------------------------------------------------

export const SQL_FRAGMENT = Symbol('culebraluxe.sql-fragment')

export type SqlFragment = {
  readonly [SQL_FRAGMENT]: true
  readonly strings: readonly string[]
  readonly values: readonly unknown[]
}

/**
 * Flattening policy. The symbol-tagged fragment is the canonical encoding
 * (`raw()` produces it). One caller legitimately needs the looser legacy shape:
 * `lib/neon-interactive.ts` historically treated ANY `{ strings, values }` object
 * as a fragment, and `workflow_app/tests/neon-interactive.test.ts` pins that
 * contract, so it opts in explicitly rather than having it be silently dropped.
 *
 * Off by default: inlining SQL for any object carrying those two array fields
 * would turn a legitimate bind value into SQL text.
 */
export type FlattenOptions = {
  acceptStructuralFragments?: boolean
}

export function isSqlFragment(
  value: unknown,
  options?: FlattenOptions,
): value is SqlFragment {
  if (typeof value !== 'object' || value === null) return false
  if ((value as SqlFragment)[SQL_FRAGMENT] === true) return true
  if (!options?.acceptStructuralFragments) return false
  const candidate = value as { strings?: unknown; values?: unknown }
  return Array.isArray(candidate.strings) && Array.isArray(candidate.values)
}

export function toTemplateStrings(parts: string[]): TemplateStringsArray {
  const template = [...parts] as unknown as TemplateStringsArray
  Object.defineProperty(template, 'raw', { value: [...parts] })
  return template
}

/**
 * Recursively compose structural fragments into the parent template. Fragment
 * SQL becomes part of the template strings; fragment values remain ordinary
 * positional bind parameters.
 */
export function flattenSqlTemplate(
  strings: readonly string[],
  values: readonly unknown[],
  options?: FlattenOptions,
): { strings: TemplateStringsArray; values: unknown[] } {
  const flattenedStrings = [strings[0] ?? '']
  const flattenedValues: unknown[] = []

  for (let index = 0; index < values.length; index++) {
    const value = values[index]
    const following = strings[index + 1] ?? ''

    if (!isSqlFragment(value, options)) {
      flattenedValues.push(value)
      flattenedStrings.push(following)
      continue
    }

    const nested = flattenSqlTemplate(value.strings, value.values, options)
    flattenedStrings[flattenedStrings.length - 1] += nested.strings[0] ?? ''

    for (let nestedIndex = 0; nestedIndex < nested.values.length; nestedIndex++) {
      flattenedValues.push(nested.values[nestedIndex])
      flattenedStrings.push(nested.strings[nestedIndex + 1] ?? '')
    }

    flattenedStrings[flattenedStrings.length - 1] += following
  }

  return {
    strings: toTemplateStrings(flattenedStrings),
    values: flattenedValues,
  }
}

/**
 * Render a template as a parameterized statement for `pg`: `$1, $2, ...` with the
 * values carried separately. Values are NEVER concatenated into the SQL text —
 * the placeholder-rewrite is the whole reason this function exists rather than
 * string interpolation at the call site.
 */
export function toPgQuery(
  strings: readonly string[],
  values: readonly unknown[],
): { text: string; values: unknown[] } {
  let text = strings[0] ?? ''
  for (let index = 0; index < values.length; index++) {
    text += `$${index + 1}` + (strings[index + 1] ?? '')
  }
  return { text, values: [...values] }
}

/**
 * Structural-SQL fragment builder. Produces a fragment for CONSTANT,
 * non-user-controlled SQL (e.g. a fixed ORDER BY clause) so it can be
 * interpolated into a `sql` template. The fragment's own bind values (if any) are
 * still parameterized. NEVER pass user input here.
 *
 * Building a fragment is pure and MUST NOT require a live connection: modules
 * define constant fragments at module scope, so resolving a pool here would make
 * importing them depend on a configured database.
 */
export function raw(strings: TemplateStringsArray, ...values: unknown[]): SqlFragment {
  return { [SQL_FRAGMENT]: true, strings, values }
}
