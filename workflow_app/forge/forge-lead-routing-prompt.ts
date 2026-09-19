import type { RoutingContext } from './forge-lead-routing'
import type { FindingHandoffSummary } from './lead-routing-context'

/**
 * The routing context the runner hands in carries the findings handoff, which lives beside the
 * Lead context rather than in the frozen `RoutingContext` shape. Widening the parameter here
 * keeps the frozen routing type untouched while the directive can still state the attempt in
 * force and name a dropped seam.
 */
type LeadRoutingDirectiveContext = RoutingContext & {
  findingHandoff?: FindingHandoffSummary | null
}

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

/**
 * THE DIRECT-TO-QA OPTION, OFFERED ONLY WHEN IT IS REAL (work package A).
 *
 * ASSAY means "the work this story asks for already exists on the base; judge it instead of authoring it".
 * The Lead cannot know that from the story text alone: it is a fact about the REPOSITORY, and the only
 * trustworthy source is the runner's own observation, which arrives in the context as `gitObservedCandidate`.
 * So the rule is stated in two directions and both are load-bearing:
 *
 *   * with an observed candidate, the option is named WITH the sha, so the decision can be recorded exactly;
 *   * with no observation, the option is refused outright, because a Lead that invents a sha would produce a
 *     route the validator must then refuse — after a whole turn was spent on it.
 *
 * This is the missing half of the earlier wiring: the validator, the arrangement and the workflow branch all
 * existed, and no directive ever told a Lead the option existed.
 */
function assayRouteRule(context: RoutingContext): string {
  const observed = context.gitObservedCandidate
  if (!observed?.sha) {
    return (
      'DIRECT-TO-QA (ASSAY) IS NOT AVAILABLE THIS RUN: no candidate observed on the base, so there is nothing ' +
      'already-existing to judge. Do not propose decision=ASSAY and do not name a candidate — an ASSAY route ' +
      'whose candidate the runner never observed is refused.'
    )
  }
  if (!observed.onBaseRef) {
    return (
      `DIRECT-TO-QA (ASSAY) IS NOT AVAILABLE THIS RUN: a candidate was seen (${observed.sha}) but it is NOT on the ` +
      'story base, so verifying it would verify work the repository does not have. Do not propose decision=ASSAY.'
    )
  }
  return [
    `DIRECT-TO-QA (ASSAY) IS AVAILABLE: the runner observed candidate ${observed.sha} ON the story base.`,
    'If — and only if — the required findings are ALREADY satisfied by that candidate, you may route decision=ASSAY instead of authoring the work again. ASSAY dispatches no Smith and writes NO chunks.',
    `Record it as: --decision ASSAY --verify-candidate ${observed.sha} --merge-checks "<the frozen story proof commands that judge it>" — plus --size/--size-reason/--reason as usual.`,
    'The candidate must be that exact observed sha: any other value is refused, and no chunks may be written on an ASSAY route.',
  ].join(' ')
}

function benchRule(context: RoutingContext): string {
  if (!context.benchIntent) return ''
  if (context.benchIntent === 'HOLD') return 'BENCH CAP: HOLD. You may only HOLD.'
  if (context.benchIntent === 'SOLO') return 'BENCH CAP: SOLO. You may SOLO or HOLD, not SMITH/SPLIT.'
  if (context.benchIntent === 'SMITH') return 'BENCH CAP: SMITH. You may SMITH, SOLO, or HOLD — not SPLIT.'
  return 'BENCH CAP: SPLIT allowed if the runtime cap allows it. You may still HOLD.'
}

/**
 * THE ATTEMPT IN FORCE AND WHAT IT LOST, STATED OUT LOUD.
 *
 * The findings rows are scoped to the newest attempt, so a later attempt that dropped a seam the
 * earlier one declared used to be invisible here. This names the attempt in force and every
 * explicitly superseded seam with the attempt that declared it, so a HOLD blames the right
 * attempt. No supersede renders nothing — never an empty string that reads like "no problem".
 */
function findingHandoffRule(context: LeadRoutingDirectiveContext): string {
  const handoff = context.findingHandoff
  if (!handoff) {
    return 'ARCHITECT FINDINGS: read from the legacy reply parser; no attempt is recorded, so do not treat any attempt number as in force.'
  }
  const lines = [
    handoff.attemptInForce == null
      ? 'ARCHITECT FINDINGS: no attempt number was recorded, so no attempt is in force.'
      : `ARCHITECT FINDINGS: attempt ${handoff.attemptInForce} is in force.`,
  ]
  for (const lost of handoff.superseded) {
    lines.push(
      `DROPPED SEAM: ${lost.seam} was declared by attempt ${lost.declaredByAttempt} ` +
        `(finding ${lost.declaredByFindingId}) and is not in the attempt in force; the later attempt explicitly superseded it.`,
    )
  }
  // WHICH TASK A RE-RUN RETIRED. The findings read keeps only the newest task per node, so the
  // operator learns here which task set it replaced — an operator-legible record, never a manual
  // PROD cleanup as the only remedy. An empty list renders nothing.
  for (const retired of handoff.retiredTasks ?? []) {
    lines.push(
      `RETIRED FINDINGS TASK: task ${retired.taskId} for node ${retired.nodeId} was superseded by ` +
        `task ${retired.supersededByTaskId} (a re-run of the same node); its findings are not in force.`,
    )
  }
  return lines.join('\n')
}

