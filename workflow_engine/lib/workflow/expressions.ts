import { createRequire } from 'node:module';

// Lazily load expr-eval so the engine core can be exercised without the
// package installed (tests inject a stub evaluator). Behavior is unchanged:
// the same expr-eval parser is used at runtime and any error still evaluates
// to `false` (silent-false), preserving the original contract.
const require = createRequire(import.meta.url);

let parser: any = null;

function getParser(): any {
  if (!parser) {
    const mod = require('expr-eval') as { Parser: new () => any };
    parser = new mod.Parser();
  }
  return parser;
}

/**
 * Safely evaluate a decision condition against process variables.
 * Supports expressions like: amount > 100000, status == "approved"
 */
export function evaluateCondition(
  expression: string,
  variables: Record<string, any>,
): boolean {
  try {
    const p = getParser();
    const expr = p.parse(expression);
    return Boolean(expr.evaluate(variables));
  } catch (err) {
    console.error('Expression evaluation failed:', expression, err);
    return false;
  }
}
