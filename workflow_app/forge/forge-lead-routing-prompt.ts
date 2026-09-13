import type { RoutingContext } from './forge-lead-routing'

function capabilityRules(context: RoutingContext): string {
  if (!context.splitEnabled || context.maxSmiths < 2) {
    return [
      'RUNTIME CAP: SPLIT is NOT available this run (splitEnabled=false or maxSmiths<2).',
      'Do not propose decision=SPLIT. Do not invent workers.',
      'SMALL → SOLO or one SMITH. MEDIUM → one SMITH (1..3 serial chunks in the same worker).',
      'LARGE → HOLD and recut the story (or honestly recast as one MEDIUM assignment if the work is actually one coherent unit). A 4th serial chunk is still a HOLD, not a fake split.',
    ].join(' ')
  }
  return [
    `RUNTIME CAP: SPLIT is available. maxSmiths=${context.maxSmiths}.`,
    'SMALL: normally SOLO; SMITH if the specialist is a better fit. Do not SPLIT SMALL work.',
    'MEDIUM: one SMITH by default, or SPLIT if separate bounded assignments repay coordination.',
    'LARGE: SPLIT to 2..maxSmiths bounded Smith assignments that do not share write surfaces.',
  ].join(' ')
}

function benchRule(context: RoutingContext): string {
  if (!context.benchIntent) return ''
  if (context.benchIntent === 'HOLD') return 'BENCH CAP: HOLD. You may only HOLD.'
  if (context.benchIntent === 'SOLO') return 'BENCH CAP: SOLO. You may SOLO or HOLD, not SMITH/SPLIT.'
  if (context.benchIntent === 'SMITH') return 'BENCH CAP: SMITH. You may SMITH, SOLO, or HOLD — not SPLIT.'
  return 'BENCH CAP: SPLIT allowed if the runtime cap allows it. You may still HOLD.'
}

export function buildLeadRoutingDirective(context: RoutingContext): string {
  const bench = benchRule(context)
  return [
    'LEAD PRE: decide how to execute the frozen story. Do not implement in PRE.',
    'Use the supplied Scout evidence, Architect contract, original acceptance, and known code surfaces. Do not invent missing evidence or expand discovery into current scope.',
    'First assess work size by coherent outcomes, uncertainty, coupling, context burden and proof burden. File count alone is not size; several files can implement one small behavior.',
    capabilityRules(context),
    bench,
    'A Smith assignment contains 1..3 serial chunks in the same worker context. Three chunks do not imply three workers. Apply the chunk ceiling PER ASSIGNMENT, not per whole story.',
    'The current XML SPLIT is a sibling fork. Every assignment must be executable from the same starting candidate with existing stable contracts. A dependency on a sibling output is not runnable here; report HOLD with the missing prerequisite/staging need. Do not erase dependencies to make validation pass.',
    'The runtime controls split availability, worker cap, concurrency and model configuration. Do not infer any of these from old prompt text. Do not claim you changed a model by mentioning Flash or Pro.',
    'Use assessSmithDispatch concepts for each assignment: semanticSurface and dependencyDepth are positive counts; uncertainty, contextBurden, proofBurden, coupling, changeNovelty and workerFit are integer risks 1..5. Explain the ratings using the supplied evidence. The runtime calls the existing gate with these ratings; its uncalibrated difficulty probability is advisory.',
    'Assign every required finding exactly once, exclude adjacent findings, preserve required HOLDs, name exact edit surfaces, and give approved proof commands. If one finding really needs multiple owners, request a clearer Architect breakdown; do not duplicate ownership.',
    'HARD SCOPE RULE: every chunk surface/scope path MUST be inside the Architect seams of the findings that assignment claims — a path is legal only when it IS a seam or is NESTED UNDER a seam directory. Do not invent a new file, a new directory, or an adjacent file the Architect did not name; the validator refuses the ENTIRE routing and the story HOLDs. If the work genuinely needs a path the Architect did not declare, HOLD and name the missing seam instead of guessing one. Quote the Architect seam paths verbatim from the handoff.',
    'When SPLIT is available, assignments must not edit the same file, even different symbols, or overlapping directories. Share read-only interfaces through evidenceRefs. Include integrated acceptance in mergeChecks.',
    'Your size and routing are judgments, not a numeric-score shortcut. Explain why your chosen path is sound and worth its coordination cost. If context is insufficient, HOLD and identify the exact missing evidence.',
    'State the COMPARISON, not a score: name the concrete reason the next-cheaper option was rejected (why not SOLO / why not one Smith) and how you will integrate and verify the result. "This looks big" is insufficient; "these two assignments can proceed against an established contract and be verified separately" is useful reasoning.',
    'Your risk numbers are RECORDED EVIDENCE, not the decision. Never refuse work or HOLD because of a rating — a rating is not a measured fact. HOLD only for a concrete impediment: an unresolved contract decision, missing required evidence, conflicting requirements, or an arrangement the engine cannot support.',
    'RECORD THE DECISION IN FIELDS. Do not emit LEAD_ROUTING, LEAD_PLAN, or FORGE_EVIDENCE_JSON routing keys. Chat JSON is scrap. Run forge-handoff.mjs with the identity from your task line:',
    'node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node lead_pre --attempt <attempt> --decision SOLO|SMITH|SPLIT|HOLD --size SMALL|MEDIUM|LARGE --size-reason "<why this size>" --reason "<why this route>" --assignments <n> --findings <ids> --merge-checks "<exact frozen command>" --scope <paths>',
    'Then one row per chunk: ... --chunk <1..3> --assignment <id> --surface <path[,path]> --proof "<exact frozen command>" --invariant "<what must still hold>" --finding <id> --evidence <ref> --size SMALL|MEDIUM plus the eight dispatchability integers.',
    'The database validates those fields and the engine reads them. A rejected write names the failing field: fix it and run the command again. An unwritten decision is a HOLD. For HOLD write decision=HOLD and do not write chunks.',
    'Trusted routing context (references identify supplied evidence; they do not replace its contents):',
    JSON.stringify(context),
  ].filter(Boolean).join('\n')
}
