# CulebraLuxe LEAD routing — code handoff

Based on GitHub `culebraluxe/Culebraluxe-web` at `f7413d858b41a4e0dc175027fda02f6985ca9c1c`, inspected September 10, 2026.

Status: tested reference implementation and integration instructions. This has not been installed in the repository, connected to Neon, or exercised through a live Forge run. It is not an automatically applicable patch. Reconcile against the receiving checkout before integrating.

## Decision ownership

LEAD assesses the work and proposes a route. The runtime validates that proposal and returns specific corrections through the existing bounded reprompt loop. It never silently changes the route. The XML engine continues to own execution.

| Work size | Allowed delivery shape |
|---|---|
| SMALL | Normally SOLO: Lead implements. One SMITH is also allowed when justified by worker fit. |
| MEDIUM | One SMITH, or SPLIT into bounded Smith assignments. |
| LARGE | SPLIT into at least two bounded Smith assignments. |

Size is a reasoned judgment about behavior, coupling, uncertainty, context and proof burden. It is not derived from the number of files. The reference SOLO policy permits one chunk, one semantic responsibility, one dependency level and risk ratings at most 2/5. These are explicit initial policy choices, not empirically calibrated thresholds. LEAD's ratings remain assertions that require grounding and later comparison with actual outcomes.

An assignment has up to three serial chunks. Two assignments with two chunks each are valid: the three-chunk ceiling applies to each worker, not the whole project.

## Fit with the tools already present

| Existing component | Role in this implementation |
|---|---|
| Scout research + Architect brief + findings | Ground size, ownership, prerequisites and scope. Supply their actual contents as well as named references. |
| `buildGroundingDirective()` | Retain for PRE. Missing context goes back for evidence; LEAD does not start broad repository searches. |
| `assessSmithDispatch()` / KRAKEN | Called once for every proposed assignment, with LEAD's qualitative feature vector. This also validates chunk structure. |
| `dispatchabilityFor()` | Reused through KRAKEN; bounds an individual worker's responsibilities, dependencies and risks. |
| `scorePlan()` / `LogisticScorer` | Reused through KRAKEN. Low predicted success remains advisory, as in the existing code. |
| `estimateWork()` / `ratesFromActuals()` | Available for optional runtime-supplied cost/time comparisons. Not called by this reference and not a hard routing threshold. Prefer comparable actuals; label seed forecasts as estimates. |
| `renderModelCostLines()` | Preserve the existing cost context. A sentence requesting Pro does not reconfigure the actual model; runtime configuration remains authoritative. |
| Existing bounded self-heal loop | Receives validation errors and lets LEAD revise its proposal. Exhaustion retains the existing stop/release behavior. |
| `renderSmithWorkOrders()` | Receives exactly the accepted assignment's `plan`, including for SOLO implementation. |
| XML `split_dispatch` / `splitPlan` / `splitCount` | Receives the accepted assignment array, unchanged and durably bound to this execution. |
| LEAD POST + QA | Integrate candidates and verify frozen acceptance against the final candidate. PRE validation is not proof that tests passed. |

These are existing TypeScript utilities and supplied context, not automatically exposed model-callable tools. The reference calls KRAKEN in the runtime after LEAD emits its proposal. It does not claim that LEAD can invoke unregistered functions or access unavailable telemetry.

## Current scheduling boundary

The inspected v4 XML implements a sibling fork, not a dependency-wave scheduler. A large proposal with an unfinished prerequisite between Smith assignments returns a correction/HOLD instead of dispatching them together. Resolve the prerequisite or add and verify engine-owned staged scheduling separately. Setting concurrency to 1 is not sufficient: serial execution of isolated workspaces does not by itself propagate predecessor output.

The reference intentionally treats different symbols in the same file as a concurrent write conflict. A future symbol-aware merge policy can relax that after proving safe integration. File-path validation here bounds assignment areas; worker-side symbol restrictions and actual changed-file enforcement remain necessary.

