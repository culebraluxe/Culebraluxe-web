import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { ArchitectAgent } from '../forge/agents/role-agents'
import {
  ARCHITECT_HANDOFF_PREFIX,
  parseArchitectHandoff,
  renderArchitectHandoff,
  type ArchitectHandoff,
  type ArchitectHandoffFinding,
} from '../forge/agents/architect-handoff'
import { ARCHITECT_HANDOFF_MISSING, assessArchitectHandoff } from '../forge/agents/architect/assess'
import { existsOnGitBaseRef } from '../forge/agents/architect/exists-git'
import { buildArchitectDirective } from '../forge/forge-architect-directive'
import { MAX_SEAMS_PER_FINDING } from '../forge/forge-shaping'
import type { ForgeGateEvidence } from '../forge/forge-facts'

// ---------------------------------------------------------------------------
// ARCHITECT ROLE SMOKE — the smallest thing that exercises the architect lane.
//
// No model, no DB, no `.env.local`: a canned architect reply is walked through
// the REAL ArchitectAgent, and seam existence is checked against REAL git. Runs
// in well under a second, so it can be run on every change to the contract.
//
// Why this file exists: the other architect tests each cover one layer — the
// base class (forge-phase-agent), the parser (handoff.test), the OLD
// FORGE_FINDINGS_JSON path (forge-architect-contract). None ran a reply through
// ArchitectAgent.collect(), which is where a handoff becomes findings or becomes
// a HOLD. The seam-existence check is fail-closed and was never asserted end to
// end, and it is why a run HOLDs on a plan that reads perfectly to a human.
//
// Run: node --import tsx --test workflow_app/tests/forge-architect-role.test.ts
// ---------------------------------------------------------------------------

