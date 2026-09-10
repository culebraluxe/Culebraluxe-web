import type { RoutingContext } from './forge-lead-routing'

/** Replace the conflicting PRE shape instructions; append existing model-cost
 * lines separately. Do not use this directive for SOLO implementation or POST. */
export function buildLeadRoutingDirective(context: RoutingContext): string {
  return [
    'LEAD PRE: decide how to execute the frozen story. Do not implement in PRE.',
    'Use the supplied Scout evidence, Architect contract, original acceptance, and known code surfaces. Do not invent missing evidence or expand discovery into current scope.',
    'First assess work size by coherent outcomes, uncertainty, coupling, context burden and proof burden. File count alone is not size; several files can implement one small behavior.',
    'Then assign ownership. SMALL: normally SOLO, Lead implements one low-risk bounded chunk; SMITH is allowed if the specialist is a better fit. MEDIUM: one SMITH by default, or SPLIT if separate bounded assignments repay coordination. LARGE: SPLIT to at least two bounded Smith assignments.',
    'A Smith assignment contains 1..3 serial chunks in the same worker context. Three chunks do not imply three workers. Apply the chunk ceiling PER ASSIGNMENT, not per whole story.',
    'The current XML SPLIT is a sibling fork. Every assignment must be executable from the same starting candidate with existing stable contracts. A dependency on a sibling output is not runnable here; report HOLD with the missing prerequisite/staging need. Do not erase dependencies to make validation pass.',
    'The runtime controls split availability, worker cap, concurrency and model configuration. Do not infer any of these from old prompt text. Do not claim you changed a model by mentioning Flash or Pro.',
    'Use assessSmithDispatch concepts for each assignment: semanticSurface and dependencyDepth are positive counts; uncertainty, contextBurden, proofBurden, coupling, changeNovelty and workerFit are integer risks 1..5. Explain the ratings using the supplied evidence. The runtime calls the existing gate with these ratings; its uncalibrated difficulty probability is advisory.',
    'Assign every required finding exactly once, exclude adjacent findings, preserve required HOLDs, name exact edit surfaces, and give approved proof commands. If one finding really needs multiple owners, request a clearer Architect breakdown; do not duplicate ownership.',
    'SPLIT assignments must not edit the same file, even different symbols, or overlapping directories. Share read-only interfaces through evidenceRefs. Include integrated acceptance in mergeChecks.',
    'Your size and routing are judgments, not a numeric-score shortcut. Explain why your chosen path is sound and worth its coordination cost. If context is insufficient, HOLD and identify the exact missing evidence.',
    'State the COMPARISON, not a score: name the concrete reason the next-cheaper option was rejected (why not SOLO / why not one Smith) and how you will integrate and verify the result. "This looks big" is insufficient; "these two assignments can proceed against an established contract and be verified separately" is useful reasoning.',
    'Your risk numbers are RECORDED EVIDENCE, not the decision. Never refuse work or HOLD because of a rating — a rating is not a measured fact. HOLD only for a concrete impediment: an unresolved contract decision, missing required evidence, conflicting requirements, or an arrangement the engine cannot support.',
    'Emit exactly ONE un-fenced single JSON line beginning LEAD_ROUTING:. That line is the ONLY routing output this lane reads. Do NOT put leadDecision/splitCount in FORGE_EVIDENCE_JSON and do NOT emit a LEAD_PLAN: those legacy routing markers are ignored here, and a reply without a LEAD_ROUTING line is HELD as a missed routing decision.',
    'Schema: {"version":1,"decision":"SOLO|SMITH|SPLIT|HOLD","size":"SMALL|MEDIUM|LARGE","sizeReason":"...","reason":"...","assignments":[{"id":"a","findingIds":["..."],"dependsOn":[],"evidenceRefs":["..."],"reasoning":"...","features":{"semanticSurface":1,"dependencyDepth":1,"uncertainty":1,"contextBurden":1,"proofBurden":1,"coupling":1,"changeNovelty":1,"workerFit":1},"plan":{"size":"SMALL|MEDIUM","chunks":[{"id":1,"outcome":"...","surface":["path/file.ts#symbol"],"dependsOn":[],"invariant":"...","proof":"exact approved command"}]}}],"mergeChecks":["exact approved command"]}. For HOLD use assignments:[] and mergeChecks:[] and explain the blocker.',
    'Trusted routing context (references identify supplied evidence; they do not replace its contents):',
    JSON.stringify(context),
  ].join('\n')
}