## Runtime integration — all required before activation

1. Add the two implementation files below and the targeted test file. They reuse the existing KRAKEN dependency chain; no external package or second scheduler is introduced.

2. Hydrate `RoutingContext` in trusted runtime code from the exact current story/task handoff. Supply current required findings, named references to the actual Scout/Architect material, frozen proof commands, and runtime split capability/max workers. Do not let model output set those values. If context is incomplete, obtain it before evaluating execution. New targeted tests are valid when their commands belong to the frozen acceptance plan; this does not require the test file to exist before implementation. Never obtain commands by harvesting arbitrary model prose.

3. Replace the contradictory PRE routing paragraphs in `agent-runtime/lead-decision.ts` and the PRE marker instructions in `workflow_app/forge/forge-role-mapping.ts` with `buildLeadRoutingDirective(context)`. Keep phase identity, Architect authority and model-cost context. Emit one `LEAD_ROUTING` object; do not ask for three competing routing markers. Do not replace the implement or POST instructions.

4. In `agent-runtime-role-runner.ts`, review the raw LEAD PRE output after findings are marshalled and before any engine completion. Use the call sequence below. Add errors to the existing bounded self-heal path. Perform this routing validation even when optional deliverable enforcement is disabled. It is the prerequisite for handing off execution, not an optional formatting check.

```ts
import {
  parseLeadRouting,
  reviewLeadProposal,
  leadRoutingFacts,
} from './forge-lead-routing'

// raw is the existing rawRoleOutput(notes, testsSummary).
// routingContext is trusted, prehydrated context for THIS task/generation.
const routingReview = nodeId === 'lead_pre'
  ? reviewLeadProposal(parseLeadRouting(raw), routingContext)
  : null

// At the existing `miss` collection / bounded self-heal seam:
if (routingReview && !routingReview.ok) {
  miss.push(...routingReview.errors.map(reason => `lead-routing:${reason}`))
}

// Only after the review succeeds, derive engine routing facts.
// These overwrite legacy mapped route values; no later source may override them.
if (routingReview?.ok) {
  const accepted = leadRoutingFacts(routingReview)
  Object.assign(evidence, accepted)
}
```

This is the integration call sequence, not a drop-in replacement for the whole runner: `raw`, `nodeId`, `evidence` and `miss` already exist at different points in that function, and `routingContext` is the explicit new trusted input. Wire the context at the runtime boundary; do not add fabricated story property names.

5. Retire the PRE call to `agent.applyLeadShape(...)`. Also retire the legacy PRE `assessLeadHandoff(...)` and global `parseLeadPlan(raw)` persistence in this runner path. They consume a different single-plan contract and would contradict the accepted multi-assignment decision. Retain other role deliverable checks, task identity, claim handling, repair budgets, candidate/QA checks and error capture. Audit legacy decision parsing/run-record creation so a missing `LEAD_DECISION` line does not invalidate the new canonical marker.

6. Persist the complete accepted `leadRouting` and `splitPlan` as authoritative execution data before the engine advances. Bind it to story + process instance + PRE task/attempt + execution generation + contract revision, not just the latest run for a story. Extend `ForgeGateEvidence`, its durable reader/writer, and `projectForgeGateFacts` together so XML's `plan-variable="splitPlan"` sees the SAME array LEAD approved. Use the project's migration/delivery rules if a new column is needed. Do not store this only through observer-only `appendForgeRunDetail(...).catch(...)`: losing the plan must prevent execution. This durable wiring is required integration work, not implemented by the pure functions below.

7. Clear stale split data when the accepted route is not SPLIT. `leadRoutingFacts` uses `splitCount: 0` and `splitPlan: []` as explicit reset values. Confirm the durable schema permits zero, or map it to an explicit SQL NULL clear. Merely omitting a field or passing NULL to the existing COALESCE merge can preserve a previous split count. Persist and project both route and plan consistently.

