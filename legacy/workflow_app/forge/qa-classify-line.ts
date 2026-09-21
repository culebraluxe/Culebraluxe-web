/**
 * ADD. Post-FAIL line parser for the failure_classifier engine node.
 *
 * Does NOT replace workflow_app/forge/failure-classifier.ts
 * (classifyFailure / routeFailure — different taxonomy).
 *
 * This file feeds ForgeGateEvidence.failureClass (CODE_DEFECT…).
 * Call after Assay has already set qaPassed=false. Never flip the verdict.
 */
export const ENGINE_FAILURE_CLASSES = [
  'CODE_DEFECT',
  'TEST_DEFECT',
  'ARCHITECTURE_GAP',
  'REQUIREMENTS_GAP',
  'UNKNOWN_CAUSE',
  'ENVIRONMENT',
  'MIGRATION',
  'PUBLISH_CONFLICT',
  'DEPLOYMENT',
  'PRODUCTION_SMOKE',
  'HOLD',
] as const

export type EngineFailureClass = (typeof ENGINE_FAILURE_CLASSES)[number]

export function parseFailureClass(notes: string | null | undefined): EngineFailureClass | null {
  if (!notes) return null
  const line = notes.split(/\r?\n/).map((s) => s.trim()).find((s) => s.startsWith('FAILURE_CLASS:'))
  if (!line) return null
  const value = line.slice('FAILURE_CLASS:'.length).trim().toUpperCase()
  return (ENGINE_FAILURE_CLASSES as readonly string[]).includes(value)
    ? (value as EngineFailureClass)
    : null
}

export function attachFailureClass<T extends { verdict?: string; failureClass?: string | null }>(
  report: T,
  notes: string | null,
): T {
  if (report.verdict && report.verdict !== 'FAIL') return { ...report, failureClass: null }
  return { ...report, failureClass: parseFailureClass(notes) }
}

export function buildClassifyDirective(input: {
  evaluatedSha: string
  blockers: string[]
  excerpts: string[]
}): string {
  return [
    'Assay already FAILED. Do not re-run tests. Do not change the verdict.',
    `evaluatedSha=${input.evaluatedSha}`,
    `blockers=${input.blockers.join(' | ')}`,
    `command excerpts: ${input.excerpts.join(' || ')}`,
    `Pick one FAILURE_CLASS: ${ENGINE_FAILURE_CLASSES.join(' | ')}`,
    'End with one line: FAILURE_CLASS: CODE_DEFECT',
    'That line becomes evidence.failureClass. Routing stays on the engine XML + qa-repair-policy, not on failure-classifier.ts.',
  ].join('\n')
}
