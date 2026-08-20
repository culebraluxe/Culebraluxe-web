// ---------------------------------------------------------------------------
// Dependency-free, bounded condition DSL for decision nodes.
//
// Grammar (Story 182/184 — exactly what current definitions use, plus the
// minimal equality/inequality forms):
//
//   expression := identifier WS op WS literal
//   identifier := [A-Za-z_][A-Za-z0-9_]*
//   op         := '==' | '!='
//   literal    := 'true' | 'false' | 'null'
//               | number (-?\d+(\.\d+)?)
//               | '"' string '"' | "'" string "'"
//
// The COMPLETE string must match. Anything else (e.g. '&&', '||', '!',
// function calls, '===', '>', '<', trailing garbage) is unsupported and is
// rejected — never silently evaluated to a branch.
//
// Semantics (Story 183/185):
//   - a supported expression evaluates deterministically with strict
//     equality/inequality; missing facts stay `undefined` and never collapse
//     into `null` (so `identifier == null` is true only when the fact is
//     explicitly null)
//   - an unsupported or malformed expression raises an explicit generic error
//     at runtime (the deployment validator should have rejected it earlier)
// ---------------------------------------------------------------------------

export class ExpressionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ExpressionError'
  }
}

const EXPRESSION_PATTERN =
  /^([A-Za-z_][A-Za-z0-9_]*)\s*(==|!=)\s*(true|false|null|-?\d+(?:\.\d+)?|"[^"\n]*"|'[^'\n]*')$/

/** True when `expression` conforms to the supported condition DSL. */
export function isSupportedExpression(expression: string): boolean {
  return EXPRESSION_PATTERN.test(expression.trim())
}

/**
 * Evaluate a decision condition against process variables.
 * Throws `ExpressionError` for any unsupported/malformed expression.
 */
export function evaluateCondition(
  expression: string,
  variables: Record<string, any>,
): boolean {
  const m = EXPRESSION_PATTERN.exec(expression.trim())
  if (!m) {
    throw new ExpressionError(`Unsupported workflow expression: ${JSON.stringify(expression)}`)
  }

  const [, name, op, literal] = m
  const lhs = variables[name]
  const rhs = parseLiteral(literal)

  switch (op) {
    case '==':
      return lhs === rhs
    case '!=':
      return lhs !== rhs
    default:
      throw new ExpressionError(`Unsupported workflow operator in: ${JSON.stringify(expression)}`)
  }
}

function parseLiteral(s: string): any {
  if (s === 'true') return true
  if (s === 'false') return false
  if (s === 'null') return null
  if (
    (s.startsWith('"') && s.endsWith('"')) ||
    (s.startsWith("'") && s.endsWith("'"))
  ) {
    return s.slice(1, -1)
  }
  return Number(s)
}