8. At worker launch, SOLO and plain SMITH receive `leadRouting.assignments[0].plan`. Each `smith_split_work` receives its own persisted `task.formData.splitBranch.plan` through `renderSmithWorkOrders`; never the whole parent plan. Check that the branch belongs to the accepted process/generation before invoking a worker. SOLO needs these work orders too: today the serial-plan injection is restricted to the Smith lane. Preserve branch identity and existing workspace ownership. Concurrency remains the executor's setting, independent of assignment count.

9. Keep `mergeChecks` with the accepted plan for LEAD POST/QA. Verify the integrated candidate against the frozen story acceptance, not merely independent child test results. A parseable command is not evidence that it executed or passed.

10. Run the targeted tests below and the relevant existing runner/phase/dispatch/fork tests. Before activation, exercise a real or integration-test sequence for SOLO, one SMITH, SPLIT, invalid-plan reprompt, exhausted reprompt, restart/replay and stale-plan rejection. Do not run the full regression without authorization.

No runtime integration, migration or deployment was performed for this handoff. The code below is the tested decision boundary and replacement prompt; items 2–9 are the explicit application wiring still needed.

## Verification completed here

25 focused behavioral tests passed, including multi-file SOLO, medium one-Smith and split choices, large split, required HOLD preservation, missing coverage, invented scope/proofs, same-file conflicts, unavailable fan-out, oversized workers, four chunks across two workers, malformed JSON and advisory difficulty scoring.

Tests executed with Node 24's TypeScript transform against exact copies of the five existing KRAKEN dependency files at the pinned commit. A temporary test mirror added `.ts` import extensions for Node resolution; the delivered source retains repository-style imports. No full repository typecheck, live database test or live engine run was performed.

Repository command after integration:

```sh
pnpm exec tsx --test workflow_app/tests/forge-lead-routing.test.ts
```

The following sections contain the complete new source files and tests.


## workflow_app/forge/forge-lead-routing.ts

