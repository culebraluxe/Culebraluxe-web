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
