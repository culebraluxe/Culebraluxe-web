import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  FORGE_SDLC_KEY,
  FORGE_SDLC_VERSION,
  forgeSdlcXmlSource,
  parseForgeSdlc,
} from '../definitions/forge-sdlc'
import { validateWorkflowDefinitionXml } from '../definitions/validate-definition'
import { forgeCommandIsRouted } from '../forge-command-types'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — FORGE_SDLC-v1 superset definition.
//
// Proves the authoritative superset (classify -> research/bug/feature/hotfix/
// migration -> lead -> SOLO|SMITH|SPLIT -> QA -> DEV_OPS publish/migrate/deploy/
// smoke -> complete|cancelled|failed|archive_research, with a <dynamic-fork>
// SPLIT path and HOLD/resume) loads through all four layers under the Forge
// inventory.
//
// No database, no packages.
// ---------------------------------------------------------------------------

test('ENG-FORGE-V9: FORGE_SDLC-v1 superset passes all four validation layers', () => {
  const source = forgeSdlcXmlSource()
  const report = validateWorkflowDefinitionXml(source, forgeCommandIsRouted)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.deepEqual(report.errors, [])
  assert.equal(report.parsed!.key, 'FORGE_SDLC')
  assert.equal(report.parsed!.version, 1)
})

test('ENG-FORGE-V9: the loader parses + validates the superset (deploy-time guard)', () => {
  const parsed = parseForgeSdlc()
  assert.equal(parsed.key, FORGE_SDLC_KEY)
  assert.equal(parsed.version, FORGE_SDLC_VERSION)
  assert.equal(parsed.name, 'Forge Software Delivery Lifecycle')
  assert.equal(parsed.graph.startNodeId, 'start')
})

test('ENG-FORGE-V9: the superset exposes classify, execution-shape and HOLD routing', () => {
  const parsed = parseForgeSdlc()
  const nodes = parsed.graph.nodes
  for (const id of [
    'classify_work',
    'execution_shape',
    'qa_result',
    'failure_route',
    'hold',
    'hold_resolution',
  ]) {
    assert.ok(nodes[id], `node '${id}' must exist`)
  }
  assert.equal(nodes.execution_shape?.type, 'decision')
})

test('ENG-FORGE-V9: the SPLIT path uses a routed <dynamic-fork> that rejoins at split_join', () => {
  const parsed = parseForgeSdlc()
  const fork = parsed.graph.nodes.split_dispatch as NodeDefinition & {
    type: string
    countVariable?: string
    branchCommandType?: string
    join?: string
    minimum?: number
    maximum?: number
  }
  assert.equal(fork?.type, 'dynamic-fork')
  assert.equal(fork.countVariable, 'splitCount')
  assert.equal(fork.branchCommandType, 'forge.run_smith_split')
  assert.equal(fork.join, 'split_join')
  assert.ok(parsed.graph.nodes.split_join, 'split_join must exist')
  assert.ok(parsed.graph.nodes.lead_post, 'lead_post must exist after the join')
})

test('ENG-FORGE-V9: exactly the four termini are declared', () => {
  const parsed = parseForgeSdlc()
  const ends = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'end')
    .map((n) => n.id)
    .sort()
  assert.deepEqual(ends, ['archive_research', 'cancelled', 'complete', 'failed'])
})

test('ENG-FORGE-V9: every task/command responsibility is a Forge position', () => {
  const parsed = parseForgeSdlc()
  const valid = new Set(['scout', 'architect', 'lead', 'smith', 'qa', 'dev_ops'])
  const roles = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'task' || n.type === 'command' || n.type === 'dynamic-fork')
    .map((n) => (n as NodeDefinition & { responsibility?: string }).responsibility)
  for (const role of roles) {
    assert.ok(role !== undefined && valid.has(role), `responsibility '${role}' must be a Forge position`)
  }
})

test('ENG-FORGE-V9: no Inspector roster node (independent review is a QA capability)', () => {
  const parsed = parseForgeSdlc()
  const ids = Object.keys(parsed.graph.nodes)
  assert.ok(!ids.some((id) => /inspector/i.test(id)))
})
