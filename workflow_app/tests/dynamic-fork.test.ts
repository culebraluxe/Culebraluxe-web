import assert from 'node:assert/strict'
import test from 'node:test'

import { parseProcessDefinitionXml } from '../xml'
import { validateWorkflowDefinitionXml } from '../definitions/validate-definition'
import { forgeCommandIsRouted } from '../forge-command-types'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — engine <dynamic-fork> definition-time support.
//
// The parser accepts a <dynamic-fork count-variable=... branch-command-type=...
// join=... minimum=... maximum=...>, the graph-validator checks its contract
// and treats its join as reachable, and Layer 4 requires the branch command to
// be routed in the domain inventory. This is the definition/validation half of
// N-way fan-out; runtime fan-out execution is wired separately.
//
// No database, no packages.
// ---------------------------------------------------------------------------

const FORK_XML = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="fork_node"/>
  </start-state>
  <dynamic-fork id="fork_node" label="Split" count-variable="splitCount"
    branch-command-type="forge.run_smith_split" join="join_node" minimum="2" maximum="8"/>
  <join id="join_node" label="Join">
    <transition name="complete" to="done"/>
  </join>
  <end-state id="done" label="Done" outcome="completed"/>
</process-definition>
`

test('ENG-FORGE-V9: the parser maps a <dynamic-fork> onto the graph', () => {
  const parsed = parseProcessDefinitionXml(FORK_XML)
  const fork = parsed.graph.nodes.fork_node as NodeDefinition & {
    countVariable?: string
    branchCommandType?: string
    join?: string
    minimum?: number
    maximum?: number
  }
  assert.equal(fork.type, 'dynamic-fork')
  assert.equal(fork.countVariable, 'splitCount')
  assert.equal(fork.branchCommandType, 'forge.run_smith_split')
  assert.equal(fork.join, 'join_node')
  assert.equal(fork.minimum, 2)
  assert.equal(fork.maximum, 8)
})

test('ENG-FORGE-V9: a dynamic-fork definition passes all four layers under the Forge inventory', () => {
  const report = validateWorkflowDefinitionXml(FORK_XML, forgeCommandIsRouted)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.deepEqual(report.errors, [])
  // The join is reachable from the dynamic-fork (no false unreachable error).
  const parsed = report.parsed!
  assert.ok(parsed.graph.nodes.join_node, 'join node must exist')
})

test('ENG-FORGE-V9: Layer 4 fails when the dynamic-fork branch command is not routed', () => {
  const bad = FORK_XML.replace('forge.run_smith_split', 'deal.set_stage_closed')
  const report = validateWorkflowDefinitionXml(bad, forgeCommandIsRouted)
  assert.equal(report.valid, false)
  assert.ok(
    report.errors.some((e) => e.includes('deal.set_stage_closed') && e.includes('no router case')),
    'branch-command-type must be routed in the domain inventory',
  )
})

test('ENG-FORGE-V9: minimum may not exceed maximum', () => {
  const bad = FORK_XML.replace('minimum="2" maximum="8"', 'minimum="8" maximum="2"')
  const report = validateWorkflowDefinitionXml(bad, forgeCommandIsRouted)
  assert.equal(report.valid, false)
  assert.ok(report.errors.some((e) => /minimum.*must not exceed.*maximum/i.test(e)))
})
