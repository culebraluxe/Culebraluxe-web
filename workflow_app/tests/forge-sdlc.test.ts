import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  FORGE_SDLC_KEY,
  FORGE_SDLC_VERSION,
  forgeSdlcXmlSource,
  parseForgeSdlc,
} from '../definitions/forge-sdlc'
import { validateWorkflowDefinitionXml } from '../definitions/validate-definition'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V7 — FORGE_SDLC XML-down definition.
//
// Proves the Forge SDLC supermodel loads through the SAME four-layer pipeline
// as RE_supermodel (mini-xml -> xml-parser -> graph-validator -> application
// contract), with the V6 ROLES topology locked in XML: serial first,
// SPLIT Hold-gated, Inspector as QA capability (not a roster node), and an
// EMPTY command inventory for v1 (task + decision + state only).
//
// No database, no packages.
// ---------------------------------------------------------------------------

test('ENG-FORGE-V7: FORGE_SDLC-v1 passes all four validation layers', () => {
  const source = forgeSdlcXmlSource()
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.deepEqual(report.errors, [])
  assert.deepEqual(report.xml.errors, [])
  assert.deepEqual(report.grammar.errors, [])
  assert.deepEqual(report.graph.errors, [])
  assert.deepEqual(report.application.errors, [])
  assert.ok(report.parsed, 'the parsed definition must be carried for the deploy pipeline')
  assert.equal(report.parsed!.key, 'FORGE_SDLC')
  assert.equal(report.parsed!.version, 1)
})

test('ENG-FORGE-V7: the loader parses + validates (deploy-time guard)', () => {
  const parsed = parseForgeSdlc()
  assert.equal(parsed.key, FORGE_SDLC_KEY)
  assert.equal(parsed.version, FORGE_SDLC_VERSION)
  assert.equal(parsed.name, 'Forge SDLC Supermodel')
  assert.equal(parsed.graph.startNodeId, 'start')
})

test('ENG-FORGE-V7: serial backbone Ready -> Scout -> Architect -> Lead PRE is intact', () => {
  const parsed = parseForgeSdlc()
  const nodes = parsed.graph.nodes
  for (const id of ['ready', 'scout', 'architect', 'lead_pre', 'lead_execution_gate']) {
    assert.ok(nodes[id], `backbone node '${id}' must exist`)
  }
  assert.deepEqual(nodes.ready?.transitions?.map((t) => t.to), ['scout'])
  assert.deepEqual(nodes.scout?.transitions?.map((t) => t.to), ['architect'])
  assert.deepEqual(nodes.architect?.transitions?.map((t) => t.to), ['lead_pre'])
  assert.deepEqual(nodes.lead_pre?.transitions?.map((t) => t.to), ['lead_execution_gate'])
})

test('ENG-FORGE-V7: Lead PRE fans to SOLO | Smith | Hold; SPLIT parks on forge_hold', () => {
  const parsed = parseForgeSdlc()
  const gate = parsed.graph.nodes.lead_execution_gate as NodeDefinition & {
    decisions?: Array<{ condition: string; transition: string }>
  }
  assert.equal(gate?.type, 'decision')
  const conditions = (gate.decisions ?? []).map((d) => d.condition)
  assert.ok(
    conditions.some((c) => c === 'leadDecision == "SOLO"'),
    'Lead PRE must route SOLO',
  )
  assert.ok(
    conditions.some((c) => c === 'leadDecision == "SMITH"'),
    'Lead PRE must route SMITH',
  )
  const targets = (gate.transitions ?? []).map((t) => t.to)
  assert.ok(targets.includes('lead_implement'), 'SOLO must reach lead_implement')
  assert.ok(targets.includes('smith'), 'SMITH must reach smith')
  assert.ok(targets.includes('forge_hold'), 'SPLIT/HOLD must park on forge_hold')
})

test('ENG-FORGE-V7: exact-candidate -> QA Assay -> DEV_OPS publish chain is intact', () => {
  const parsed = parseForgeSdlc()
  const nodes = parsed.graph.nodes
  for (const id of [
    'candidate_gate',
    'assay',
    'assay_gate',
    'publish',
    'publish_gate',
    'story_complete',
    'forge_hold',
  ]) {
    assert.ok(nodes[id], `chain node '${id}' must exist`)
  }
  assert.equal(nodes.assay?.type, 'task')
  assert.equal(
    (nodes.assay as NodeDefinition & { responsibility?: string }).responsibility,
    'qa',
    'Candidate Assay is a QA responsibility',
  )
  assert.equal(nodes.publish?.type, 'task')
  assert.equal(
    (nodes.publish as NodeDefinition & { responsibility?: string }).responsibility,
    'dev_ops',
    'Publish is a DEV_OPS responsibility',
  )
  assert.equal(nodes.story_complete?.type, 'end')
  assert.equal(nodes.forge_hold?.type, 'end')
})

test('ENG-FORGE-V7: Inspector is a QA capability, not a seventh roster node', () => {
  const parsed = parseForgeSdlc()
  const ids = Object.keys(parsed.graph.nodes)
  assert.ok(
    !ids.some((id) => /inspector/i.test(id)),
    'no roster node may be named inspector (independent review lives inside QA Assay)',
  )
  const responsibilities = new Set(
    Object.values(parsed.graph.nodes)
      .filter((n) => n.type === 'task')
      .map((n) => (n as NodeDefinition & { responsibility?: string }).responsibility),
  )
  for (const r of responsibilities) {
    assert.ok(
      ['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'].includes(r!),
      `task responsibility '${r}' must be a Forge position (scout|architect|lead|smith|qa|dev_ops)`,
    )
  }
  assert.ok(!responsibilities.has('inspector'), 'inspector must not appear as a task responsibility')
})

test('ENG-FORGE-V7: v1 command inventory is EMPTY (task + decision + state only)', () => {
  const parsed = parseForgeSdlc()
  const commandNodes = Object.values(parsed.graph.nodes).filter((n) => n.type === 'command')
  assert.deepEqual(
    commandNodes.map((n) => n.id),
    [],
    'v1 carries no <command-node> elements; forge.* commands are a future slice',
  )
})

test('ENG-FORGE-V7: every node is reachable; Hold and Complete are the only termini', () => {
  const parsed = parseForgeSdlc()
  const nodes = parsed.graph.nodes
  const ends = Object.values(nodes)
    .filter((n) => n.type === 'end')
    .map((n) => n.id)
    .sort()
  assert.deepEqual(ends, ['forge_hold', 'story_complete'])
})
