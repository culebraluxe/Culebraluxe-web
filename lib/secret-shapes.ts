// ---------------------------------------------------------------------------
// ONE CREDENTIAL-SHAPE CATALOG (ENG-FORGE-LANE-SECRET-GATE-01).
//
// The harness linter (`scripts/forge-packet-lint.ts`) and the candidate publish gate both ask
// "does this line look like a credential?". Two lists answering that question would be two
// sources for one fact, so the shapes live here and both readers import them.
//
// A match is a REFUSAL, so a false positive blocks a real publish. Keep the patterns narrow and
// line-oriented, and prove any change with the fence (`legacy/workflow_app/tests/lane-secret-gate.test.ts`).
// ---------------------------------------------------------------------------

export type SecretShape = {
  /** Stable rule name, printed in a refusal and asserted by the fence. */
  name: string
  /** Line-oriented detector. Must not carry the `g` flag (lastIndex is stateful). */
  pattern: RegExp
}

export const SECRET_SHAPES: ReadonlyArray<SecretShape> = [
  { name: 'openai-style key', pattern: /\bsk-[A-Za-z0-9]{16,}\b/ },
  { name: 'github token', pattern: /\bghp_[A-Za-z0-9]{20,}\b/ },
  { name: 'aws access key id', pattern: /\bAKIA[0-9A-Z]{12,}\b/ },
  { name: 'database url with credentials', pattern: /postgres(?:ql)?:\/\/[^\s'"]+:[^\s'"]+@/i },
]

/** Every shape this line matches. Empty means the line carries no known credential shape. */
export function secretShapesInLine(line: string): SecretShape[] {
  return SECRET_SHAPES.filter((shape) => shape.pattern.test(line))
}
