import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  isWorkflowDefinitionValid,
  validateParsedDefinition,
  validateWorkflowDefinitionXml,
} from '../definitions/validate-definition'
import {
  parseReSupermodel,
  reSupermodelXmlSource,
} from '../definitions/re-supermodel'

// ---------------------------------------------------------------------------
// ENG-14 — Workflow Definition Validation / Static Analysis.
//
// Proves the four-layer pipeline fails invalid definitions deterministically
// at DEPLOY time (before anything becomes runnable), with actionable
// layer-prefixed diagnostics, and that the existing RE_supermodel definition
// remains deployable. No database, no packages.
// ---------------------------------------------------------------------------

const MINIMAL_VALID = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="work"/>
  </start-state>
  <task-node id="work" label="Do Work" responsibility="brokerage">
    <transition name="done" to="end"/>
  </task-node>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`

test('ENG-14: a well-formed, valid definition passes all four layers', () => {
  const report = validateWorkflowDefinitionXml(MINIMAL_VALID)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.deepEqual(report.errors, [])
  assert.deepEqual(report.xml.errors, [])
  assert.deepEqual(report.grammar.errors, [])
  assert.deepEqual(report.graph.errors, [])
  assert.deepEqual(report.application.errors, [])
  assert.ok(report.parsed, 'the parsed definition must be carried for the deploy pipeline')
  assert.equal(report.parsed!.key, 'demo')
})

test('ENG-14: malformed XML fails Layer 1 (well-formedness) before anything becomes runnable', () => {
  // Trailing garbage after the root element is not well-formed.
  const malformed = MINIMAL_VALID + '</process-definition>'
  const report = validateWorkflowDefinitionXml(malformed)
  assert.equal(report.valid, false)
  assert.ok(
    report.xml.errors.length > 0,
    `expected an xml well-formedness error, got: ${report.errors.join('; ')}`,
  )
  assert.equal(report.parsed, null)
  assert.ok(report.errors.some((e) => e.startsWith('[xml]')), 'errors must be layer-prefixed')
})

test('ENG-14: malformed XML (unterminated element) fails Layer 1', () => {
  const source = MINIMAL_VALID.replace(
    '<end-state id="end" label="Done" outcome="completed"/>',
    '<end-state id="end" label="Done" outcome="completed">',
  )
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, false)
  assert.ok(
    report.xml.errors.length > 0,
    `expected an xml well-formedness error, got: ${report.errors.join('; ')}`,
  )
  assert.equal(report.parsed, null)
  assert.ok(report.errors.some((e) => e.startsWith('[xml]')), 'errors must be layer-prefixed')
})

test('ENG-14: a grammar violation (unknown element) fails Layer 2', () => {
  const source = MINIMAL_VALID.replace(
    '<task-node',
    '<mystery-node',
  ).replace('</task-node>', '</mystery-node>')
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, false)
  assert.ok(
    report.grammar.errors.length > 0,
    `expected a grammar error, got: ${report.errors.join('; ')}`,
  )
  assert.ok(report.errors.some((e) => e.startsWith('[grammar]')))
})

test('ENG-14: an unreachable node fails Layer 3 (graph semantics)', () => {
  const source = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="work"/>
  </start-state>
  <task-node id="work" label="Do Work" responsibility="brokerage">
    <transition name="done" to="end"/>
  </task-node>
  <state id="orphan" label="Never Reached"/>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, false)
  assert.ok(
    report.errors.some((e) => e.startsWith('[graph]') && /'orphan' is unreachable/.test(e)),
    `expected an unreachable graph error, got: ${report.errors.join('; ')}`,
  )
})

test('ENG-14: an unknown application command fails Layer 4 (application contract)', () => {
  const source = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="cmd"/>
  </start-state>
  <command-node id="cmd" label="Unknown" command-type="deal.set_unknown_thing">
    <transition name="ok" to="end"/>
  </command-node>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, false)
  assert.ok(
    report.errors.some(
      (e) => e.startsWith('[application]') && /'deal\.set_unknown_thing' which has no router case/.test(e),
    ),
    `expected an application-contract error, got: ${report.errors.join('; ')}`,
  )
})

test('ENG-14: an impossible join (closed-loop required branch) fails Layer 3 at deploy', () => {
  const source = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="fork"/>
  </start-state>
  <fork id="fork" label="Fork">
    <transition name="loop" to="loop_node"/>
    <transition name="ok" to="join"/>
  </fork>
  <task-node id="loop_node" label="Loop" responsibility="brokerage">
    <transition name="again" to="loop_node"/>
  </task-node>
  <join id="join" label="Join">
    <transition name="go" to="end"/>
  </join>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, false)
  assert.ok(
    report.errors.some((e) => e.startsWith('[graph]') && /closed loop with no exit/.test(e)),
    `expected an impossible-join error, got: ${report.errors.join('; ')}`,
  )
})

test('ENG-14: intentional cycles (blocker loops) remain deployable', () => {
  const source = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="work"/>
  </start-state>
  <task-node id="work" label="Work" responsibility="brokerage">
    <transition name="issue" to="blocker"/>
    <transition name="done" to="end"/>
  </task-node>
  <task-node id="blocker" label="Blocker" responsibility="brokerage">
    <transition name="resolved" to="work"/>
  </task-node>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.equal(isWorkflowDefinitionValid(source), true)
})

test('ENG-14: RE_supermodel-v1 remains deployable through the full pipeline', () => {
  const source = reSupermodelXmlSource()
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, true, report.errors.join('; '))
  assert.deepEqual(report.errors, [])
  assert.ok(report.parsed, 'the pipeline must parse the authoritative definition')
  assert.equal(report.parsed!.key, 'RE_supermodel')

  // The loader itself (deploy-time guard) must agree.
  const parsed = parseReSupermodel()
  assert.equal(parsed.key, 'RE_supermodel')
  const layers = validateParsedDefinition(parsed)
  assert.equal(layers.valid, true, layers.errors.join('; '))
})

test('ENG-14: warnings surface without failing deployment', () => {
  const source = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="fork"/>
  </start-state>
  <fork id="fork" label="Fork">
    <transition name="escape" to="escaped"/>
    <transition name="ok" to="join"/>
  </fork>
  <task-node id="escaped" label="Escaped" responsibility="brokerage">
    <transition name="done" to="end"/>
  </task-node>
  <join id="join" label="Join">
    <transition name="go" to="end"/>
  </join>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`
  const report = validateWorkflowDefinitionXml(source)
  assert.equal(report.valid, true)
  assert.ok(
    report.warnings.some((w) => /required branch 'escape' of fork 'fork' never reaches a join/.test(w)),
    `expected a bypass warning, got: ${report.warnings.join('; ')}`,
  )
})