const REPO_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const BASE_REF = execFileSync('git', ['-C', REPO_DIR, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
const existsOnBaseRef = existsOnGitBaseRef(REPO_DIR)

/** A real file and a real directory on BASE_REF — the two legal shapes of a seam. */
const REAL_FILE = 'workflow_app/forge/agents/role-agents.ts'
const REAL_DIR = 'workflow_app/forge/agents'
const INVENTED_FILE = 'workflow_app/forge/agents/this-file-does-not-exist.ts'

const finding = (overrides: Partial<ArchitectHandoffFinding> = {}): ArchitectHandoffFinding => ({
  id: 'F1',
  required: true,
  summary: 'route the architect handoff through the phase agent',
  preconditions: ['baseRef pinned'],
  scope: [REAL_FILE],
  postconditions: ['findings are set from the handoff'],
  classes: ['ArchitectHandoff'],
  risks: [],
  hint: 'SAME_UNIT',
  ...overrides,
})

/** What the architect actually returns: prose, then ONE un-fenced handoff line. */
function architectReply(findings: ArchitectHandoffFinding[], baseRef = BASE_REF): string {
  const handoff: ArchitectHandoff = { version: 1, baseRef, findings }
  return `I surveyed the frozen story against ${baseRef.slice(0, 12)}.\n\n${renderArchitectHandoff(handoff)}`
}

const ports = { existsOnBaseRef }

test('a valid handoff becomes findings and is not rejected', () => {
  const agent = new ArchitectAgent('architect')
  const raw = architectReply([finding()])

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.equal(evidence.deliverableRejection, undefined, 'a legal handoff must not be rejected')
  assert.equal(evidence.findings?.length, 1)
  assert.equal(evidence.findings?.[0].id, 'F1')
  assert.deepEqual(evidence.findings?.[0].seams, [REAL_FILE])
  // The gate's own decider must agree: nothing missing once findings are set.
  assert.deepEqual(agent.missingDeliverables(evidence, raw, false, true), [])
})

test('an invented seam is fail-closed: rejected, findings UNSET, and the gate HOLDs', () => {
  const agent = new ArchitectAgent('architect')
  const raw = architectReply([finding({ scope: [INVENTED_FILE] })])

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.match(String(evidence.deliverableRejection), /does not exist on/)
  assert.equal(evidence.findings, undefined, 'unassessed evidence must never become findings')
  assert.ok(
    agent.missingDeliverables(evidence, raw, false, false).includes('architect-plan'),
    'a rejected handoff is never a delivered plan',
  )
})

test('a new file is legal when the finding declares its existing parent DIRECTORY', () => {
  const agent = new ArchitectAgent('architect')
  const raw = architectReply([finding({ summary: 'add an architect smoke test', scope: [REAL_DIR] })])

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.equal(evidence.deliverableRejection, undefined)
  assert.deepEqual(evidence.findings?.[0].seams, [REAL_DIR])
})
test('a reply with no handoff line is a HOLD that names the exact marker to emit', () => {
  const agent = new ArchitectAgent('architect')
  const raw = 'I read the code. It looks fine and needs no changes.'

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.equal(evidence.deliverableRejection, ARCHITECT_HANDOFF_MISSING)
  assert.match(ARCHITECT_HANDOFF_MISSING, /FORGE_ARCHITECT_HANDOFF:/)
  assert.equal(parseArchitectHandoff(raw), null)
  assert.ok(agent.missingDeliverables(evidence, raw, false, false).includes('architect-plan'))
})

test('a required HOLD must name a concrete risk', () => {
  const agent = new ArchitectAgent('architect')
  const raw = architectReply([finding({ hint: 'HOLD', risks: [] })])

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.match(String(evidence.deliverableRejection), /required HOLD must name a concrete risk/)
  assert.equal(evidence.findings, undefined)
})

test('a fat finding is rejected with the ceiling named, not silently truncated', () => {
  const agent = new ArchitectAgent('architect')
  const fat = Array.from({ length: MAX_SEAMS_PER_FINDING + 1 }, () => REAL_FILE)
  const raw = architectReply([finding({ scope: fat })])

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.match(String(evidence.deliverableRejection), /recut \(max \d+\)/)
  assert.equal(evidence.findings, undefined)
})

test('the handoff line survives the write-on-exit prose cap and still reparses', () => {
  const agent = new ArchitectAgent('architect')
  const raw = architectReply([finding()])
  const noisy = `${'surveyed line. '.repeat(600)}\n\n${raw}`

  const brief = agent.architectBrief(noisy)

  assert.ok(brief, 'an architect with a handoff must write a brief')
  assert.match(brief!, /\[Architect prose truncated/)
  const reparsed = parseArchitectHandoff(brief!)
  assert.ok(reparsed, 'the contract line is never sliced')
  assert.equal(reparsed!.baseRef, BASE_REF)
  assert.equal(reparsed!.findings[0].id, 'F1')
})

test('the directive and the parser agree on the required keys (drift guard)', () => {
  const directive = buildArchitectDirective(BASE_REF, ['node --import tsx --test x.test.ts'])

  // Every key parseArchitectHandoff rejects on absence must be named in the
  // instruction the model is actually given, or the contract is unlearnable.
  for (const key of ['version', 'baseRef', 'findings', 'scope', 'required', 'risks']) {
    assert.ok(directive.includes(key), `directive must name ${key}`)
  }
  assert.match(directive, /FORGE_ARCHITECT_HANDOFF:/)
  assert.ok(directive.includes(BASE_REF), 'the model is told which SHA it inspected')

  // And the schema line it advertises must actually parse.
  const advertised = JSON.parse(
    directive.slice(directive.indexOf('{', directive.indexOf('Schema:'))),
  ) as unknown as ArchitectHandoff
  const reply = `${ARCHITECT_HANDOFF_PREFIX} ${JSON.stringify(advertised)}`
  assert.ok(parseArchitectHandoff(reply), 'the advertised schema must round-trip through the parser')
})

test('assessment is advisory-clean on the happy path and reports nothing to self-heal', () => {
  const handoff = parseArchitectHandoff(architectReply([finding()]))
  const assessment = assessArchitectHandoff(handoff, { existsOnBaseRef })

  assert.equal(assessment.ok, true)
  if (assessment.ok) assert.deepEqual(assessment.advisories, [])
})

test('the legacy FORGE_FINDINGS_JSON reply still passes (the fallback is not broken)', () => {
  const agent = new ArchitectAgent('architect')
  const raw =
    'Legacy plan.\n' +
    'FORGE_FINDINGS_JSON: [{"id":"legacy-1","summary":"keep the fallback working","required":true,' +
    '"seams":["workflow_app/forge/agents/role-agents.ts"],"hint":"SAME_UNIT"}]'

  const evidence = agent.collect({} as ForgeGateEvidence, raw, ports)

  assert.equal(evidence.deliverableRejection, undefined, 'the legacy marker must not be rejected')
  assert.equal(evidence.findings?.length, 1)
  assert.equal(evidence.findings?.[0].id, 'legacy-1')
})