```ts
import { assessSmithDispatch } from './forge-dispatch-gate'
import type { DispatchabilityFeatures } from './forge-dispatchability'
import type { SmithExecutionPlan } from './forge-execution-shaping'

export type Route = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'
export type Size = 'SMALL' | 'MEDIUM' | 'LARGE'
export type LeadAssignment = {
  id: string
  findingIds: string[]
  /** Other assignments whose OUTPUT this assignment needs. */
  dependsOn: string[]
  /** Existing contract/evidence references; not a claim of runtime readiness. */
  evidenceRefs: string[]
  reasoning: string
  features: DispatchabilityFeatures
  plan: SmithExecutionPlan
}
export type LeadProposal = {
  version: 1
  decision: Route
  size: Size
  sizeReason: string
  reason: string
  assignments: LeadAssignment[]
  /** Frozen story acceptance command(s) for the integrated candidate. */
  mergeChecks: string[]
}
export type RoutingContext = {
  findings: Array<{ id: string; required: boolean; hint?: string; seams: string[] }>
  /** Exact references supplied in the current Scout/Architect handoff. */
  evidenceRefs: string[]
  /** Supplied by the trusted runtime, never by the model. */
  splitEnabled: boolean
  maxSmiths: number
  /** Exact approved story/packet commands; not model-invented acceptance. */
  allowedProofs: string[]
}
export type RoutingReview =
  | { ok: false; errors: string[]; advisories: string[] }
  | { ok: true; proposal: LeadProposal; advisories: string[] }

const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === 'object' && !Array.isArray(v)
const nonempty = (v: unknown): v is string => typeof v === 'string' && v.trim().length > 0
const strings = (v: unknown): v is string[] => Array.isArray(v) && v.every(nonempty)
const unique = (v: string[]) => new Set(v).size === v.length
const riskKeys = ['uncertainty', 'contextBurden', 'proofBurden', 'coupling', 'changeNovelty', 'workerFit'] as const

function featuresValid(v: unknown): v is DispatchabilityFeatures {
  return object(v) && ['semanticSurface', 'dependencyDepth'].every(k =>
    Number.isInteger(v[k]) && Number(v[k]) >= 1 && Number(v[k]) <= 100
  ) && riskKeys.every(k => Number.isInteger(v[k]) && Number(v[k]) >= 1 && Number(v[k]) <= 5)
}

function planValidShape(v: unknown): v is SmithExecutionPlan {
  return object(v) && ['SMALL', 'MEDIUM', 'LARGE'].includes(String(v.size)) &&
    Array.isArray(v.chunks) && v.chunks.length >= 1 && v.chunks.length <= 3 &&
    v.chunks.every(c => object(c) && Number.isInteger(c.id) && nonempty(c.outcome) &&
      strings(c.surface) && c.surface.length > 0 && nonempty(c.invariant) && nonempty(c.proof) &&
      (c.dependsOn === undefined || (Array.isArray(c.dependsOn) && c.dependsOn.every(d => Number.isInteger(d) && d > 0))))
}

function proposalValidShape(v: unknown): v is LeadProposal {
  return object(v) && v.version === 1 && ['SOLO', 'SMITH', 'SPLIT', 'HOLD'].includes(String(v.decision)) &&
    ['SMALL', 'MEDIUM', 'LARGE'].includes(String(v.size)) && nonempty(v.sizeReason) && nonempty(v.reason) &&
    strings(v.mergeChecks) && Array.isArray(v.assignments) && v.assignments.length <= 8 &&
    v.assignments.every(a => object(a) && nonempty(a.id) && strings(a.findingIds) &&
      strings(a.dependsOn) && strings(a.evidenceRefs) && nonempty(a.reasoning) &&
      featuresValid(a.features) && planValidShape(a.plan))
}

/** Canonical repository-relative scope. Symbols count as their owning file for
 * concurrent write conflicts: file#a and file#b still share one merge surface. */
function pathOf(scope: string): string | null {
  const p = scope.trim().split('#')[0].replace(/^\.\//, '').replace(/\/+$/, '')
  if (!p || p.startsWith('/') || /[\\*?\[\]{}:]/.test(p) ||
      p.split('/').some(s => !s || s === '.' || s === '..')) return null
  return p
}
const within = (path: string, area: string) => path === area || path.startsWith(area + '/')
const overlap = (a: string, b: string) => within(a, b) || within(b, a)

/** The AI proposes; code accepts or returns corrections. Never silently reroute.
 * Uses the EXISTING KRAKEN gate once per assignment, not once per whole story.
 * Current XML is a sibling fork, so unfinished sibling dependencies are refused.
 */
export function reviewLeadProposal(raw: unknown, context: RoutingContext): RoutingReview {
  const errors: string[] = []
  const advisories: string[] = []
  if (!proposalValidShape(raw)) return { ok: false, errors: ['Malformed LEAD_ROUTING proposal'], advisories }
  const p = raw
  if (p.decision === 'HOLD') {
    if (p.assignments.length || p.mergeChecks.length) errors.push('HOLD must not dispatch assignments or checks')
    return errors.length ? { ok: false, errors, advisories } : { ok: true, proposal: p, advisories }
  }
  if (!Number.isInteger(context.maxSmiths) || context.maxSmiths < 1 || context.maxSmiths > 8) {
    errors.push('Runtime maxSmiths must be 1..8 (current XML ceiling)')
  }
  const required = context.findings.filter(f => f.required)
  if (!required.length) errors.push('No required findings supplied; obtain the bounded Architect handoff')
  if (!unique(context.findings.map(f => f.id))) errors.push('Duplicate finding IDs in Architect handoff')
  if (required.some(f => f.hint === 'HOLD')) errors.push('Required Architect HOLD remains unresolved')
  const n = p.assignments.length
  if ((p.decision === 'SOLO' || p.decision === 'SMITH') && n !== 1) errors.push(p.decision + ' requires one assignment')
  if (p.decision === 'SPLIT' && (!context.splitEnabled || n < 2 || n > context.maxSmiths)) {
    errors.push('SPLIT requires runtime support and 2..maxSmiths assignments')
  }
  if (p.size === 'SMALL' && p.decision === 'SPLIT') errors.push('SMALL work does not justify a split; revise the size or assignment')
  if (p.size === 'LARGE' && p.decision !== 'SPLIT') errors.push('LARGE requires two or more bounded Smith assignments')
  if (p.decision === 'SOLO' && p.size !== 'SMALL') errors.push('SOLO requires SMALL work')
  if (!unique(p.assignments.map(a => a.id))) errors.push('Duplicate assignment IDs')
  if (!p.mergeChecks.length || p.mergeChecks.some(c => !context.allowedProofs.includes(c))) {
    errors.push('Integrated acceptance must use the frozen story proof commands')
  }
  const covered = p.assignments.flatMap(a => a.findingIds)
  if (!unique(covered)) errors.push('A finding has multiple assignment owners; refine findings before dispatch')
  for (const f of required) if (!covered.includes(f.id)) errors.push('Unassigned required finding: ' + f.id)
  const pathsByAssignment: string[][] = []
  for (const a of p.assignments) {
    const prefix = a.id + ': '
    if (!a.findingIds.length || a.findingIds.some(id => !required.some(f => f.id === id))) {
      errors.push(prefix + 'claim only named required findings; no adjacent discovery in this story')
    }
    if (!a.evidenceRefs.length || a.evidenceRefs.some(ref => !context.evidenceRefs.includes(ref))) {
      errors.push(prefix + 'cite evidence present in the current handoff')
    }
    if (a.dependsOn.length) {
      errors.push(prefix + 'unfinished assignment dependencies require staged execution; current XML only supports sibling fan-out')
    }
    if (a.plan.size === 'LARGE') errors.push(prefix + 'assignment is still LARGE; decompose before dispatch')
    const scopes = required.filter(f => a.findingIds.includes(f.id)).flatMap(f => f.seams).map(pathOf)
    if (!scopes.length || scopes.some(s => s === null)) errors.push(prefix + 'Architect must supply explicit repository-relative scopes')
    const paths = a.plan.chunks.flatMap(c => c.surface).map(pathOf)
    pathsByAssignment.push(paths.filter((s): s is string => s !== null))
    if (paths.some(path => !path || !scopes.some(scope => scope && within(path, scope)))) {
      errors.push(prefix + 'chunk edits exceed the assigned Architect scope or use an invalid path')
    }
    if (a.plan.chunks.some(c => !context.allowedProofs.includes(c.proof))) {
      errors.push(prefix + 'chunk proof is not in the frozen story acceptance commands')
    }
    const gate = assessSmithDispatch(a.plan, { qualitativeFeatures: a.features })
    if (gate.verdict === 'HOLD') errors.push(...gate.reasons.map(r => prefix + r))
    else if (gate.verdict === 'FLAG') advisories.push(...gate.reasons.map(r => prefix + r))
    if (p.decision === 'SOLO' && (a.plan.size !== 'SMALL' || a.plan.chunks.length !== 1 ||
        a.features.semanticSurface !== 1 || a.features.dependencyDepth !== 1 ||
        riskKeys.some(k => a.features[k] > 2))) {
      errors.push(prefix + 'SOLO requires one bounded chunk, one behavior, low uncertainty/coupling/proof/context burden')
    }
  }
  if (p.decision === 'SPLIT') {
    for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) {
      if (pathsByAssignment[i].some(a => pathsByAssignment[j].some(b => overlap(a, b)))) {
        errors.push('Concurrent write conflict: ' + p.assignments[i].id + ' / ' + p.assignments[j].id)
      }
    }
  }
  return errors.length ? { ok: false, errors, advisories } : { ok: true, proposal: p, advisories }
}

/** One JSON line avoids brace counting inside shell commands/quoted strings.
 * Duplicates are ambiguous and rejected. Invalid output uses the existing
 * bounded reprompt path, not an invented default route. */
export function parseLeadRouting(notes: string): unknown {
  const lines = notes.split(/\r?\n/).map(s => s.trim()).filter(s => s.startsWith('LEAD_ROUTING:'))
  if (lines.length !== 1) return null
  try { return JSON.parse(lines[0].slice('LEAD_ROUTING:'.length).trim()) } catch { return null }
}

/** Narrow adapter for engine facts / dynamic-fork plan-variable. Persist the
 * same accepted proposal before advancing. This function performs NO I/O. */
export function leadRoutingFacts(review: Extract<RoutingReview, { ok: true }>) {
  const p = review.proposal
  return {
    leadDecision: p.decision,
    splitCount: p.decision === 'SPLIT' ? p.assignments.length : 0,
    splitPlan: p.decision === 'SPLIT' ? p.assignments : [],
    leadRouting: p,
  }
}
```