export function buildLeadRoutingDirective(context: LeadRoutingDirectiveContext): string {
  const bench = benchRule(context)
  return [
    'LEAD PRE: decide how to execute the frozen story. Do not implement in PRE.',
    'Use the supplied Scout evidence, Architect contract, original acceptance, and known code surfaces. Do not invent missing evidence or expand discovery into current scope.',
    'First assess work size by coherent outcomes, uncertainty, coupling, context burden and proof burden. File count alone is not size; several files can implement one small behavior.',
    capabilityRules(context),
    bench,
    assayRouteRule(context),
    findingHandoffRule(context),
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
    'node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node lead_pre --attempt <attempt> --decision SOLO|SMITH|SPLIT|HOLD|ASSAY --size SMALL|MEDIUM|LARGE --size-reason "<why this size>" --reason "<why this route>" --assignments <n> --findings <ids> --merge-checks "<exact frozen command>" --scope <paths>',
    'DECLARE THE ACCEPTANCE-TO-ASSERTION MAPPING when you route (it is frozen before the work and QA consumes it; it is NEVER authored by the lane that does the work). On the SAME contract command add one flag per clause: --acceptance-assertion "<acceptance clause quoted verbatim>=<assertion ref in the frozen proof>". The clause must be one of the story\'s own acceptance clauses and the ref must exist in the frozen proof; a malformed or unmatched entry is refused by name and the whole contract write is refused. The handoff-declared mapping beats any mapping declared on the story row, and the record names which source was used.',
    'ASSERTION REF FORMAT: write the ref as a bare `<name>`, or file-qualified `path#name` when the assertion must come from a specific file. The name is matched EXACTLY, and only on a pass/fail marker line the proof prints — a name echoed in a header, a summary or source text is not evidence. A bare name is accepted only while it is UNAMBIGUOUS in the executed output (one origin, one verdict). A file-qualified ref is answered ONLY by the file it names: the reader takes the originating file from the runner’s structured output (the JUnit reporter names the file for every test, passing included; TAP names it in a failure’s `location:` field), so a proof invoked with a reporter that prints no file provenance leaves a qualified ref UNPROVEN, and an assertion that ran in a DIFFERENT file never satisfies it. When you qualify a ref, the frozen proof must be able to name the file: prefer a bare ref otherwise.',
    'Then one row per chunk. The chunk command ALSO writes that assignment row, so it carries the eight dispatchability integers — every flag spelled out, because a chunk without them cannot be routed:',
    'node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node lead_pre --attempt <attempt> --chunk <1..3> --assignment <id> --surface <path[,path]> --proof "<exact frozen command>" --invariant "<what must still hold>" --finding <id> --evidence <ref> --size SMALL|MEDIUM --reasoning "<why this chunk is the cheapest sound shape>" --semantic-surface <1..100> --dependency-depth <1..100> --uncertainty <1..5> --context-burden <1..5> --proof-burden <1..5> --coupling <1..5> --change-novelty <1..5> --worker-fit <1..5>',
    'The chunk command refuses a row without --reasoning, --surface and --proof: the reader treats an assignment with no reasoning as no assignment at all, so a missing field costs the whole route rather than one column.',
    'Ranges: semantic-surface and dependency-depth are 1..100; the other six are 1..5. The database refuses a value outside its range, and the routing validator then refuses the route.',
    'SCALE DIRECTION — EVERY ONE OF THE EIGHT RISES WITH DIFFICULTY. 1 is always the lowest burden, risk or uncertainty; higher is always worse. worker-fit is how much the work does NOT fit one worker in one context (1 = one worker is plainly enough, 5 = it needs a team), so a high number is not praise: writing 5 because "a worker fits this well" is the exact inversion that HOLDs a perfectly good story.',
    'FOR SOLO THE VECTOR IS NOT NEGOTIABLE: semantic-surface=1, dependency-depth=1, and uncertainty, context-burden, proof-burden, coupling, change-novelty and worker-fit at 2 or less. Work that cannot honestly be written that way is not SOLO — send it to a Smith or HOLD it. A missing number is not a low number, and an omitted vector is exactly what makes a SOLO unroutable.',
    'The database validates those fields and the engine reads them. A rejected write names the failing field: fix it and run the command again. An unwritten decision is a HOLD. For HOLD write decision=HOLD and do not write chunks.',
    'Trusted routing context (references identify supplied evidence; they do not replace its contents):',
    JSON.stringify(context),
  ].filter(Boolean).join('\n')
}
