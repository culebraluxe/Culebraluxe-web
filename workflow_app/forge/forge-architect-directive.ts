/**
 * ADD. Architect PRE instruction. Not a Lead prompt.
 *
 * Call site: Architect session builder (role-runner), after
 * forgeRoleNodePlan().evidenceInstruction, with this run's pinned baseRef
 * and the story's frozen assay commands.
 */
export function buildArchitectDirective(baseRef: string, frozenProofs: string[]): string {
  const proofs = frozenProofs.filter((c) => c.trim().length > 0)
  return [
    'ARCHITECT: survey the frozen story against the pinned baseRef. Do not implement. Do not choose SOLO/SMITH/SPLIT.',
    `baseRef=${baseRef || '<missing — HOLD if you cannot name the SHA you inspected>'}`,
    'For each finding emit: preconditions, scope (existing repo paths on baseRef, ≤3), postconditions, classes, risks, required, hint.',
    'hint is topology only: SAME_UNIT | SPLIT_CHILD | FOLLOW_UP_STORY | NOTE | HOLD. A required HOLD must name a concrete risk.',
    'Adjacent work is required:false. Do not stuff it into this story.',
    'An ACTIVE DECISION in your lane context is an INPUT TO THE CONTRACT, in force for this story: contradicting one is a HOLD or a learn item, never a prose overwrite.',
    'Do not invent a blob seam that does not exist on baseRef. A new file is not its own seam — declare the parent directory (`seamForNewFile`).',
    'NEW FILE? Declare its PARENT DIRECTORY as a scope entry (e.g. "workflow_app/tests/"). A seam must EXIST on baseRef, and a directory does — while a file that is not there yet does not. Without the directory the Lead has no legal surface to assign and the story HOLDs even though your plan is sound. This is the law in `seamForNewFile` (workflow_app/forge/forge-shaping.ts): the function wins if this sentence and it ever drift.',
    `Frozen proofs (do not invent commands): ${proofs.join(' | ') || '(none declared on the story)'}`,
    'End with exactly ONE un-fenced JSON line beginning FORGE_ARCHITECT_HANDOFF:. Do not fence it.',
    'Schema: {"version":1,"baseRef":"<the sha above>","findings":[{"id":"F1","required":true,"summary":"...","preconditions":["..."],"scope":["path/file.ts"],"postconditions":["..."],"classes":["Type"],"risks":["concrete"],"hint":"SAME_UNIT"]}',
  ].join('\n')
}