## workflow_app/forge/forge-lead-routing-prompt.ts

```ts
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
    'Emit exactly one un-fenced single JSON line beginning LEAD_ROUTING:. No parallel LEAD_DECISION or FORGE_EVIDENCE_JSON routing claim; the runner derives engine facts from the validated proposal.',
    'Schema: {"version":1,"decision":"SOLO|SMITH|SPLIT|HOLD","size":"SMALL|MEDIUM|LARGE","sizeReason":"...","reason":"...","assignments":[{"id":"a","findingIds":["..."],"dependsOn":[],"evidenceRefs":["..."],"reasoning":"...","features":{"semanticSurface":1,"dependencyDepth":1,"uncertainty":1,"contextBurden":1,"proofBurden":1,"coupling":1,"changeNovelty":1,"workerFit":1},"plan":{"size":"SMALL|MEDIUM","chunks":[{"id":1,"outcome":"...","surface":["path/file.ts#symbol"],"dependsOn":[],"invariant":"...","proof":"exact approved command"}]}}],"mergeChecks":["exact approved command"]}. For HOLD use assignments:[] and mergeChecks:[] and explain the blocker.',
    'Trusted routing context (references identify supplied evidence; they do not replace its contents):',
    JSON.stringify(context),
  ].join('\n')
}
```


## workflow_app/tests/forge-lead-routing.test.ts

```ts
import assert from 'node:assert/strict'
import test from 'node:test'
import { reviewLeadProposal, parseLeadRouting, leadRoutingFacts } from '../forge/forge-lead-routing'
import type { LeadProposal, RoutingContext } from '../forge/forge-lead-routing'
import { buildLeadRoutingDirective } from '../forge/forge-lead-routing-prompt'

const proof = 'pnpm exec tsx --test workflow_app/tests/example.test.ts'
function fixture(): { p: LeadProposal; context: RoutingContext } {
  return {
    context: {
      findings: [{ id: 'behavior', required: true, seams: ['db/contact.ts', 'services/contact.ts', 'components/contact.tsx'] }],
      evidenceRefs: ['architect:behavior', 'scout:contact'],
      splitEnabled: true, maxSmiths: 8, allowedProofs: [proof],
    },
    p: {
      version: 1, decision: 'SOLO', size: 'SMALL',
      sizeReason: 'One known behavior propagated through three existing surfaces.',
      reason: 'Lead already has the context; delegation adds overhead.',
      assignments: [{
        id: 'a', findingIds: ['behavior'], dependsOn: [], evidenceRefs: ['architect:behavior'],
        reasoning: 'Known mapping and one focused proof; no interface discovery.',
        features: { semanticSurface: 1, dependencyDepth: 1, uncertainty: 1, contextBurden: 1,
          proofBurden: 1, coupling: 1, changeNovelty: 1, workerFit: 1 },
        plan: { size: 'SMALL', chunks: [{ id: 1, outcome: 'Correct displayed contact value',
          surface: ['db/contact.ts', 'services/contact.ts', 'components/contact.tsx'],
          invariant: 'Preserve identities and unrelated rows', proof }] },
      }],
      mergeChecks: [proof],
    },
  }
}
function splitFixture() {
  const f = fixture()
  f.p.decision = 'SPLIT'; f.p.size = 'LARGE'
  f.p.assignments[0].plan.chunks[0].surface = ['db/contact.ts']
  f.context.findings.push({ id: 'other', required: true, seams: ['components/banner.tsx'] })
  const b = structuredClone(f.p.assignments[0])
  b.id = 'b'; b.findingIds = ['other']; b.plan.chunks[0].surface = ['components/banner.tsx']
  f.p.assignments.push(b)
  return f
}
function rejects(f: ReturnType<typeof fixture>, pattern: RegExp) {
  const before = JSON.stringify(f.p)
  const r = reviewLeadProposal(f.p, f.context)
  assert.equal(r.ok, false)
  if (!r.ok) assert.match(r.errors.join('\n'), pattern)
  assert.equal(JSON.stringify(f.p), before, 'validation must not mutate the proposed route')
}

test('small coherent change across three files stays SOLO', () => {
  const {p, context} = fixture(); assert.equal(reviewLeadProposal(p, context).ok, true)
})
test('medium cohesive work goes to one Smith despite separate files', () => {
  const {p, context} = fixture(); p.decision = 'SMITH'; p.size = 'MEDIUM'
  p.assignments[0].plan.size = 'MEDIUM'; p.assignments[0].features.coupling = 3
  assert.equal(reviewLeadProposal(p, context).ok, true)
})
test('large work dispatches two bounded Smiths and exact fork payload', () => {
  const {p, context} = splitFixture(); const r = reviewLeadProposal(p, context)
  assert.equal(r.ok, true)
  if (r.ok) { const facts = leadRoutingFacts(r); assert.equal(facts.splitCount, 2); assert.deepEqual(facts.splitPlan, p.assignments) }
})
test('medium work may also split', () => {
  const {p, context} = splitFixture(); p.size = 'MEDIUM'; assert.equal(reviewLeadProposal(p, context).ok, true)
})
test('large self-labelled SOLO cannot bypass validation', () => {
  const f = fixture(); f.p.size = 'LARGE'; rejects(f, /LARGE requires/)
})
test('small high uncertainty cannot use SOLO', () => {
  const f = fixture(); f.p.assignments[0].features.uncertainty = 3; rejects(f, /SOLO requires/)
})
test('a required Architect HOLD blocks even a well-shaped SOLO', () => {
  const f = fixture(); Object.assign(f.context.findings[0], { hint: 'HOLD' }); rejects(f, /Architect HOLD/)
})
test('sibling dependencies are not disguised as parallel work', () => {
  const f = splitFixture(); f.p.assignments[1].dependsOn = ['a']; rejects(f, /staged execution/)
})
test('same-file different-symbol writers conflict', () => {
  const f = splitFixture(); f.context.findings[1].seams = ['db/contact.ts']
  f.p.assignments[0].plan.chunks[0].surface = ['db/contact.ts#first']
  f.p.assignments[1].plan.chunks[0].surface = ['db/contact.ts#second']; rejects(f, /Concurrent write conflict/)
})
test('directory and child file writers conflict', () => {
  const f = splitFixture(); f.context.findings[0].seams = ['db']; f.context.findings[1].seams = ['db/other.ts']
  f.p.assignments[0].plan.chunks[0].surface = ['db']; f.p.assignments[1].plan.chunks[0].surface = ['db/other.ts']
  rejects(f, /Concurrent write conflict/)
})
test('all required findings must be assigned', () => {
  const f = fixture(); f.context.findings.push({id:'missing', required:true, seams:['db/missing.ts']}); rejects(f, /Unassigned required/)
})
test('adjacent findings cannot expand the accepted story', () => {
  const f = fixture(); f.context.findings.push({id:'later', required:false, seams:['db/later.ts']})
  f.p.assignments[0].findingIds.push('later'); rejects(f, /no adjacent discovery/)
})
test('missing evidence references are rejected', () => {
  const f = fixture(); f.p.assignments[0].evidenceRefs = ['invented']; rejects(f, /cite evidence/)
})
test('unapproved proof commands are rejected', () => {
  const f = fixture(); f.p.assignments[0].plan.chunks[0].proof = 'pnpm test'; rejects(f, /frozen story/)
})
test('path traversal and unknown scope are rejected', () => {
  for (const scope of ['../secrets', 'db/../secrets', '/tmp/out', '*', 'other.ts']) {
    const f = fixture(); f.p.assignments[0].plan.chunks[0].surface = [scope]; rejects(f, /invalid path/)
  }
})
test('one-Smith route resets split payload', () => {
  const {p, context} = fixture(); const r = reviewLeadProposal(p, context)
  assert.equal(r.ok, true); if (r.ok) assert.deepEqual(leadRoutingFacts(r).splitPlan, [])
})
test('runtime capability rejects unavailable fan-out', () => {
  const f = splitFixture(); f.context.splitEnabled = false; rejects(f, /runtime support/)
})
test('four total chunks across two Smiths are allowed; three is per assignment', () => {
  const {p, context} = splitFixture()
  for (const a of p.assignments) {
    a.plan.size = 'MEDIUM'
    a.plan.chunks.push({...structuredClone(a.plan.chunks[0]), id:2, dependsOn:[1]})
  }
  assert.equal(reviewLeadProposal(p, context).ok, true)
})
test('one oversized assignment cannot hide inside a split', () => {
  const f = splitFixture(); f.p.assignments[0].features.semanticSurface = 4
  rejects(f, /semantic-surface/)
})
test('difficulty scorer stays advisory', () => {
  const {p, context} = fixture(); p.decision = 'SMITH'; p.size = 'MEDIUM'
  const files = Array.from({length:20}, (_,i) => 'db/f' + i + '.ts')
  context.findings[0].seams = files; p.assignments[0].plan.chunks[0].surface = files
  const r = reviewLeadProposal(p, context); assert.equal(r.ok, true); assert.ok(r.advisories.length)
})
test('HOLD is a legitimate decision with no executable assignments', () => {
  const {p, context} = fixture(); p.decision = 'HOLD'; p.assignments = []; p.mergeChecks = []
  assert.equal(reviewLeadProposal(p, context).ok, true)
})
test('malformed model output never throws or defaults to SOLO', () => {
  for (const raw of [null, [], {}, {version:1}, { ...fixture().p, assignments:[null] }]) {
    assert.equal(reviewLeadProposal(raw, fixture().context).ok, false)
  }
})
test('JSON braces in quoted proof text parse correctly; duplicate markers fail', () => {
  const p = fixture().p; p.reason = 'Existing function returns {value}.'
  const line = 'LEAD_ROUTING: ' + JSON.stringify(p)
  assert.deepEqual(parseLeadRouting(line), p); assert.equal(parseLeadRouting(line + '\n' + line), null)
  assert.equal(parseLeadRouting('LEAD_ROUTING: {broken'), null)
})
test('prompt uses current capabilities and one authoritative routing marker', () => {
  const {context} = fixture(); const text = buildLeadRoutingDirective(context)
  assert.match(text, /LEAD_ROUTING/); assert.match(text, /PER ASSIGNMENT/)
  assert.match(text, /"splitEnabled":true/); assert.doesNotMatch(text, /SPLIT:n is currently DISABLED/)
})
test('chunk dependency zero is invalid, not a predecessor of chunk one', () => {
  const f = fixture(); f.p.assignments[0].plan.chunks[0].dependsOn = [0]
  rejects(f, /Malformed/)
})
```
